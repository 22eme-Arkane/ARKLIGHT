// Panneau "Inspecteur" : réglages du projecteur sélectionné.
import { effectiveType, getFixtureType, getMode, modeFootprint } from '/shared/fixtures/index.js';
import { controlSource, isAsteraV5, isTraditional, tungstenCct } from '/shared/engine/output.js';
import { deg, h, rad, setFieldValue } from './dom.js';
import { patchIssues, patchLabel } from './patch.js';
import { SUPPORT_CHOICES } from '/shared/fixtures/supports.js';
import { CLOTHS, getGripType } from '/shared/grip.js';

function artnetLabel(universe, offset) {
  const pa = universe - offset;
  if (pa < 0) return '—';
  return `${pa >> 8}:${(pa >> 4) & 15}:${pa & 15}`;
}

function deepAssign(target, patch) {
  for (const [k, v] of Object.entries(patch)) {
    if (v && typeof v === 'object' && !Array.isArray(v) && target[k] && typeof target[k] === 'object') deepAssign(target[k], v);
    else target[k] = v;
  }
}

export function mountInspector({ store, net, viewport, controlPanel, onboard, actions }) {
  const root = document.getElementById('inspector');
  let fields = {};
  let renderedId = null;
  let renderedKey = '';
  /** Clé de ce qui est affiché : un projecteur, ou une sélection multiple. */
  const selectionKey = () =>
    store.selectedObjectId && !store.selection.size ? `obj:${store.selectedObjectId}` : store.selection.size > 1 ? `multi:${store.selectedIds().join(',')}` : store.selectedId || '';
  let throttle = 0;
  let pending = null;

  /** Applique localement puis envoie au serveur (limité à ~30 envois/s pendant un glisser). */
  function update(changes, immediate = true) {
    const f = store.selected();
    if (!f) return;
    deepAssign(f, changes);
    store.upsertFixture(f, store.clientId);
    pending = pending ? (deepAssign(pending, changes), pending) : structuredClone(changes);
    const now = performance.now();
    if (immediate || now - throttle > 33) {
      throttle = now;
      net.send({ t: 'fixture.update', id: f.id, changes: pending });
      pending = null;
    } else {
      clearTimeout(update._t);
      update._t = setTimeout(() => {
        if (pending) net.send({ t: 'fixture.update', id: f.id, changes: pending });
        pending = null;
      }, 40);
    }
  }

  const num = (value, attrs, onchange) =>
    h('input', { type: 'number', value, ...attrs, onchange: (e) => onchange(Number(e.target.value)) });

  const slider = (min, max, step, value, oninput) =>
    h('input', { type: 'range', min, max, step, value, oninput: (e) => oninput(Number(e.target.value)) });

  function renderEmpty() {
    root.append(
      h('h2', {}, 'Inspecteur'),
      h(
        'div',
        { class: 'note' },
        h('p', {}, 'Aucun projecteur sélectionné.'),
        h('p', {}, '1. Glisse des projecteurs de la bibliothèque (à gauche) dans la vue 3D.'),
        h('p', {}, '2. Place-les (W : déplacer, E : pivoter). Ctrl+clic pour en sélectionner plusieurs, clic droit pour les actions.'),
        h('p', {}, '3. Pilote-les : console de test (en bas), appli Art-Net / sACN, ou bientôt l’AsteraApp via la fausse AsteraBox.'),
      ),
    );
  }

  /** Sélection multiple : réglages communs (patch à la suite, profil, état, position). */
  function renderMulti() {
    const list = store.selectedFixtures();
    const types = new Set(list.map((f) => f.type));
    const sameType = types.size === 1 ? getFixtureType([...types][0]) : null;
    root.append(
      h(
        'div',
        { class: 'insp-head' },
        h('div', { class: 'multi-title' }, `${list.length} projecteurs sélectionnés`),
        h('button', { title: 'Grouper la sélection (Ctrl+G)', onclick: () => actions.groupSelection() }, '▦'),
        h('button', { title: 'Dupliquer (Ctrl+D)', onclick: () => actions.duplicateSelection() }, '⧉'),
        h('button', { title: 'Supprimer (Suppr)', onclick: () => actions.deleteSelection() }, '🗑'),
      ),
      h('div', { class: 'insp-sub' }, sameType ? `${sameType.brand} ${sameType.name} × ${list.length}` : `${types.size} modèles différents`),
    );

    const rows = h('div', { class: 'multi-list' });
    fields.multiRows = rows;
    root.append(h('div', { class: 'group' }, h('h4', {}, 'Projecteurs'), rows, h('div', { class: 'note' }, 'Ctrl+clic : ajouter / retirer · Maj+clic : plage · clic droit : actions')));

    // ---- patch : adressage à la suite à partir d'une adresse de départ
    const first = list[0];
    const inU = list.filter((f) => f.settings.universe === first.settings.universe).map((f) => f.settings.dmxAddress);
    const uni = h('input', { type: 'number', min: 1, max: 63999, value: first.settings.universe });
    const addr = h('input', { type: 'number', min: 1, max: 512, value: Math.min(...inU) });
    const patchRows = [
      h('div', { class: 'row' }, h('span', {}, 'Univers'), uni),
      h('div', { class: 'row' }, h('span', {}, 'Adresse de départ'), addr),
    ];
    if (sameType?.dmx?.modes?.length) {
      const modes = new Set(list.map((f) => f.settings.dmxMode));
      const common = modes.size === 1 ? [...modes][0] : '';
      patchRows.push(
        h(
          'div',
          { class: 'row' },
          h('span', {}, 'Profil'),
          h(
            'select',
            { onchange: (e) => e.target.value && actions.updateMany(list.map((f) => ({ id: f.id, changes: { settings: { dmxMode: e.target.value } } }))) },
            common ? null : h('option', { value: '', selected: true }, '— profils différents —'),
            sameType.dmx.modes.map((m) => h('option', { value: m.id, selected: m.id === common }, `${m.label || m.name} — ${modeFootprint(sameType, m.id)} c.`)),
          ),
        ),
      );
    }
    const warn = h('div', { class: 'warnbox' });
    fields.multiWarn = warn;
    patchRows.push(
      h(
        'div',
        { class: 'btn-row' },
        h(
          'button',
          {
            class: 'primary',
            title: 'Adresse les projecteurs sélectionnés l’un après l’autre (ordre de la liste), sans chevaucher les autres',
            onclick: () => actions.addressSequential(store.selectedIds(), Math.max(1, Math.round(Number(uni.value) || 1)), Math.min(512, Math.max(1, Math.round(Number(addr.value) || 1)))),
          },
          'Adresser à la suite',
        ),
        h('button', { title: 'Ordre, alignement des adresses, débordement…', onclick: () => actions.openReaddress({ scope: 'selection' }) }, 'Options…'),
      ),
      warn,
    );
    root.append(h('div', { class: 'group' }, h('h4', {}, 'Patch DMX'), patchRows));

    // ---- état
    const ids = list.map((f) => f.id);
    const v5 = list.filter((f) => isAsteraV5(getFixtureType(f.type)));
    root.append(
      h(
        'div',
        { class: 'group' },
        h('h4', {}, 'État simulé'),
        h(
          'div',
          { class: 'btn-row' },
          h('button', { onclick: () => actions.setPower(ids, true) }, 'Allumer'),
          h('button', { onclick: () => actions.setPower(ids, false) }, 'Éteindre'),
          v5.length ? h('button', { title: 'Comme la touche LINK d’un émetteur CRMX', onclick: () => actions.setCrmx(ids, true) }, 'Lier CRMX') : null,
          v5.length ? h('button', { onclick: () => actions.setCrmx(ids, false) }, 'Délier CRMX') : null,
        ),
      ),
      h(
        'div',
        { class: 'group' },
        h('h4', {}, 'Position'),
        h('div', { class: 'note' }, 'La poignée de la vue 3D déplace et fait pivoter toute la sélection autour de son centre.'),
        h(
          'div',
          { class: 'btn-row' },
          h('button', { onclick: () => actions.aim() }, 'Viser le personnage'),
          h('button', { onclick: () => actions.drop() }, 'Poser au sol'),
          h('button', { onclick: () => actions.frame() }, 'Cadrer'),
        ),
      ),
    );
    refreshMulti();
  }

  function refreshMulti() {
    if (!fields.multiRows) return;
    const list = store.selectedFixtures();
    const all = store.scene.fixtures;
    fields.multiRows.replaceChildren(
      ...list.map((f) => {
        const issues = patchIssues(f, all);
        return h(
          'div',
          { class: `multi-row${f.id === store.selectedId ? ' primary' : ''}`, onclick: (e) => (e.ctrlKey || e.metaKey ? store.toggleSelect(f.id) : store.select(f.id)), title: 'Clic : n’afficher que ce projecteur' },
          h('span', { class: 'nm' }, f.name),
          h('span', { class: `patch mono ${issues.length ? 'warn' : ''}`, title: issues.join('\n') }, issues.length ? '⚠ ' : '', patchLabel(f)),
        );
      }),
    );
    const conflicts = list.filter((f) => patchIssues(f, all).length).length;
    fields.multiWarn.textContent = conflicts ? `⚠ ${conflicts} projecteur(s) de la sélection en conflit de patch` : '';
  }

  /** Élément de grip sélectionné : nom, toile, hauteur, position. */
  function renderObject() {
    const o = store.selectedObject();
    const type = getGripType(o.type);
    const set = (changes) => actions.updateObject(o.id, changes);
    const name = h('input', { value: o.name, onchange: (e) => set({ name: e.target.value }) });
    root.append(
      h(
        'div',
        { class: 'insp-head' },
        name,
        h('button', { title: 'Dupliquer (Ctrl+D)', onclick: () => actions.duplicateObject(o.id) }, '⧉'),
        h('button', { title: 'Supprimer (Suppr)', onclick: () => actions.deleteObject(o.id) }, '🗑'),
      ),
      h('div', { class: 'insp-sub' }, `Grip · ${type.name}`),
    );
    const rows = [];
    if (type.cloths) {
      rows.push(
        h(
          'div',
          { class: 'row' },
          h('span', {}, type.family === 'frame' ? 'Toile' : 'Garniture'),
          h('select', { onchange: (e) => set({ props: { cloth: e.target.value } }) }, type.cloths.map((c) => h('option', { value: c, selected: o.props.cloth === c }, CLOTHS[c].name))),
        ),
      );
    }
    if (type.family === 'stand') rows.push(h('div', { class: 'row' }, h('span', {}, 'Hauteur (m)'), num(o.props.height ?? type.height, { step: 0.1, min: 0.5, max: 6 }, (v) => set({ props: { height: v } }))));
    const pos = [0, 1, 2].map((i) =>
      num(o.position[i], { step: 0.05 }, (v) => {
        const p = [...o.position];
        p[i] = v;
        set({ position: p });
      }),
    );
    const rot = [0, 1, 2].map((i) =>
      num(deg(o.rotation[i]), { step: 5 }, (v) => {
        const r = [...o.rotation];
        r[i] = rad(v);
        set({ rotation: r });
      }),
    );
    fields.objPos = pos;
    fields.objRot = rot;
    rows.push(h('div', { class: 'row' }, h('span', {}, 'X / Y / Z (m)'), h('div', { class: 'vec3' }, pos)), h('div', { class: 'row' }, h('span', {}, 'Rotation (°)'), h('div', { class: 'vec3' }, rot)));
    rows.push(h('div', { class: 'btn-row' }, h('button', { onclick: () => set({ position: [o.position[0], 0, o.position[2]] }) }, 'Poser au sol')));
    root.append(h('div', { class: 'group' }, h('h4', {}, 'Élément'), rows));
    const notes = [];
    if (type.stands) notes.push(`Support compté dans la liste du matériel : ${type.stands.count} × ${type.stands.label}.`);
    if (type.sandbags) notes.push(`Sacs de sable conseillés : ${type.sandbags}.`);
    notes.push('Le grip est visuel : il ne fait pas encore d’ombre et ne diffuse pas la lumière.');
    root.append(h('div', { class: 'group' }, h('h4', {}, 'Liste du matériel'), h('div', { class: 'note' }, notes.map((n) => h('div', {}, n)))));
  }

  function refreshObject() {
    const o = store.selectedObject();
    if (!o || !fields.objPos) return;
    fields.objPos.forEach((inp, i) => setFieldValue(inp, o.position[i]));
    fields.objRot.forEach((inp, i) => setFieldValue(inp, deg(o.rotation[i])));
  }

  function render() {
    const f = store.selected();
    renderedId = f?.id || null;
    renderedKey = selectionKey();
    root.replaceChildren();
    fields = {};
    controlPanel?.unmount();
    if (store.selectedObjectId && !store.selection.size && store.selectedObject()) return renderObject();
    if (store.selection.size > 1) return renderMulti();
    if (!f) return renderEmpty();
    const type = getFixtureType(f.type);

    // ---- en-tête
    const name = h('input', { value: f.name, onchange: (e) => update({ name: e.target.value }) });
    root.append(
      h(
        'div',
        { class: 'insp-head' },
        name,
        h('button', { title: 'Dupliquer (Ctrl+D)', onclick: () => document.dispatchEvent(new CustomEvent('ark:duplicate')) }, '⧉'),
        h('button', { title: 'Supprimer (Suppr)', onclick: () => document.dispatchEvent(new CustomEvent('ark:delete')) }, '🗑'),
      ),
      h(
        'div',
        { class: 'insp-sub' },
        `${type.brand} ${type.name} (${type.model})${type.pixels ? ` · ${type.pixels} pixel${type.pixels > 1 ? 's' : ''}` : ''}${type.output?.luxAt1m && !type.output.estimated ? ` · ${effectiveType(type, f).output.luxAt1m} lux à 1 m${type.zoom ? ` (zoom ${f.settings.zoom ?? type.beamAngle}°)` : ''}` : ''}`,
      ),
    );
    fields.name = name;

    // ---- panneau de commande embarqué (menus du projecteur)
    if (controlPanel) root.append(controlPanel.render(f, type));

    // ---- position
    const posInputs = [0, 1, 2].map((i) =>
      num(f.position[i], { step: 0.05 }, (v) => {
        const p = [...f.position];
        p[i] = v;
        update({ position: p });
      }),
    );
    const rotInputs = [0, 1, 2].map((i) =>
      num(deg(f.rotation[i]), { step: 5 }, (v) => {
        const r = [...f.rotation];
        r[i] = rad(v);
        update({ rotation: r });
      }),
    );
    fields.pos = posInputs;
    fields.rot = rotInputs;
    const quick = [];
    if (type.geometry.kind === 'tube') {
      quick.push(h('button', { onclick: () => viewport.setSelectedRotation([0, f.rotation[1], 0]) }, 'Debout'));
      quick.push(h('button', { onclick: () => viewport.setSelectedRotation([0, f.rotation[1], Math.PI / 2]) }, 'Couché'));
    } else {
      quick.push(h('button', { onclick: () => viewport.aimSelectedAtActor() }, 'Viser le personnage'));
    }
    quick.push(h('button', { onclick: () => viewport.dropSelectedToFloor() }, 'Poser au sol'));
    const supportSel = h(
      'select',
      { onchange: (e) => update({ settings: { support: e.target.value } }) },
      SUPPORT_CHOICES.map(([v, l]) => h('option', { value: v, selected: (f.settings.support || 'auto') === v }, l)),
    );
    const supportInfo = h('div', { class: 'note' });
    Object.assign(fields, { supportSel, supportInfo });
    root.append(
      h(
        'div',
        { class: 'group' },
        h('h4', {}, 'Position'),
        h('div', { class: 'row' }, h('span', {}, 'X / Y / Z (m)'), h('div', { class: 'vec3' }, posInputs)),
        h('div', { class: 'row' }, h('span', {}, 'Rotation (°)'), h('div', { class: 'vec3' }, rotInputs)),
        h('div', { class: 'btn-row' }, quick),
        h('div', { class: 'row' }, h('span', {}, 'Support'), supportSel),
        supportInfo,
      ),
    );

    // ---- zoom mécanique (SolaBulb, Pluto, Leo)
    if (type.zoom) {
      const zoomVal = h('span', { class: 'mono' }, `${f.settings.zoom ?? type.beamAngle}°`);
      const zoom = slider(type.zoom.min, type.zoom.max, 1, f.settings.zoom ?? type.beamAngle, (v) => {
        zoomVal.textContent = type.zoom.hotBeam === v ? 'HotBeam' : `${v}°${type.zoom.detent === v ? ' (cran)' : ''}`;
        update({ settings: { zoom: v } }, false);
      });
      root.append(
        h(
          'div',
          { class: 'group' },
          h('h4', {}, 'Zoom (bague manuelle)', zoomVal),
          h('div', { class: 'row' }, h('span', {}, `${type.zoom.min}° – ${type.zoom.max}°`), zoom),
        ),
      );
    }

    // ---- patch DMX
    const v5 = isAsteraV5(type);
    const modeSel = h(
      'select',
      { onchange: (e) => update({ settings: { dmxMode: e.target.value } }) },
      (type.dmx?.modes || []).map((m) => h('option', { value: m.id, selected: m.id === f.settings.dmxMode }, `${m.label || m.name} — ${modeFootprint(type, m.id)} c.`)),
    );
    const uni = num(f.settings.universe, { min: 1, max: 63999 }, (v) => update({ settings: { universe: Math.max(1, Math.round(v)) } }));
    const addr = num(f.settings.dmxAddress, { min: 1, max: 512 }, (v) => update({ settings: { dmxAddress: Math.min(512, Math.max(1, Math.round(v))) } }));
    const src = h(
      'select',
      { onchange: (e) => update({ settings: { source: e.target.value } }) },
      [
        ['auto', 'Auto (DMX si reçu, sinon manuel)'],
        ['dmx', 'DMX uniquement'],
        ['manual', 'Manuel uniquement'],
      ].map(([v, l]) => h('option', { value: v, selected: (f.settings.source || 'auto') === v }, l)),
    );
    const info = h('div', { class: 'note mono' });
    const warn = h('div', { class: 'warnbox' });
    const live = h('div', { class: 'note' });
    Object.assign(fields, { modeSel, uni, addr, src, info, warn, live });
    const isBox = type.platform === 'astera-box';
    const patchRows = isBox
      ? [
          h('div', { class: 'row' }, h('span', {}, 'Entrée DMX'), uni),
          h('div', { class: 'note' }, 'Univers que la console / l’appli envoie dans la prise DMX de la box. En mode TX, la box le transmet en CRMX aux projecteurs liés par la touche LINK.'),
        ]
      : [
          h('div', { class: 'row' }, h('span', {}, v5 ? 'Univers CRMX' : 'Univers'), uni),
          h('div', { class: 'row' }, h('span', {}, 'Adresse'), addr),
          h('div', { class: 'row' }, h('span', {}, 'Profil'), modeSel),
        ];
    if (v5) {
      // Liaison CRMX (touche LINK de l'émetteur) et câble DMX filaire
      const linkBtn = h('button', {
        onclick: () => {
          const linked = !store.selected().settings.crmxLinked;
          update({ settings: { crmxLinked: linked } });
          const rt = onboard?.runtime(f.id);
          if (rt) {
            rt.latch = null;
            rt.justUnlinked = !linked;
          }
        },
      });
      const wired = h('input', { type: 'checkbox', checked: !!f.settings.wired, onchange: (e) => update({ settings: { wired: e.target.checked } }) });
      fields.linkBtn = linkBtn;
      fields.wired = wired;
      const wiredLabel = type.controls?.wiredLabel || (type.category === 'bar' ? 'XLR DMX IN' : type.model?.startsWith('FP3') ? 'DataLink (filaire)' : 'PowerBox (filaire)');
      patchRows.push(
        h('div', { class: 'row' }, h('span', {}, 'CRMX'), linkBtn),
        type.platform === 'astera-v5-button' ? null : h('div', { class: 'row' }, h('span', {}, wiredLabel), wired),
      );
    } else if (!isBox) patchRows.push(h('div', { class: 'row' }, h('span', {}, 'Commande'), src));
    root.append(h('div', { class: 'group' }, h('h4', {}, isBox ? 'Liaison CRMX' : 'Patch DMX'), patchRows, isBox ? null : info, isBox ? null : warn, isBox ? null : live));

    // ---- réglage rapide (formateur) : projecteurs génériques seulement
    const m = f.state.manual;
    const trad = isTraditional(type);
    const manualBox = h('div', { class: 'group' });
    const modeBtns = ['cct', 'hsi', 'rgb'].map((k) =>
      h(
        'button',
        {
          class: m.mode === k ? 'active' : '',
          onclick: () => {
            update({ state: { manual: { mode: k } } });
            render();
          },
        },
        k.toUpperCase(),
      ),
    );
    const rows = [
      h('div', { class: 'row' }, h('span', {}, 'Intensité'), slider(0, 1, 0.005, m.intensity, (v) => update({ state: { manual: { intensity: v } } }, false))),
    ];
    if (m.mode === 'cct') {
      rows.push(h('div', { class: 'row' }, h('span', {}, 'Kelvin'), slider(1750, 20000, 50, m.cct, (v) => update({ state: { manual: { cct: v } } }, false))));
      rows.push(h('div', { class: 'row' }, h('span', {}, 'Teinte ±vert'), slider(-1, 1, 0.01, m.tint, (v) => update({ state: { manual: { tint: v } } }, false))));
    } else if (m.mode === 'hsi') {
      rows.push(h('div', { class: 'row' }, h('span', {}, 'Teinte (°)'), slider(0, 360, 1, m.hue, (v) => update({ state: { manual: { hue: v } } }, false))));
      rows.push(h('div', { class: 'row' }, h('span', {}, 'Saturation'), slider(0, 1, 0.01, m.sat, (v) => update({ state: { manual: { sat: v } } }, false))));
    } else {
      const rgb = m.rgb || [1, 1, 1];
      ['R', 'V', 'B'].forEach((lbl, i) =>
        rows.push(
          h(
            'div',
            { class: 'row' },
            h('span', {}, lbl),
            slider(0, 1, 0.005, rgb[i], (v) => {
              const next = [...(f.state.manual.rgb || [1, 1, 1])];
              next[i] = v;
              update({ state: { manual: { rgb: next } } }, false);
            }),
          ),
        ),
      );
    }
    if (trad) {
      // tungstène / HMI : seule l'intensité se règle, la couleur découle de la lampe
      manualBox.append(h('h4', {}, 'Réglage rapide (formateur)'), rows[0]);
      manualBox.append(h('div', { class: 'note' }, 'Comme un gradateur manuel, quand le projecteur n’est pas piloté en DMX.'));
    } else {
      manualBox.append(h('h4', {}, 'Réglage rapide (formateur)', h('span', { class: 'btn-row' }, modeBtns)), ...rows);
      manualBox.append(h('div', { class: 'note' }, 'Utilisé quand le projecteur n’est pas piloté en DMX.'));
    }
    if (!v5 && !isBox) root.append(manualBox);

    // ---- état physique
    const power = h('input', { type: 'checkbox', checked: f.state.power, onchange: (e) => update({ state: { power: e.target.checked } }) });
    const battery = slider(0, 100, 1, f.state.battery, (v) => update({ state: { battery: v } }, false));
    const charging = h('input', { type: 'checkbox', checked: !!f.state.charging, onchange: (e) => update({ state: { charging: e.target.checked } }) });
    fields.power = power;
    fields.battery = battery;
    fields.charging = charging;
    root.append(
      h(
        'div',
        { class: 'group' },
        h('h4', {}, 'État simulé'),
        h('div', { class: 'row' }, h('span', {}, 'Allumé'), power),
        // batterie : seulement pour les projecteurs Astera (les autres sont simulés sur secteur)
        type.platform?.startsWith('astera') ? h('div', { class: 'row' }, h('span', {}, 'Batterie (%)'), battery) : null,
        type.platform?.startsWith('astera') ? h('div', { class: 'row' }, h('span', {}, 'Sur secteur'), charging) : null,
        isBox
          ? h(
              'div',
              { class: 'row' },
              h('span', {}, 'Câble DMX'),
              h('input', { type: 'checkbox', checked: f.state.dmxCable !== false, onchange: (e) => update({ state: { dmxCable: e.target.checked } }) }),
            )
          : null,
        v5
          ? h(
              'div',
              { class: 'btn-row' },
              h('button', { title: 'Comme quand l’appli demande de « taper » les lampes (DMX Configuration, Sets)', onclick: () => update({ state: { acceptConfig: true } }) }, 'Demande « tap » de l’appli'),
            )
          : null,
      ),
    );

    // ---- origine des données (autres marques, tungstène, HMI)
    if (type.source || type.output?.estimated) {
      const notes = [];
      if (type.source?.kind === 'ofl') notes.push(h('div', {}, 'Profils DMX : ', h('a', { href: type.source.url, target: '_blank', rel: 'noopener' }, 'Open Fixture Library'), '.'));
      if (type.confidence) notes.push(h('div', {}, `Fiabilité : ${type.confidence}.`));
      if (type.source?.notes) notes.push(h('div', {}, type.source.notes));
      if (type.output?.estimated) notes.push(h('div', {}, 'Sortie lumineuse estimée : à remplacer par la fiche photométrique ou une mesure.'));
      if (type.source?.url && type.source.kind !== 'ofl') notes.push(h('div', {}, h('a', { href: type.source.url, target: '_blank', rel: 'noopener' }, 'Source')));
      root.append(h('div', { class: 'group' }, h('h4', {}, 'Données du modèle'), h('div', { class: 'note' }, notes)));
    }

    refresh();
  }

  /** Met à jour les valeurs affichées sans reconstruire le panneau. */
  function refresh() {
    if (fields.multiRows) return refreshMulti();
    const f = store.selected();
    if (!f || f.id !== renderedId) return;
    const type = getFixtureType(f.type);
    setFieldValue(fields.name, f.name);
    fields.pos?.forEach((inp, i) => setFieldValue(inp, f.position[i]));
    fields.rot?.forEach((inp, i) => setFieldValue(inp, deg(f.rotation[i])));
    if (fields.supportSel) setFieldValue(fields.supportSel, f.settings.support || 'auto');
    refreshSupport();
    setFieldValue(fields.uni, f.settings.universe);
    setFieldValue(fields.addr, f.settings.dmxAddress);
    setFieldValue(fields.modeSel, f.settings.dmxMode);
    setFieldValue(fields.src, f.settings.source || 'auto');
    setFieldValue(fields.power, f.state.power);
    setFieldValue(fields.battery, f.state.battery);
    if (fields.charging) setFieldValue(fields.charging, !!f.state.charging);
    if (fields.wired) setFieldValue(fields.wired, !!f.settings.wired);
    if (fields.linkBtn) fields.linkBtn.textContent = f.settings.crmxLinked ? `Lié (univers ${f.settings.universe}) — délier` : 'Non lié — lier (touche LINK)';
    const mode = getMode(type, f.settings.dmxMode);
    const fp = modeFootprint(type, f.settings.dmxMode);
    const end = f.settings.dmxAddress + fp - 1;
    fields.info.textContent = `Canaux ${f.settings.dmxAddress}–${end} (${fp} c., ${mode?.name || '—'}) · Art-Net ${artnetLabel(f.settings.universe, store.config.artnetUniverseOffset ?? 1)}`;
    const issues = patchIssues(f, store.scene.fixtures);
    fields.warn.textContent = issues.length ? `⚠ ${issues.join(' · ')}` : '';
    refreshLive();
    controlPanel?.refresh(f, type);
  }

  /** Pied retenu (calculé par la vue 3D selon la hauteur) et avertissements de charge / hauteur. */
  function refreshSupport() {
    const f = store.selected();
    const sup = f && viewport.outputOf(f.id)?.support;
    if (!fields.supportInfo || !sup) return;
    const what = sup.def ? `${sup.auto ? 'Choisi : ' : ''}${sup.def.name} — ${sup.def.minH}–${sup.def.maxH} m, ${sup.def.load} kg maxi, spigot ${sup.def.spigot}` : sup.id === 'hanging' ? 'Accroché : pas de pied dans la liste du matériel.' : 'Pas de pied.';
    fields.supportInfo.replaceChildren(h('div', {}, what), ...sup.warnings.map((w) => h('div', { class: 'warnbox' }, `⚠ ${w}`)));
  }

  function refreshLive() {
    refreshSupport();
    const f = store.selected();
    if (!f || !fields.live) return;
    const hasData = store.dmx.has(f.settings.universe);
    const isLive = store.liveUniverses.has(f.settings.universe);
    const src = controlSource(f, hasData);
    const sources = (store.status?.sources || []).filter((s) => s.universes.includes(f.settings.universe));
    const type = getFixtureType(f.type);
    let text;
    if (isAsteraV5(type)) {
      const rt = onboard?.runtime(f.id) || {};
      const names = { DETECTING: 'aucune (détection en cours)', CRMX: 'CRMX sans fil', WIRED: 'DMX filaire', APP: 'appli', STANDALONE: 'autonome', REMOTE: 'télécommande IR', ART3: 'ART3', BLUEMODE: 'BlueMode', STANDBY: 'veille', OFF: '—' };
      const sel = f.settings.inputSelect || 'AUTO';
      text = !f.state.power
        ? 'Projecteur éteint.'
        : `INPUT SELECT = ${sel} · entrée active : ${names[rt.input] || rt.input || '—'}${rt.input === 'CRMX' || rt.input === 'WIRED' ? (rt.signal ? ` — signal de ${sources.map((s) => s.name || s.address).join(', ')}` : ' — pas de signal') : ''}`;
      fields.live.textContent = text;
      return;
    }
    if (isTraditional(type) && f.state.power) {
      // flux réel du filament / de la lampe et température de couleur qui en découle
      const flux = onboard?.runtime(f.id)?.flux?.[0] ?? 0;
      const nominal = type.lamp?.cct || 3200;
      const kelvin = type.technology === 'tungsten' ? Math.round(tungstenCct(nominal, flux) / 10) * 10 : nominal;
      const lamp = flux > 0.001 ? `lampe à ${Math.round(flux * 100)} %${type.technology === 'tungsten' ? ` · ≈ ${kelvin} K` : ''}` : 'lampe éteinte';
      text = `${src === 'dmx' ? (isLive ? 'Piloté en DMX' : 'DMX (dernier état conservé)') : 'Commande manuelle'} — ${lamp}.`;
      fields.live.textContent = text;
      return;
    }
    if (!f.state.power) text = 'Projecteur éteint.';
    else if (src === 'dmx')
      text = isLive
        ? `Piloté en DMX — signal reçu de ${sources.map((s) => `${s.name || s.address} (${s.protocol})`).join(', ')}`
        : hasData
          ? 'Piloté en DMX — plus de signal, dernier état conservé.'
          : 'DMX uniquement — aucun signal reçu sur cet univers.';
    else text = hasData ? 'Commande manuelle (DMX ignoré).' : `Commande manuelle — aucun signal DMX sur l’univers ${f.settings.universe}.`;
    fields.live.textContent = text;
  }

  store.addEventListener('selection', () => {
    if (selectionKey() !== renderedKey) render();
    else refresh();
  });
  store.addEventListener('scene', render);
  store.addEventListener('object-selection', () => {
    if (selectionKey() !== renderedKey) render();
    else refreshObject();
  });
  store.addEventListener('object-updated', (e) => {
    if (e.detail.object.id === store.selectedObjectId) refreshObject();
  });
  viewport.addEventListener('object-transform', refreshObject);
  let multiQueued = false;
  store.addEventListener('fixture-updated', (e) => {
    if (fields.multiRows) {
      // sélection multiple : une seule mise à jour par image
      if (multiQueued || !store.isSelected(e.detail.fixture.id)) return;
      multiQueued = true;
      requestAnimationFrame(() => {
        multiQueued = false;
        refreshMulti();
      });
    } else if (e.detail.fixture.id === renderedId) refresh();
  });
  store.addEventListener('status', refreshLive);
  viewport.addEventListener('transform', () => refresh());
  render();
  return { render, refresh };
}
