//! Géométrie du plan de feu (vue de dessus ; portage de `web/js/plot/geometry.js`) : empreintes des
//! projecteurs et du grip, faisceaux. Repère du plan : x vers la droite, z vers le bas de l'écran
//! (le fond / cyclo en haut, la caméra en bas).

use crate::catalog::{ObjectType, grip_size, grip_wh, prop_size, props_of};
use crate::js::{hypot, max2, min2, round};
use arklight_core::library::FixtureType;
use arklight_core::project::{Fixture, SceneObject};

/// Point du plan [x, z] (mètres) ou de l'écran [x, y] (pixels).
pub type P2 = [f64; 2];
/// Matrice de rotation 3×3 (lignes).
pub type M3 = [[f64; 3]; 3];

/// Demi-dimensions locales du corps d'un projecteur (axes du modèle 3D).
pub fn body_half_extents(ty: &FixtureType) -> [f64; 3] {
    let g = |k: &str| ty.dim(k).unwrap_or(f64::NAN);
    // `g.width || 0.3` : absent ou nul
    let or = |k: &str, d: f64| ty.dim(k).filter(|v| *v != 0.0 && !v.is_nan()).unwrap_or(d);
    match ty.geometry.kind.as_str() {
        "tube" => [g("diameter") / 2.0, g("length") / 2.0, g("diameter") / 2.0],
        "bar" => [g("length") / 2.0, g("height") / 2.0, g("depth") / 2.0],
        "par" => [g("diameter") / 2.0, g("diameter") / 2.0, g("depth") / 2.0],
        "bulb" => [g("diameter") / 2.0, or("length", g("diameter")) / 2.0, g("diameter") / 2.0],
        _ => [or("width", 0.3) / 2.0, or("height", 0.3) / 2.0, or("depth", 0.1) / 2.0],
    }
}

/// Matrice de rotation d'un élément de scène (angles d'Euler XYZ en radians, comme three.js).
pub fn rotation_matrix(rot: [f64; 3]) -> M3 {
    let v = |x: f64| if x.is_nan() { 0.0 } else { x };
    let (x, y, z) = (v(rot[0]), v(rot[1]), v(rot[2]));
    let (a, b, c, d, e, f) = (x.cos(), x.sin(), y.cos(), y.sin(), z.cos(), z.sin());
    let (ae, af, be, bf) = (a * e, a * f, b * e, b * f);
    [[c * e, -c * f, d], [af + be * d, ae - bf * d, -b * c], [bf - ae * d, be + af * d, a * c]]
}

/// `Vector3.applyMatrix4` (rotation seule).
pub fn apply(m: &M3, [x, y, z]: [f64; 3]) -> [f64; 3] {
    let row = |r: &[f64; 3]| r[0] * x + r[1] * y + r[2] * z + 0.0;
    [row(&m[0]), row(&m[1]), row(&m[2])]
}

