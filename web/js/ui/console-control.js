// Commande des projecteurs par la console ARKLIGHT (onglet Console, réglages « Canaux » de
// l'inspecteur) : valeurs canal par canal rangées dans l'état du projecteur (state.console),
// appliquées tout de suite ici et envoyées au serveur en lot (une seule étape d'annulation par geste).
import { getFixtureType, getMode } from '/shared/fixtures/index.js';
import { activeConsoleValues, adjustColor, consoleHeld, consoleOptions, lookToValues, seedConsole } from '/shared/engine/console.js';
import { EMITTER_FUNCTIONS, compileMode } from '/shared/engine/dmx.js';

const slotMax = (slot) => (slot.bytes === 2 ? 65535 : 255);

const isObject = (v) => v && typeof v === 'object' && !Array.isArray(v);
function deepAssign(target, patch) {
  for (const [k, v] of Object.entries(patch)) {
    if (isObject(v) && isObject(target[k])) deepAssign(target[k], v);
    else target[k] = v;
  }
  return target;
}

export function createConsoleControl({ store, net, viewport }) {
  const pending = new Map(); // id du projecteur → changements à envoyer
  const colorMemory = new Map(); // id → proportions des couleurs (profils sans gradateur, ramenés à 0)
  const lookMemory = new Map(); // id → { sig, look } : réglage de couleur du dernier geste (sans tout recalculer)
  const flashes = new Map(); // id → valeurs d'avant le flash (null : le projecteur n'était pas piloté)
  let timer = null;

  function flush() {
    clearTimeout(timer);
    timer = null;
    if (!pending.size) return;
    const items = [...pending].map(([id, changes]) => ({ id, changes }));
    pending.clear();
    net.send({ t: 'fixtures.update', items });
  }

  function send(f, changes, immediate) {
    deepAssign(f, structuredClone(changes));
    store.upsertFixture(f, store.clientId);
    const prev = pending.get(f.id);
    pending.set(f.id, prev ? deepAssign(prev, structuredClone(changes)) : structuredClone(changes));
    if (immediate) flush();
    else if (!timer) timer = setTimeout(flush, 40); // ~25 envois par seconde pendant un glisser
  }

  const modeOf = (f) => {
    const type = getFixtureType(f.type);
    return { type, mode: type && getMode(type, f.settings.dmxMode) };
  };

  /** Valeurs de la console pour un projecteur : les siennes s'il est piloté, sinon son réglage actuel. */
  function valuesOf(f, mode = modeOf(f).mode) {
    const type = getFixtureType(f.type);
    if (!type || !mode) return [];
    if (consoleHeld(f)) return [...activeConsoleValues(f, type, mode, {})];
    return seedConsole(f, type, mode, { dmx: store.dmx, source: viewport.outputOf(f.id)?.source });
  }

  /**
   * Modifie les valeurs de la console de chaque projecteur (et le prend en main s'il ne l'était pas).
   * fn(values, fixture, mode, type) change `values` sur place ; false : ne rien écrire.
   */
  function write(fixtures, fn, { immediate = false } = {}) {
    for (const f of fixtures) {
      const { type, mode } = modeOf(f);
      if (!mode) continue;
      const values = valuesOf(f, mode);
      if (fn(values, f, mode, type) === false) continue;
      send(f, { state: { console: { mode: mode.id, values } } }, immediate);
    }
    if (immediate) flush();
  }

  /** Remplace toutes les valeurs par celles d'un réglage (ou d'un réglage par pixel). */
  function writeLook(fixtures, lookOf) {
    write(
      fixtures,
      (values, f, mode, type) => {
        const next = lookToValues(mode, lookOf(f, type, mode), consoleOptions(f, type));
        next.forEach((v, i) => (values[i] = v));
      },
      { immediate: true },
    );
  }

  /**
   * Intensité (0..1) par le gradateur du profil ; sans canal Dimmer, l'intensité passe dans les couleurs,
   * dans les mêmes proportions (la teinte ne change pas, et revient après un passage à zéro).
   */
  function setIntensity(fixtures, level, opts) {
    write(
      fixtures,
      (values, f, mode) => {
        const { slots } = compileMode(mode);
        const dimmers = slots.map((s, i) => (s.f === 'dimmer' ? i : -1)).filter((i) => i >= 0);
        if (dimmers.length) {
          dimmers.forEach((i) => (values[i] = Math.round(level * slotMax(slots[i]))));
          return;
        }
        const emit = slots.map((s, i) => (EMITTER_FUNCTIONS.includes(s.f) ? i : -1)).filter((i) => i >= 0);
        if (!emit.length) return false;
        const top = Math.max(...emit.map((i) => values[i] / slotMax(slots[i])));
        let ratios = top > 0 ? emit.map((i) => values[i] / slotMax(slots[i]) / top) : colorMemory.get(f.id);
        if (top > 0) colorMemory.set(f.id, ratios);
        if (!ratios || ratios.length !== emit.length) ratios = emit.map(() => 1);
        emit.forEach((i, k) => (values[i] = Math.round(ratios[k] * level * slotMax(slots[i]))));
      },
      opts,
    );
  }

  /**
   * Couleur des projecteurs : { cct } ou { tint } (un blanc), { hue } ou { sat } (une couleur), à la même
   * intensité. Pendant un geste (molette, fader MIDI), le réglage de départ est retenu au lieu d'être
   * recalculé à chaque message.
   */
  function setColor(fixtures, change) {
    write(fixtures, (values, f, mode, type) => {
      const sig = values.join(',');
      const mem = lookMemory.get(f.id);
      const res = adjustColor(mode, values, change, { ...consoleOptions(f, type), pixelCount: type.pixels, look: mem?.sig === sig ? mem.look : null });
      res.values.forEach((v, i) => (values[i] = v));
      lookMemory.set(f.id, { sig: values.join(','), look: res.look });
    });
  }

  /** Flash : plein feu tant que la touche est enfoncée, puis retour à l'état d'avant. */
  function flash(fixtures, on) {
    if (on) {
      for (const f of fixtures) if (!flashes.has(f.id)) flashes.set(f.id, consoleHeld(f) ? [...f.state.console.values] : null);
      setIntensity(fixtures, 1, { immediate: true });
      return;
    }
    const back = [];
    for (const [id, prev] of flashes) {
      const f = store.fixtures.get(id);
      if (!f) continue;
      if (prev === null) back.push(f);
      else send(f, { state: { console: { mode: f.state.console?.mode ?? f.settings.dmxMode, values: prev } } }, false);
    }
    flashes.clear();
    release(back);
    flush();
  }

  /** Rend la main : les projecteurs reprennent leurs propres réglages (ou le DMX reçu). */
  function release(fixtures) {
    for (const f of fixtures) if (consoleHeld(f)) send(f, { state: { console: null } }, false);
    flush();
  }

  return {
    valuesOf,
    write,
    writeLook,
    setIntensity,
    setColor,
    flash,
    release,
    flush,
    held: consoleHeld,
    /** Projecteurs de la scène pilotés par la console. */
    heldFixtures: () => (store.scene?.fixtures || []).filter(consoleHeld),
    /** Univers dont le dernier état DMX est conservé alors que plus rien n'arrive. */
    staleUniverses: () => [...store.dmx.keys()].filter((u) => !store.liveUniverses.has(u)).sort((a, b) => a - b),
    /** « Reprendre la main » sur le DMX conservé : les projecteurs en auto reviennent à leurs réglages. */
    forgetDmx: (universes) => net.send({ t: 'dmx.forget', universes }),
  };
}
