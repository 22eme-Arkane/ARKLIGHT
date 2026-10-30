// Fenêtre « Contrôleur MIDI » : appareils branchés, dernier message reçu, correspondances. On apprend à
// ARKLIGHT ce que fait chaque fader ou touche : on bouge le fader du contrôleur, on choisit sa fonction.
// Des assistants apprennent à la suite les faders de la console, les groupes ou les projecteurs.
// Préréglages : consoles connues (shared/midi-presets.js) et préréglages enregistrés, exportables.
import { COMMANDS } from '../commands.js';
import { TARGET_GROUPS, TARGET_KINDS, describeSource, isContinuous, kindOf, sourceKey } from '/shared/midi.js';
import { MIDI_PRESETS, presetById, presetLabel } from '/shared/midi-presets.js';
import { t } from '../i18n.js';
import { askConfirm, askText } from './dialogs.js';
import { h, toast } from './dom.js';

const MAX_FADERS = 32;
const MAX_KEYS = 12; // touches apprises à la suite (les 12 touches « projecteur » d'une DMX Operator 384)

/** Une touche appuyée (note jouée, programme, CC qui passe au-dessus de la moitié). */
const isPress = (msg) => (msg.type === 'note' && msg.on) || msg.type === 'program' || (msg.type === 'cc' && msg.on);