/// Enveloppe convexe (monotone chain) d'une liste de points [x, z].
pub fn convex_hull(points: &[P2]) -> Vec<P2> {
    let mut pts = points.to_vec();
    pts.sort_by(|a, b| {
        // `a[0] - b[0] || a[1] - b[1]` (0 ou NaN : on départage par z)
        let d = a[0] - b[0];
        let d = if d == 0.0 || d.is_nan() { a[1] - b[1] } else { d };
        d.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal)
    });
    if pts.len() < 3 {
        return pts;
    }
    let cross = |o: P2, a: P2, b: P2| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let mut lower: Vec<P2> = Vec::new();
    for &p in &pts {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<P2> = Vec::new();
    for &p in pts.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Empreinte au sol d'une boîte locale (demi-dimensions) placée et orientée dans la scène.
pub fn footprint(half: [f64; 3], position: [f64; 3], rotation: [f64; 3]) -> Vec<P2> {
    let m = rotation_matrix(rotation);
    let mut corners = Vec::with_capacity(8);
    for sx in [-1.0, 1.0] {
        for sy in [-1.0, 1.0] {
            for sz in [-1.0, 1.0] {
                let v = apply(&m, [sx * half[0], sy * half[1], sz * half[2]]);
                corners.push([position[0] + v[0], position[2] + v[2]]);
            }
        }
    }
    convex_hull(&corners)
}

/// Projecteur vu de dessus.
#[derive(Debug, Clone, PartialEq)]
pub struct FixturePlan {
    /// empreinte du corps
    pub poly: Vec<P2>,
    pub center: P2,
    /// direction du faisceau projetée au sol (None s'il pointe presque à la verticale)
    pub dir: Option<P2>,
    /// pointe vers le sol
    pub down: bool,
    /// départ du faisceau (face avant)
    pub front: P2,
    pub height: f64,
}

/// `fixturePlan`
pub fn fixture_plan(f: &Fixture, ty: &FixtureType) -> FixturePlan {
    let half = body_half_extents(ty);
    let poly = footprint(half, f.position, f.rotation);
    let m = rotation_matrix(f.rotation);
    let axis = apply(&m, [0.0, 0.0, 1.0]); // axe du faisceau (+Z local)
    let flat = hypot(axis[0], axis[2]);
    let kind = ty.geometry.kind.as_str();
    let omni = matches!(kind, "bulb" | "tube" | "box-device") || ty.pixels == 0; // les tubes éclairent tout autour
    let front = apply(&m, [0.0, 0.0, half[2]]);
    FixturePlan {
        poly,
        center: [f.position[0], f.position[2]],
        dir: (!omni && flat > 0.25).then(|| [axis[0] / flat, axis[2] / flat]),
        down: !omni && flat <= 0.25 && axis[1] < 0.0,
        front: [f.position[0] + front[0], f.position[2] + front[2]],
        height: f.position[1],
    }
}

/// Objet vu de dessus (grip ou décor).
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectPlan {
    pub poly: Vec<P2>,
    pub center: P2,
}

/// `objectPlan` : empreinte selon la famille du grip ou les dimensions du décor.
pub fn object_plan(o: &SceneObject, ty: ObjectType) -> ObjectPlan {
    let center = [o.position[0], o.position[2]];
    let half = match ty {
        ObjectType::Prop(p) if ty.is_prop() => {
            let [w, h, d] = prop_size(p, props_of(o));
            [w / 2.0, h / 2.0, d / 2.0]
        }
        ObjectType::Grip(g) => {
            let (w, h) = grip_wh(g);
            match g.family.as_deref() {
                Some("frame" | "flag") => [w / 2.0, h / 2.0, 0.02],
                Some("bounce") => [w / 2.0, (if h != 0.0 && !h.is_nan() { h } else { w }) / 2.0, 0.03],
                Some("box" | "bag") => grip_size(g).map(|v| v / 2.0),
                _ => [0.35, 0.05, 0.35], // pied seul : emprise des jambes
            }
        }
        ObjectType::Prop(_) => [0.35, 0.05, 0.35],
    };
    ObjectPlan { poly: footprint(half, o.position, o.rotation), center }
}

/// Oreilles d'un cadre (milieu des côtés, où se fixent les pieds), au sol.
pub fn frame_ears(o: &SceneObject, w: f64) -> [P2; 2] {
    let m = rotation_matrix(o.rotation);
    [-1.0, 1.0].map(|k| {
        let v = apply(&m, [(k * w) / 2.0, 0.0, 0.0]);
        [o.position[0] + v[0], o.position[2] + v[2]]
    })
}

/// Distance d'un point à un segment (pour cliquer sur les lignes et flèches).
pub fn dist_to_segment([px, pz]: P2, [ax, az]: P2, [bx, bz]: P2) -> f64 {
    let dx = bx - ax;
    let dz = bz - az;
    let l = dx * dx + dz * dz;
    let l = if l == 0.0 || l.is_nan() { 1e-9 } else { l };
    let t = max2(0.0, min2(1.0, ((px - ax) * dx + (pz - az) * dz) / l));
    hypot(px - (ax + t * dx), pz - (az + t * dz))
}

/// Point dans un polygone (règle pair-impair).
pub fn point_in_polygon([x, z]: P2, poly: &[P2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let [xi, zi] = poly[i];
        let [xj, zj] = poly[j];
        if (zi > z) != (zj > z) && x < ((xj - xi) * (z - zi)) / (zj - zi + 1e-12) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Tourne un élément autour de la verticale (lacet) en gardant son inclinaison (`rotateYaw`,
/// calcul des quaternions de three.js) ; angles arrondis au millième.
pub fn rotate_yaw(rotation: [f64; 3], delta: f64) -> [f64; 3] {
    // Quaternion.setFromEuler (XYZ)
    let (c1, c2, c3) = ((rotation[0] / 2.0).cos(), (rotation[1] / 2.0).cos(), (rotation[2] / 2.0).cos());
    let (s1, s2, s3) = ((rotation[0] / 2.0).sin(), (rotation[1] / 2.0).sin(), (rotation[2] / 2.0).sin());
    let b = [s1 * c2 * c3 + c1 * s2 * s3, c1 * s2 * c3 - s1 * c2 * s3, c1 * c2 * s3 + s1 * s2 * c3, c1 * c2 * c3 - s1 * s2 * s3];
    // setFromAxisAngle((0, 1, 0), delta)
    let s = (delta / 2.0).sin();
    let a = [0.0 * s, 1.0 * s, 0.0 * s, (delta / 2.0).cos()];
    // premultiply : a × b
    let (qax, qay, qaz, qaw) = (a[0], a[1], a[2], a[3]);
    let (qbx, qby, qbz, qbw) = (b[0], b[1], b[2], b[3]);
    let x = qax * qbw + qaw * qbx + qay * qbz - qaz * qby;
    let y = qay * qbw + qaw * qby + qaz * qbx - qax * qbz;
    let z = qaz * qbw + qaw * qbz + qax * qby - qay * qbx;
    let w = qaw * qbw - qax * qbx - qay * qby - qaz * qbz;
    // Matrix4.makeRotationFromQuaternion puis Euler.setFromRotationMatrix (XYZ)
    let (x2, y2, z2) = (x + x, y + y, z + z);
    let (xx, xy, xz) = (x * x2, x * y2, x * z2);
    let (yy, yz, zz) = (y * y2, y * z2, z * z2);
    let (wx, wy, wz) = (w * x2, w * y2, w * z2);
    let m11 = 1.0 - (yy + zz);
    let m12 = xy - wz;
    let m13 = xz + wy;
    let m22 = 1.0 - (xx + zz);
    let m23 = yz - wx;
    let m32 = yz + wx;
    let m33 = 1.0 - (xx + yy);
    let ey = m13.clamp(-1.0, 1.0).asin();
    let (ex, ez) = if m13.abs() < 0.9999999 { ((-m23).atan2(m33), (-m12).atan2(m11)) } else { (m32.atan2(m22), 0.0) };
    let r3 = |v: f64| round(v * 1000.0) / 1000.0;
    [r3(ex), r3(ey), r3(ez)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enveloppe_et_polygone() {
        let hull = convex_hull(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.5, 0.5], [1.0, 0.0]]);
        assert_eq!(hull, vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        assert!(point_in_polygon([0.5, 0.5], &hull));
        assert!(!point_in_polygon([1.5, 0.5], &hull));
        assert!((dist_to_segment([0.5, 1.0], [0.0, 0.0], [1.0, 0.0]) - 1.0).abs() < 1e-12);
        assert_eq!(rotate_yaw([0.0, 0.0, 0.0], std::f64::consts::FRAC_PI_2), [0.0, 1.571, 0.0]);
    }
}
