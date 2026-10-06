// Console de test : un fader par canal du projecteur sélectionné (injecte du DMX dans le simulateur).
import { getFixtureType, getMode } from '/shared/fixtures/index.js';
import { compileMode } from '/shared/engine/dmx.js';
import { linearToSrgb } from '/shared/engine/color.js';
import { h } from './dom.js';

export const CHANNEL_LABELS = {
  dimmer: 'Dimmer',
  red: 'Rouge',
  green: 'Vert',
  blue: 'Bleu',
  white: 'Blanc',
  amber: 'Ambre',
  mint: 'Menthe',
  lime: 'Lime',
  cyan: 'Cyan',
  uv: 'UV',
  cct: 'Kelvin',
  tint: 'Teinte ±V',
  hue: 'Teinte',
  sat: 'Satur.',
  strobe: 'Strobe',
  x: 'x',
  y: 'y',
  effect: 'Effet',
  speed: 'Vitesse',
  none: '—',
};

export function channelLabel(slot) {
  const base = slot.def.label || CHANNEL_LABELS[slot.f] || slot.f;
  return slot.px >= 0 ? `${base} P${slot.px + 1}` : base;
}

export function mountTestConsole({ store, net }) {
  const body = document.querySelector('[data-body="console"]');
  const buffers = new Map();
  const dirty = new Set();
  let timer = null;
  let renderedKey = '';

  const bufferFor = (u) => {
    let b = buffers.get(u);
    if (!b) {
      b = new Uint8Array(512);
      const live = store.dmx.get(u);
      if (live) b.set(live);
      buffers.set(u, b);
    }
    return b;
  };

  const flush = () => {
    timer = null;
    for (const u of dirty) net.send({ t: 'dmx.inject', universe: u, data: Array.from(buffers.get(u)) });
    dirty.clear();
  };
  const schedule = (u) => {
    dirty.add(u);
    if (!timer) timer = setTimeout(flush, 30);
  };

  /** Une application ou une console externe pilote-t-elle déjà cet univers ? */
  const externalSource = (u) => (store.status?.sources || []).find((s) => s.protocol !== 'test' && s.universes.includes(u));

  // Maintien : la console réémet ses valeurs pour ne pas être considérée comme une source perdue,
  // sauf si une application pilote l'univers (elle reprend alors la main au bout de 3 s).
  setInterval(() => {
    for (const [u, b] of buffers) if (b.some((v) => v > 0) && !externalSource(u)) schedule(u);
  }, 1000);

  const release = (u) => {
    buffers.get(u)?.fill(0);
    dirty.delete(u);
    net.send({ t: 'dmx.release', universe: u });
    renderedKey = '';
    render();
  };

  function render() {
    const f = store.selected();
    const type = f && getFixtureType(f.type);
    const mode = type && getMode(type, f.settings.dmxMode);
    const key = f ? `${f.id}|${f.settings.universe}|${f.settings.dmxAddress}|${f.settings.dmxMode}` : '';
    if (key === renderedKey) return;
    renderedKey = key;
    body.replaceChildren();
    if (!f || !mode) {
      body.append(h('div', { class: 'empty' }, 'Sélectionne un projecteur pour afficher ses canaux DMX ici.'));
      return;
    }
    const u = f.settings.universe;
    const start = f.settings.dmxAddress;
    const buf = bufferFor(u);
    const { slots } = compileMode(mode);
    const faders = [];

    const setChannel = (slot, v) => {
      const i = start - 1 + slot.offset;
      if (i >= 512) return;
      buf[i] = v;
      if (slot.bytes === 2 && i + 1 < 512) buf[i + 1] = v;
    };

    const strip = h('div', { class: 'faders' });
    for (const slot of slots) {
      const ch = start + slot.offset;
      if (ch > 512) break;
      const val = h('span', { class: 'val' }, String(buf[ch - 1]));
      const input = h('input', {
        type: 'range',
        min: 0,
        max: 255,
        value: buf[ch - 1],
        oninput: (e) => {
          const v = Number(e.target.value);
          setChannel(slot, v);
          val.textContent = String(v);
          schedule(u);
        },
      });
      faders.push({ slot, input, val });
      strip.append(
        h('div', { class: 'fader', title: `${channelLabel(slot)} — canal ${ch}${slot.bytes === 2 ? ' (16 bits)' : ''}` }, val, input, h('div', { class: 'lbl' }, channelLabel(slot)), h('div', { class: 'ch' }, String(ch))),
      );
    }

    const setAll = (fn) => {
      for (const fd of faders) {
        const v = Math.max(0, Math.min(255, Math.round(fn(fd.slot))));
        setChannel(fd.slot, v);
        fd.input.value = v;
        fd.val.textContent = String(v);
      }
      schedule(u);
    };

    const rainbow = (slot) => {
      if (!['red', 'green', 'blue', 'dimmer'].includes(slot.f)) return slot.f === 'sat' ? 255 : 0;
      if (slot.f === 'dimmer') return 255;
      const n = Math.max(1, type.pixels);
      const hue = ((slot.px < 0 ? 0 : slot.px) / n) * 6;
      // Valeur de chaque primaire aux bornes des 6 secteurs de teinte (0°, 60°, ... 300°)
      const c = { red: [1, 1, 0, 0, 0, 1], green: [0, 1, 1, 1, 0, 0], blue: [0, 0, 0, 1, 1, 1] }[slot.f];
      const k = Math.floor(hue) % 6;
      const frac = hue - Math.floor(hue);
      const a = c[k];
      const b = c[(k + 1) % 6];
      return 255 * linearToSrgb(a + (b - a) * frac);
    };

    body.append(
      h(
        'div',
        { class: 'console-head' },
        h('strong', {}, f.name),
        h('span', { class: 'mono' }, `Univers ${u} · canaux ${start}–${start + compileMode(mode).footprint - 1} · ${mode.name}`),
        h('span', { class: 'spacer' }),
        h('button', { onclick: () => setAll(() => 0) }, 'Tout à 0'),
        h('button', { onclick: () => setAll((s) => (['dimmer', 'red', 'green', 'blue', 'white'].includes(s.f) ? 255 : s.f === 'cct' ? 128 : s.f === 'tint' ? 128 : 0)) }, 'Plein feu'),
        type.pixels > 1 ? h('button', { onclick: () => setAll(rainbow) }, 'Arc-en-ciel') : null,
        h('button', { title: 'Remet la console à zéro sur cet univers et rend la main aux applications', onclick: () => release(u) }, 'Relâcher'),
      ),
      notice,
      strip,
    );
    refreshNotice();
  }

  const notice = h('div', { class: 'note' });
  function refreshNotice() {
    const f = store.selected();
    const ext = f && externalSource(f.settings.universe);
    notice.textContent = ext ? `⚠ ${ext.name || ext.address} (${ext.protocol}) pilote aussi cet univers : les valeurs sont fusionnées (la plus haute l’emporte).` : '';
  }
  store.addEventListener('status', refreshNotice);

  store.addEventListener('selection', render);
  store.addEventListener('fixture-updated', (e) => {
    if (e.detail.fixture.id === store.selectedId) render();
  });
  store.addEventListener('scene', render);
  render();
}