export function createMidiPanel({ midi, store, fixtureOrder }) {
  const dlg = h('dialog', { class: 'dialog midi' });
  document.body.append(dlg);
  let learning = null; // 'new', id d'une correspondance, ou 'wizard'
  let wizard = null; // { kind: 'faders' | 'groups' | 'fixtures', items, next, keys }
  let renderedKey = '';

  const devices = h('div', { class: 'midi-devices' });
  const last = h('div', { class: 'midi-last mono' });
  const pageInfo = h('div', { class: 'midi-page' });
  const status = h('div', { class: 'midi-status' });
  const table = h('div', { class: 'midi-table' });
  const learnBtn = h('button', { class: 'primary', onclick: () => (learning === 'new' ? stopLearning() : startLearn('new')) });
  const wizardBtns = {
    faders: h('button', { title: 'Bouge les faders du contrôleur l’un après l’autre : ils commandent les faders 1 à 8 de la console', onclick: () => toggleWizard('faders') }),
    groups: h('button', { title: 'Appuie sur des touches du contrôleur l’une après l’autre : chacune sélectionne un groupe, dans l’ordre de la liste Scène', onclick: () => toggleWizard('groups') }),
    fixtures: h('button', { title: 'Appuie sur des touches l’une après l’autre : chacune sélectionne un projecteur, dans l’ordre de la liste Scène (12 au plus)', onclick: () => toggleWizard('fixtures') }),
  };
  const WIZARD_LABELS = { faders: 'Faders 1 → 8 de la console', groups: 'Touches → groupes', fixtures: 'Touches → projecteurs' };

  // ---- préréglage : console connue ou préréglage enregistré
  const presetSel = h('select', { class: 'midi-preset-sel', onchange: (e) => midi.setPreset(e.target.value || null) });
  const channelSel = h(
    'select',
    { title: 'Premier canal MIDI de la console (réglable sur la DMX Operator 384 : Midi/Bkc maintenu + UP/DOWN)', onchange: (e) => midi.setPreset(midi.state.preset?.id, { channel: Number(e.target.value) }) },
    Array.from({ length: 14 }, (_, i) => h('option', { value: i + 1 }, t('canal {0}', { 0: i + 1 }))),
  );
  const deletePresetBtn = h('button', { class: 'ib', title: 'Supprimer ce préréglage enregistré', onclick: () => deleteUserPreset() }, '🗑');
  const presetInfo = h('div', { class: 'midi-preset-info' });
  const suggest = h('div', { class: 'midi-suggest hidden' });
  const importInput = h('input', { type: 'file', accept: '.json,application/json', class: 'hidden', onchange: (e) => importFile(e.target.files[0]) });
  const presetBox = h(
    'div',
    { class: 'midi-preset' },
    h('div', { class: 'midi-preset-row' }, h('span', { class: 'midi-k' }, 'Préréglage'), presetSel, channelSel, deletePresetBtn),
    suggest,
    presetInfo,
    h(
      'div',
      { class: 'midi-preset-tools' },
      h('button', { title: 'Garde l’ensemble en cours (préréglage et commandes apprises) sous un nom, pour le retrouver ou le partager', onclick: () => saveUserPreset() }, 'Enregistrer comme préréglage…'),
      h('button', { title: 'Fichier à partager avec un autre poste ou un autre utilisateur de la même console', onclick: () => exportFile() }, 'Exporter…'),
      h('button', { onclick: () => importInput.click() }, 'Importer…'),
      importInput,
    ),
  );

  dlg.append(
    h('h3', {}, 'Contrôleur MIDI'),
    h('div', { class: 'note' }, 'Branche le contrôleur en USB (DMX Operator 384 : interrupteur à l’arrière sur MIDI), puis apprends à ARKLIGHT ce que fait chaque fader ou touche. Ces réglages restent sur ce PC.'),
    h('div', { class: 'midi-info' }, h('span', { class: 'midi-k' }, 'Appareils'), devices, h('span', { class: 'midi-k' }, 'Reçu'), last),
    presetBox,
    h('h4', { class: 'midi-own-title' }, 'Commandes apprises'),
    pageInfo,
    h('div', { class: 'midi-tools' }, learnBtn, h('span', { class: 'midi-k' }, 'À la suite :'), wizardBtns.faders, wizardBtns.groups, wizardBtns.fixtures, h('span', { class: 'spacer' }), h('button', { onclick: () => clearAll() }, 'Tout effacer')),
    status,
    table,
    h('div', { class: 'dialog-actions' }, h('button', { onclick: () => dlg.close() }, 'Fermer')),
  );
  dlg.addEventListener('close', () => stopLearning());

  // ------------------------------------------------------------------ apprentissage
  function stopLearning() {
    learning = null;
    wizard = null;
    midi.learn(null);
    refresh();
  }

  /** Fonction proposée pour une commande qu'on vient d'apprendre. */
  function defaultTarget(source) {
    const used = midi.state.mappings;
    if (isContinuous(source)) {
      const next = Math.max(0, ...used.filter((m) => m.target.kind === 'fader').map((m) => m.target.index || 0)) + 1;
      return next <= MAX_FADERS ? { kind: 'fader', index: next } : { kind: 'selectionLevel' };
    }
    const groups = store.scene?.groups || [];
    const free = groups.find((g) => !used.some((m) => m.target.kind === 'group' && m.target.name === g.name));
    return free ? { kind: 'group', name: free.name } : { kind: 'command', id: 'console.full' };
  }

  function startLearn(id) {
    learning = id;
    midi.learn((source) => {
      if (id === 'new') midi.add({ source, target: defaultTarget(source) });
      else midi.update(id, { source });
      stopLearning();
    });
    refresh();
  }

  /** Remplace ce qui commandait déjà la même chose, ou ce que faisait déjà ce fader / cette touche. */
  function assign(source, target) {
    const key = sourceKey(source);
    const same = (m) => m.target.kind === target.kind && (target.kind === 'fader' ? m.target.index === target.index : m.target.name === target.name);
    for (const m of midi.state.mappings.filter((x) => (x.source && sourceKey(x.source) === key) || same(x))) midi.remove(m.id);
    midi.add({ source, target });
  }

  function toggleWizard(kind) {
    if (learning === 'wizard' && wizard?.kind === kind) return stopLearning();
    const items =
      kind === 'faders'
        ? Array.from({ length: 8 }, (_, i) => i + 1)
        : kind === 'groups'
          ? (store.scene?.groups || []).map((g) => g.name)
          : (fixtureOrder?.() || []).map((id) => store.fixtures.get(id)?.name).filter(Boolean).slice(0, MAX_KEYS);
    if (!items.length) return;
    learning = 'wizard';
    wizard = { kind, items, next: 0, keys: new Set() };
    const step = () => {
      midi.learn(
        (source) => {
          wizard.keys.add(sourceKey(source));
          const item = wizard.items[wizard.next];
          assign(source, kind === 'faders' ? { kind: 'fader', index: item } : { kind: kind === 'groups' ? 'group' : 'fixture', name: item });
          wizard.next++;
          if (wizard.next >= wizard.items.length) stopLearning();
          else step();
        },
        (msg) => !wizard.keys.has(sourceKey(msg)) && (kind === 'faders' ? isContinuous(msg) : isPress(msg)),
      );
      refresh();
    };
    step();
  }

  async function clearAll() {
    if (!midi.state.mappings.length) return;
    if (await askConfirm({ title: 'Effacer toutes les correspondances MIDI ?', ok: 'Effacer', danger: true })) midi.clear();
  }

  // ------------------------------------------------------------------ préréglages
  async function saveUserPreset() {
    if (!midi.all().length) return toast(t('Rien à enregistrer : charge un préréglage ou apprends des commandes'), 'error');
    const cur = midi.activePreset();
    const name = await askText({ title: 'Enregistrer comme préréglage', label: 'Nom (la console ou le contrôleur, l’usage…)', value: cur ? (cur.build ? presetLabel(cur) : cur.name) : '', ok: 'Enregistrer' });
    if (!name) return;
    midi.saveUserPreset(name);
    toast(t('Préréglage enregistré : {0}', { 0: name }));
  }

  async function deleteUserPreset() {
    const p = midi.activePreset();
    if (!p || p.build) return;
    if (await askConfirm({ title: t('Supprimer le préréglage « {0} » ?', { 0: p.name }), ok: 'Supprimer', danger: true })) midi.deleteUserPreset(p.id);
  }

  function exportFile() {
    if (!midi.all().length) return toast(t('Rien à exporter : charge un préréglage ou apprends des commandes'), 'error');
    const cur = midi.activePreset();
    const name = cur ? (cur.build ? presetLabel(cur) : cur.name) : t('Mon contrôleur MIDI');
    const blob = new Blob([JSON.stringify(midi.exportPreset(name), null, 1)], { type: 'application/json' });
    const a = h('a', { href: URL.createObjectURL(blob), download: `${name.replace(/[\\/:*?"<>|]+/g, '-')}.arkmidi.json` });
    document.body.append(a);
    a.click();
    setTimeout(() => {
      URL.revokeObjectURL(a.href);
      a.remove();
    }, 1000);
  }

  async function importFile(file) {
    importInput.value = '';
    if (!file) return;
    try {
      const name = midi.importPreset(JSON.parse(await file.text()));
      toast(t('Préréglage importé : {0}', { 0: name }));
    } catch (e) {
      toast(t(e.message || 'Fichier illisible'), 'error', 4000);
    }
  }

  function refreshPreset() {
    const s = midi.state;
    const cur = midi.activePreset();
    const brands = [...new Set(MIDI_PRESETS.map((p) => p.brand))];
    presetSel.replaceChildren(
      h('option', { value: '', selected: !cur }, t('— Aucun —')),
      ...brands.map((b) => h('optgroup', { label: b }, MIDI_PRESETS.filter((p) => p.brand === b).map((p) => h('option', { value: p.id, selected: cur?.id === p.id }, presetLabel(p))))),
      s.userPresets.length ? h('optgroup', { label: t('Mes préréglages') }, s.userPresets.map((p) => h('option', { value: p.id, selected: cur?.id === p.id, translate: 'no' }, p.name))) : null,
    );
    channelSel.classList.toggle('hidden', !cur?.build);
    channelSel.value = String(s.preset?.channel || 1);
    deletePresetBtn.classList.toggle('hidden', !cur || !!cur.build);
    const sug = s.suggest && presetById(s.suggest);
    suggest.classList.toggle('hidden', !sug);
    suggest.replaceChildren(
      ...(sug ? [h('span', {}, t('{0} détectée.', { 0: sug.name })), ' ', h('button', { class: 'primary', onclick: () => midi.setPreset(sug.id) }, 'Charger son préréglage')] : []),
    );
    presetInfo.replaceChildren(
      ...(cur
        ? [
            h('ul', { class: 'midi-summary' }, (cur.summary || [t('{0} commandes', { 0: midi.layer().length })]).map((line) => h('li', {}, t(line)))),
            cur.source ? h('div', { class: 'note' }, t('Source : {0}', { 0: t(cur.source) })) : null,
            h('div', { class: 'note' }, 'Les commandes que tu apprends (ci-dessous) passent avant celles du préréglage.'),
          ]
        : [h('div', { class: 'note' }, 'Aucun préréglage : apprends les commandes une à une, ou choisis ta console dans la liste.')]),
    );
  }

  // ------------------------------------------------------------------ affichage
  const option = (value, label, selected) => h('option', { value, selected }, label);
  const NO_PARAM = {
    selectionLevel: 'les projecteurs sélectionnés',
    selCct: 'les projecteurs sélectionnés',
    selTint: 'les projecteurs sélectionnés',
    selHue: 'les projecteurs sélectionnés',
    selSat: 'les projecteurs sélectionnés',
    flash: 'les projecteurs sélectionnés',
    blackout: 'tout dans le noir, les réglages ne bougent pas',
    pageNext: 'faders du contrôleur → canaux suivants de la console',
    pagePrev: 'faders du contrôleur → canaux précédents de la console',
    familyNext: 'onglet Console',
    groupNext: 'dans l’ordre de la liste Scène',
    groupPrev: 'dans l’ordre de la liste Scène',
    fixtureNext: 'dans l’ordre de la liste Scène',
    fixturePrev: 'dans l’ordre de la liste Scène',
  };

  /** Réglage de la fonction choisie (fader n°, groupe, projecteur, commande). */
  function paramControl(m) {
    const tg = m.target;
    const set = (patch) => midi.update(m.id, { target: { ...tg, ...patch } });
    if (tg.kind === 'fader')
      return h(
        'select',
        { onchange: (e) => set({ index: Number(e.target.value) }) },
        Array.from({ length: Math.max(MAX_FADERS, tg.index || 0) }, (_, i) => option(i + 1, t('Fader {0}', { 0: i + 1 }), tg.index === i + 1)),
      );
    if (['fixtureIndex', 'groupIndex', 'planIndex', 'fixtureLatch', 'groupLatch'].includes(tg.kind))
      return h(
        'select',
        { onchange: (e) => set({ index: Number(e.target.value) }) },
        Array.from({ length: 48 }, (_, i) => option(i + 1, t('n° {0}', { 0: i + 1 }), (tg.index || 1) === i + 1)),
      );
    if (tg.kind === 'group' || tg.kind === 'groupLevel' || tg.kind === 'fixture' || tg.kind === 'fixtureLevel') {
      const names = tg.kind.startsWith('group') ? (store.scene?.groups || []).map((g) => g.name) : (store.scene?.fixtures || []).map((f) => f.name);
      if (tg.name && !names.includes(tg.name)) names.unshift(tg.name);
      return h('select', { translate: 'no', onchange: (e) => set({ name: e.target.value }) }, names.length ? names.map((n) => option(n, n, n === tg.name)) : option('', t('aucun'), true));
    }
    if (tg.kind === 'command') {
      const sections = [...new Set(COMMANDS.map((c) => c.section))];
      return h(
        'select',
        { onchange: (e) => set({ id: e.target.value }) },
        sections.map((s) => h('optgroup', { label: s }, COMMANDS.filter((c) => c.section === s && c.id !== 'midi.open').map((c) => option(c.id, c.label, c.id === tg.id)))),
      );
    }
    return h('span', { class: 'note midi-param-note' }, NO_PARAM[tg.kind] ? t(NO_PARAM[tg.kind]) : '');
  }

  function kindSelect(m) {
    return h(
      'select',
      {
        onchange: (e) => {
          const kind = e.target.value;
          const groups = store.scene?.groups || [];
          const fixtureName = store.selected()?.name || store.scene?.fixtures[0]?.name || '';
          const target =
            kind === 'fader'
              ? { kind, index: m.target.index || 1 }
              : kind === 'group' || kind === 'groupLevel'
                ? { kind, name: m.target.name && groups.some((g) => g.name === m.target.name) ? m.target.name : groups[0]?.name || '' }
                : kind === 'fixture' || kind === 'fixtureLevel'
                  ? { kind, name: fixtureName }
                  : kind === 'command'
                    ? { kind, id: 'console.full' }
                    : ['fixtureIndex', 'groupIndex', 'planIndex', 'fixtureLatch', 'groupLatch'].includes(kind)
                      ? { kind, index: 1 }
                      : { kind };
          midi.update(m.id, { target });
        },
      },
      TARGET_GROUPS.map((g) =>
        h(
          'optgroup',
          { label: t(g) },
          TARGET_KINDS.filter((k) => k.group === g).map((k) => option(k.id, t(k.label), k.id === m.target.kind)),
        ),
      ),
    );
  }

  const listKey = () => JSON.stringify([midi.state.mappings, midi.state.preset, learning, (store.scene?.groups || []).map((g) => g.name), (store.scene?.fixtures || []).length]);

  function renderTable() {
    const maps = midi.state.mappings;
    renderedKey = listKey();
    if (!maps.length) {
      table.replaceChildren(
        h(
          'div',
          { class: 'empty' },
          midi.activePreset()
            ? 'Aucune commande apprise : le préréglage fait tout. « Apprendre une commande » pour en changer une.'
            : 'Aucune correspondance. « Apprendre une commande », puis bouge un fader ou appuie sur une touche du contrôleur.',
        ),
      );
      return;
    }
    table.replaceChildren(
      ...maps.map((m) =>
        h(
          'div',
          { class: `midi-row${learning === m.id ? ' learning' : ''}`, 'data-id': m.id },
          h('span', { class: 'midi-src mono', title: 'Commande du contrôleur' }, learning === m.id ? t('Bouge un fader ou appuie sur une touche…') : describeSource(m.source, t) || '—'),
          h('button', { class: 'ib', title: 'Réapprendre : bouge le fader ou la touche voulue', onclick: () => (learning === m.id ? stopLearning() : startLearn(m.id)) }, learning === m.id ? 'Annuler' : 'Apprendre'),
          h('span', { class: 'midi-arrow', title: { continuous: 'suit le fader', latch: 'suit la touche (allumée / éteinte)', momentary: 'tant qu’on appuie' }[kindOf(m.target.kind)?.mode] || 'à l’appui' }, '→'),
          kindSelect(m),
          paramControl(m),
          h('button', { class: 'ib', title: 'Supprimer', onclick: () => midi.remove(m.id) }, '🗑'),
        ),
      ),
    );
  }

  function refresh() {
    const s = midi.state;
    devices.replaceChildren(
      !s.supported
        ? h('span', { class: 'warnbox' }, 'MIDI indisponible dans ce navigateur (utilise l’application ARKLIGHT, Chrome ou Edge).')
        : s.error
          ? h('span', { class: 'warnbox' }, t('MIDI refusé : {0}', { 0: s.error }))
          : s.inputs.length
            ? s.inputs.map((d) => h('span', { class: 'midi-dev', translate: 'no' }, h('i'), d.name))
            : h('span', { class: 'note' }, 'Aucun contrôleur MIDI détecté. Branche-le en USB (et passe la DMX Operator 384 en mode MIDI).'),
    );
    last.textContent = s.last ? `${describeSource(s.last.msg, t)} = ${Math.round(s.last.msg.value * 127)}${s.last.device ? ` — ${s.last.device}` : ''}` : '—';
    const idx = midi.faderIndexes();
    pageInfo.textContent = idx.length && idx.length <= MAX_FADERS && !midi.activePreset() ? t('Faders du contrôleur → faders {0} à {1} de la console (page {2})', { 0: idx[0], 1: idx[idx.length - 1], 2: s.page + 1 }) : '';
    learnBtn.textContent = learning === 'new' ? t('Annuler') : t('＋ Apprendre une commande');
    for (const [kind, btn] of Object.entries(wizardBtns)) {
      btn.textContent = learning === 'wizard' && wizard?.kind === kind ? t('Arrêter') : t(WIZARD_LABELS[kind]);
      btn.disabled = (kind === 'groups' && !(store.scene?.groups || []).length) || (kind === 'fixtures' && !(store.scene?.fixtures || []).length);
    }
    const item = wizard?.items[wizard.next];
    status.textContent =
      learning === 'new'
        ? t('Bouge un fader ou appuie sur une touche du contrôleur…')
        : learning === 'wizard' && wizard.kind === 'faders'
          ? t('Bouge le fader du contrôleur qui doit commander le fader {0} de la console…', { 0: item })
          : learning === 'wizard' && wizard.kind === 'groups'
            ? t('Appuie sur la touche du contrôleur pour le groupe « {0} » ({1} sur {2})…', { 0: item, 1: wizard.next + 1, 2: wizard.items.length })
            : learning === 'wizard'
              ? t('Appuie sur la touche du contrôleur pour « {0} » ({1} sur {2})…', { 0: item, 1: wizard.next + 1, 2: wizard.items.length })
              : '';
    status.classList.toggle('active', !!learning);
    refreshPreset();
    if (listKey() !== renderedKey) renderTable();
  }

  midi.onChange(() => {
    if (dlg.open) refresh();
  });
  store.addEventListener('groups', () => dlg.open && refresh());

  return {
    open() {
      midi.start();
      refresh();
      renderTable();
      if (!dlg.open) dlg.showModal();
    },
  };
}
