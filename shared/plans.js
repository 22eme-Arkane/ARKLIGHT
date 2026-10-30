// Séquences et plans d'un projet : un projet contient des séquences, chaque séquence ses plans, et
// chaque plan son propre plan de feu (projecteurs, grip, décor, annotations). On passe d'un plan à
// l'autre sans quitter le projet : un même décor peut avoir un plan de feu pour le large, un autre
// pour le contrechamp…
// Résumé envoyé aux interfaces : { current: { seq, plan }, sequences: [{ id, name, plans: [{ id, name, thumbnail, fixtures, objects }] }] }

export const MAX_PLAN_NAME = 60;

/** Nom de séquence ou de plan : une ligne, sans espaces superflus. */
export const cleanPlanName = (v) =>
  String(v ?? '')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, MAX_PLAN_NAME);

/** Noms par défaut, dans la langue du projet. */
export const defaultSequenceName = (n, lang = 'fr') => (lang === 'en' ? `Sequence ${n}` : `Séquence ${n}`);
export const defaultPlanName = (n, lang = 'fr') => (lang === 'en' ? `Shot ${n}` : `Plan ${n}`);

/** Prochain numéro libre après les noms « Plan 3 », « Séquence 2 »… (ou après le nombre d'éléments). */
export function nextNumber(names, count) {
  let n = count;
  for (const name of names) {
    const m = /^(?:Plan|Shot|Séquence|Sequence|Séq\.|Seq\.)\s*(\d+)\b/i.exec(String(name || ''));
    if (m) n = Math.max(n, Number(m[1]));
  }
  return n + 1;
}

/** Tous les plans dans l'ordre du projet : [{ seq, plan }]. */
export function flatPlans(info) {
  return (info?.sequences || []).flatMap((seq) => (seq.plans || []).map((plan) => ({ seq, plan })));
}

/** Plan courant et sa séquence ({ seq, plan }), ou null. */
export function currentPlan(info) {
  return flatPlans(info).find((x) => x.plan.id === info?.current?.plan) || null;
}

/** Plan voisin dans l'ordre du projet (dir = 1 : suivant, -1 : précédent), ou null. */
export function neighborPlan(info, dir) {
  const all = flatPlans(info);
  const i = all.findIndex((x) => x.plan.id === info?.current?.plan);
  return i < 0 ? null : all[i + dir] || null;
}

/** Nombre total de plans. */
export const planCount = (info) => flatPlans(info).length;
