// Console ARKLIGHT (onglet du bas) : pilote les projecteurs sélectionnés — un projecteur ou tout un
// groupe — canal par canal selon leur profil DMX, en partant de ce qu'ils font déjà. Les autres
// projecteurs gardent leurs réglages ; « Relâcher » leur rend la main. Voir shared/engine/console.js.
import { getFixtureType, getMode, modeFootprint } from '/shared/fixtures/index.js';
import { compileMode } from '/shared/engine/dmx.js';
import { modePixels } from '/shared/engine/console.js';
import { t } from '../i18n.js';
import { h } from './dom.js';
import { patchIssues } from './patch.js';
import { channelLabel, faderText, functionLabel, rawText, shortLabel } from './channels.js';

export { CHANNEL_LABELS, channelLabel } from './channels.js';

const RAW_KEY = 'arklight.console.raw';
const slotMax = (slot) => (slot.bytes === 2 ? 65535 : 255);
const loadRaw = () => {
  try {
    return localStorage.getItem(RAW_KEY) === '1';
  } catch {
    return false;
  }
};

export function mountTestConsole({ store, net, control }) {
  const body = document.querySelector('[data-body="console"]');
  let raw = loadRaw();
  let familyKey = null; // famille affichée (modèle + profil) quand la sélection en mélange plusieurs
  let renderedKey = '';
  let current = null; // { key, type, mode, fixtures }
  let faders = []; // { indices, slot, input, val }
  let note = null;
  let queued = false;
  let fams = []; // familles affichées (modèle + profil)
  let midiMarks = { marks: [], page: 0 }; // faders commandés par le contrôleur MIDI : [{ index (1 = le premier), label }]
  /** Projecteurs de la famille affichée, tels qu'ils sont maintenant dans la scène. */
  const live = () => (current ? current.fixtures.map((f) => store.fixtures.get(f.id)).filter(Boolean) : []);

  /** Projecteurs sélectionnés regroupés par modèle et profil DMX (une console par famille). */
  function families() {
    const map = new Map();
    for (const f of store.selectedFixtures()) {
      const type = getFixtureType(f.type);
      const mode = type?.pixels ? getMode(type, f.settings.dmxMode) : null;
      if (!mode) continue;
      const key = `${f.type}|${mode.id}`;
      if (!map.has(key)) map.set(key, { key, type, mode, fixtures: [] });
      map.get(key).fixtures.push(f);
    }
    const list = [...map.values()];
    const primary = store.selected();
    // la famille du projecteur principal d'abord
    list.sort((a, b) => b.fixtures.includes(primary) - a.fixtures.includes(primary));
    return list;
  }

  /** Nom du groupe quand la sélection est exactement un groupe. */
  function groupName() {
    const ids = store.selectedIds();
    if (ids.length < 2) return null;
    const g = (store.scene?.groups || []).find((x) => {
      const members = x.ids.filter((id) => store.fixtures.has(id));
      return members.length === ids.length && members.every((id) => store.isSelected(id));
    });
    return g?.name || null;
  }

  const structureKey = (list) => [store.selectedIds().join(','), list.map((f) => f.key).join(';'), familyKey, raw].join('|');

  function schedule() {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => {
      queued = false;
      const list = families();
      if (structureKey(list) !== renderedKey) render(list);
      else refresh();
    });
  }

  // ------------------------------------------------------------------ affichage
  function render(list = families()) {
    fams = list;
    renderedKey = structureKey(fams);
    faders = [];
    body.replaceChildren();
    const selected = store.selectedFixtures();
    if (!fams.length) {
      current = null;
      note = h('div', { class: 'console-note' });
      body.append(
        h(
          'div',
          { class: 'empty console-empty' },
          selected.length ? 'Ces projecteurs n’ont pas de profil DMX.' : 'Sélectionne un projecteur ou un groupe (liste Scène, vue 3D) : ses canaux s’affichent ici, et la console le pilote dès que tu bouges un fader. Les autres projecteurs ne changent pas.',
        ),
        note,
      );
      refreshNote();
      return;
    }
    current = fams.find((f) => f.key === familyKey) || fams[0];
    const { type, mode, fixtures } = current;
    const { slots } = compileMode(mode);
    const single = fixtures.length === 1 ? fixtures[0] : null;
    const pixels = modePixels(mode, type.pixels);

    // ---- en-tête : qui est piloté, profil, actions
    const title = single && selected.length === 1 ? single.name : groupName() || t('{0} projecteurs', { 0: selected.length });
    const profile = h(
      'select',
      { class: 'console-profile', title: 'Profil DMX (mode) de ces projecteurs', onchange: (e) => setProfile(e.target.value) },
      (type.dmx?.modes || []).map((m) => h('option', { value: m.id, selected: m.id === mode.id }, `${m.label || m.name} — ${modeFootprint(type, m.id)} c.`)),
    );
    const where = single ? t('Univers {0} · canaux {1}–{2}', { 0: single.settings.universe, 1: single.settings.dmxAddress, 2: single.settings.dmxAddress + modeFootprint(type, mode.id) - 1 }) : null;
    const btn = (label, title, onclick, cls = '') => h('button', { class: cls, title, onclick }, label);
    const setDimmers = (v) => control.setIntensity(live(), v, { immediate: true });
    const held = control.heldFixtures();
    // plusieurs modèles ou profils dans la sélection : une famille à la fois
    const famChips =
      fams.length > 1
        ? h(
            'span',
            { class: 'console-fams' },
            fams.map((f) =>
              h(
                'button',
                {
                  class: f === current ? 'active' : '',
                  title: `${f.type.brand} ${f.type.name} — ${f.mode.label || f.mode.name}`,
                  onclick: () => {
                    familyKey = f.key;
                    schedule();
                  },
                },
                `${f.type.name} ×${f.fixtures.length}`,
              ),
            ),
          )
        : null;
    body.append(
      h(
        'div',
        { class: 'console-head' },
        h('strong', { translate: 'no' }, title),
        famChips || h('span', { class: 'console-sub' }, `${fixtures.length} × ${type.brand} ${type.name}`),
        profile,
        where ? h('span', { class: 'mono console-where' }, where) : null,
        h('span', { class: 'spacer' }),
        btn('Plein feu', 'Intensité au maximum (la couleur ne change pas)', () => setDimmers(1)),
        btn('Noir', 'Intensité à zéro', () => setDimmers(0)),
        pixels > 1 ? btn('Arc-en-ciel', 'Une couleur par pixel', () => control.writeLook(live(), (f, ty, m) => Array.from({ length: modePixels(m, ty.pixels) }, (_, p) => ({ level: 1, hue: (360 * p) / modePixels(m, ty.pixels), sat: 1 })))) : null,
        btn('Relâcher', 'Rend la main aux projecteurs sélectionnés : ils reprennent leurs propres réglages', () => control.release(store.selectedFixtures())),
        held.length ? btn(t('Tout relâcher ({0})', { 0: held.length }), 'Rend la main à tous les projecteurs pilotés par la console', () => control.release(control.heldFixtures())) : null,
        btn(
          '0–255',
          'Afficher les valeurs DMX brutes (0–255) plutôt qu’en %, K ou °',
          () => {
            raw = !raw;
            try {
              localStorage.setItem(RAW_KEY, raw ? '1' : '0');
            } catch {
              /* stockage indisponible : choix gardé pour cette session */
            }
            schedule();
          },
          `console-raw${raw ? ' active' : ''}`,
        ),
      ),
    );

    note = h('div', { class: 'console-note' });
    body.append(note);

    // ---- faders : d'abord « tous les pixels » par fonction, puis un par canal
    const strip = h('div', { class: 'faders' });
    const start = single?.settings.dmxAddress;
    if (pixels > 1) {
      const byKey = new Map();
      slots.forEach((s, i) => {
        if (s.px < 0 || s.f === 'none') return;
        if (!byKey.has(s.key)) byKey.set(s.key, []);
        byKey.get(s.key).push(i);
      });
      for (const indices of byKey.values()) {
        const s = slots[indices[0]];
        const base = functionLabel(s);
        strip.append(fader(indices, s, t('{0} (tous)', { 0: base }), t('{0} : tous les pixels', { 0: base }), 'tous', true));
      }
      strip.append(h('span', { class: 'fader-sep' }));
    }
    slots.forEach((s, i) => {
      const ch = single ? start + s.offset : null;
      const full = `${channelLabel(s)} — ${single ? t('canal {0}', { 0: ch }) : t('canal {0} du profil', { 0: s.offset + 1 })}${s.bytes === 2 ? ' (16 bits)' : ''}`;
      strip.append(fader([i], s, shortLabel(s), full, single ? String(ch) : `c${s.offset + 1}`));
    });
    body.append(strip);
    applyMidiMarks();
    refresh();
  }

  /** Un fader : un canal (ou le même canal de tous les pixels). */
  function fader(indices, slot, label, title, chText, group = false) {
    const val = h('span', { class: 'val' });
    const input = h('input', {
      type: 'range',
      min: 0,
      max: slotMax(slot),
      step: 1,
      'aria-label': label,
      oninput: () => {
        const v = Number(input.value);
        val.textContent = (raw ? rawText : faderText)(slot, v);
        control.write(live(), (values) => indices.forEach((i) => (values[i] = v)));
      },
      onchange: () => control.flush(),
    });
    const el = h('div', { class: `fader${group ? ' group' : ''}`, title }, val, input, h('div', { class: 'lbl' }, label), h('div', { class: 'ch' }, chText));
    faders.push({ indices, slot, input, val, el });
    return el;
  }

  /** Repères « M1, M2… » sur les faders que le contrôleur MIDI commande (page en cours). */
  function applyMidiMarks() {
    faders.forEach((fd) => {
      fd.el.classList.remove('midi');
      delete fd.el.dataset.midi;
    });
    midiMarks.marks.forEach(({ index, label }, k) => {
      const fd = faders[index - 1];
      if (!fd) return;
      fd.el.classList.add('midi');
      fd.el.dataset.midi = label || `M${k + 1}`;
    });
  }

  /** Valeurs affichées : celles du premier projecteur piloté de la famille, sinon de son réglage actuel. */
  function refresh() {
    if (!current) return refreshNote();
    const fixtures = live();
    const ref = fixtures.find(control.held) || fixtures[0];
    if (ref) {
      const values = control.valuesOf(ref, current.mode);
      for (const fd of faders) {
        const v = values[fd.indices[0]] ?? 0;
        if (document.activeElement !== fd.input) fd.input.value = v;
        fd.val.textContent = (raw ? rawText : faderText)(fd.slot, Number(fd.input.value));
      }
    }
    refreshNote();
  }

  /** Ligne d'état : qui est piloté, DMX conservé sans signal, adresses qui se chevauchent. */
  function refreshNote() {
    if (!note) return;
    const parts = [];
    if (current) {
      const fixtures = live();
      const n = fixtures.filter(control.held).length;
      if (fixtures.length === 1) parts.push(h('span', {}, n ? 'Piloté par la console.' : 'Pas encore piloté : la console part de son réglage actuel. Bouge un fader pour en prendre la main.'));
      else parts.push(h('span', {}, n ? t('Pilotés par la console : {0} sur {1}.', { 0: n, 1: fixtures.length }) : 'Pas encore pilotés : la console part de leur réglage actuel. Bouge un fader pour en prendre la main (les autres projecteurs ne changent pas).'));
      const shown = midiMarks.marks.map((m) => m.index).filter((i) => i <= faders.length);
      if (shown.length) parts.push(h('span', { class: 'console-midi' }, t('MIDI : faders {0} à {1} de la console', { 0: shown[0], 1: shown[shown.length - 1] })));
      const issues = fixtures.flatMap((f) => patchIssues(f, store.scene.fixtures));
      if (issues.length) parts.push(h('span', { class: 'warnbox' }, `⚠ ${issues[0]}${issues.length > 1 ? ' …' : ''}`));
    }
    const stale = control.staleUniverses().filter((u) => (store.scene?.fixtures || []).some((f) => f.settings.universe === u && (f.settings.source || 'auto') !== 'manual'));
    if (stale.length)
      parts.push(
        h(
          'span',
          { class: 'warnbox' },
          t('⚠ Univers {0} : dernier DMX reçu conservé (plus de signal), les projecteurs en « Auto » le suivent.', { 0: stale.join(', ') }),
          ' ',
          h('button', { class: 'linkish', title: 'Oublie ce DMX : les projecteurs reprennent leurs réglages, comme rallumés', onclick: () => control.forgetDmx(stale) }, 'Reprendre la main'),
        ),
      );
    note.replaceChildren(...parts);
  }

  function setProfile(modeId) {
    if (!current) return;
    const items = [];
    for (const f of live()) {
      f.settings.dmxMode = modeId;
      store.upsertFixture(f, store.clientId);
      items.push({ id: f.id, changes: { settings: { dmxMode: modeId } } });
    }
    net.send({ t: 'fixtures.update', items });
    familyKey = `${current.type.id}|${modeId}`;
    schedule();
  }

  store.addEventListener('selection', schedule);
  store.addEventListener('scene', schedule);
  store.addEventListener('groups', schedule);
  store.addEventListener('fixture-updated', (e) => {
    if (store.isSelected(e.detail.fixture.id) || (current && current.fixtures.some((f) => f.id === e.detail.fixture.id))) schedule();
  });
  store.addEventListener('status', refreshNote);
  render();

  return {
    /** Nombre de faders affichés (sélection en cours). */
    faderCount: () => faders.length,
    /** Faders commandés par le contrôleur MIDI (repères M1, M2…). */
    setMidiFaders(marks, page = 0) {
      midiMarks = { marks: marks.map((m) => (typeof m === 'number' ? { index: m, label: null } : m)), page };
      applyMidiMarks();
      refreshNote();
    },
    /** Groupe de plusieurs modèles : la console passe au modèle suivant. */
    nextFamily() {
      if (fams.length < 2) return;
      const i = fams.findIndex((f) => f === current);
      familyKey = fams[(i + 1) % fams.length].key;
      schedule();
    },
    /** Fader n° index (1 = le premier à gauche) à la valeur 0..1, comme si on le bougeait (contrôleur MIDI). */
    setFader(index, value) {
      const fd = faders[index - 1];
      if (!fd || !current) return false;
      fd.input.value = String(Math.round(Math.max(0, Math.min(1, value)) * slotMax(fd.slot)));
      fd.input.dispatchEvent(new Event('input'));
      return true;
    },
  };
}
