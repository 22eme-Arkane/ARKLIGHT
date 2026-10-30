//! Image d'un projet sans fenêtre (vérifier le moteur, comparer avec la version JavaScript) :
//!   cargo run --release -p arklight-render --example snapshot -- projet.json image.png [largeur hauteur]

use arklight_core::Project;
use arklight_core::engine::{Ctx, Lights, Universes};
use arklight_render::{Camera, Renderer, stage::scene_from_project, wgpu};
use std::collections::HashSet;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(input), Some(output)) = (args.first(), args.get(1)) else {
        eprintln!("usage : snapshot projet.json image.png [largeur hauteur]");
        std::process::exit(2);
    };
    let width: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1600);
    let height: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(900);
    let project = Project::open(&PathBuf::from(input))?;
    // sortie des projecteurs à l'instant 1 s (sans DMX reçu)
    let (dmx, live) = (Universes::new(), HashSet::new());
    let mut lights = Lights::default();
    lights.update(&project.fixtures, &Ctx { dmx: &dmx, live: &live, time: 1.0 });
    let scene = scene_from_project(&project, &lights);
    if std::env::var_os("ARKLIGHT_LIGHTS").is_some() {
        // intensités des lumières (comparaison avec la version JavaScript)
        for l in &scene.lights {
            eprintln!("{:?} {:.2} cd", l.kind, l.candela);
        }
    }

    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() }))?;
    let info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

    let mut renderer = Renderer::new(&device);
    let start = std::time::Instant::now();
    let texture = renderer.render(&device, &queue, (width, height), &scene, &Camera::default());
    // mesure : 60 images de plus
    for _ in 0..60 {
        renderer.render(&device, &queue, (width, height), &scene, &Camera::default());
    }

    // relecture des pixels (lignes alignées sur 256 octets)
    let row = (width * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("relecture"), size: (row * height) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(height) } },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    queue.submit(Some(encoder.finish()));
    buffer.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("relecture impossible"));
    device.poll(wgpu::PollType::wait_indefinitely())?;
    let elapsed = start.elapsed();
    let data = buffer.slice(..).get_mapped_range()?;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        let start = (y * row) as usize;
        pixels.extend_from_slice(&data[start..start + (width * 4) as usize]);
    }
    drop(data);

    let file = std::fs::File::create(output)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&pixels)?;
    println!(
        "{} : {} projecteurs, {} volumes, {} lumières — {} ({:?}) — {:.1} ms par image",
        project.name,
        project.fixtures.len(),
        scene.items.len(),
        scene.lights.len(),
        info.name,
        info.backend,
        elapsed.as_secs_f64() * 1000.0 / 61.0
    );
    Ok(())
}
