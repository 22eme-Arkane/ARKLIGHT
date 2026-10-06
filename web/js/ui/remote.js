// Télécommandes infrarouges ARC1 / ARC3 : panneau flottant qui « vise » le projecteur sélectionné.
import { getFixtureType } from '/shared/fixtures/index.js';
import { ARC1_KEYS, ARC3_KEYS, applyIrKey } from '/shared/onboard/ir-remote.js';
import { h, toast } from './dom.js';

// Couleur de l'anneau (ARC1) ou du fond (ARC3) des touches, d'après les photos officielles
const ARC1_RING = { RED: '#e53935', ORANGE: '#fb8c00', YELLOW: '#fdd835', GREEN: '#43a047', CYAN: '#00acc1', BLUE: '#3949ab', PINK: '#ec407a', MAGENTA: '#8e24aa', 'WARM WHITE': '#fbc02d' };
const ARC1_LABEL = { SPEED4: '↑ ▮▮▮▮', SPEED3: '↑ ▮▮▮', SPEED2: '↑ ▮▮', SPEED1: '↑ ▮', STOP: '■', RUN5: '☀ 5h', RUN8: '☼ 8h', RUN12: '· 12h' };
const ARC3_FILL = {
  '2400 K': '#d98a2b',
  '2700 K': '#e3a24a',
  '3000 K': '#e8b765',
  '3200 K': '#eec986',
  '3500 K': '#f2d9a6',
  '4000 K': '#f5e6c6',
  '4500 K': '#f6eedc',
  '5000 K': '#ecedee',
  '5500 K': '#e3e6e9',
  '6000 K': '#d7dde3',
  '6500 K': '#c7d3df',
  '10000 K': '#a9c8ea',
  'BLUE MODE 1/2': '#2b2f7a',
  'BLUE MODE 2/2': '#2b2f7a',
  'MAX RUNTIME': '#d32f2f',
};

export function mountRemote({ store, viewport, onboard }) {
  const panel = h('div', { class: 'remote-panel hidden' });
  document.querySelector('.viewport-wrap').append(panel);
  let model = 'ARC1';
  let target = 'selected';

  function targets() {
    if (target === 'all') return (store.scene?.fixtures || []).filter((f) => getFixtureType(f.type)?.platform?.startsWith('astera-v5'));
    return store.selectedFixtures(); // une télécommande IR atteint tous les projecteurs visés
  }

  function send(key) {
    const list = targets();
    if (!list.length) return toast('Vise un projecteur : sélectionne-le dans la vue 3D', 'error');
    const results = [];
    for (const f of list) {
      const type = getFixtureType(f.type);
      const rt = onboard.runtime(f.id);
      const r = applyIrKey(f, type, rt, key, performance.now() / 1000);
      if (r.changes) onboard._apply(f.id, r.changes);
      results.push(`${f.name} : ${r.accepted ? 'OK' : 'ignoré'}${r.note ? ` (${r.note})` : ''}`);
    }
    feedback.textContent = `${key} → ${results.join(' · ')}`;
  }

  const feedback = h('div', { class: 'remote-feedback' });

  function render() {
    const keys = model === 'ARC1' ? ARC1_KEYS : ARC3_KEYS;
    const grid = h('div', { class: `remote-grid ${model.toLowerCase()}` });
    for (const row of keys) {
      for (const key of row) {
        if (!key) {
          grid.append(h('span'));
          continue;
        }
        const label = model === 'ARC1' ? ARC1_LABEL[key] || key : key;
        const style = model === 'ARC1' ? { borderColor: ARC1_RING[key] || '#d8dce0' } : { background: ARC3_FILL[key] || '#f4f5f6', color: /BLUE|MAX/.test(key) ? '#fff' : '#222' };
        grid.append(h('button', { class: 'remote-key', style, title: key, onclick: () => send(key) }, label));
      }
    }
    panel.replaceChildren(
      h(
        'div',
        { class: 'remote-head' },
        ['ARC1', 'ARC3'].map((m) =>
          h(
            'button',
            {
              class: m === model ? 'active' : '',
              onclick: () => {
                model = m;
                render();
              },
            },
            m === 'ARC1' ? 'ARC1 (FX)' : 'ARC3 (White)',
          ),
        ),
        h('button', { title: 'Fermer', onclick: () => panel.classList.add('hidden') }, '×'),
      ),
      h(
        'select',
        {
          class: 'remote-target',
          onchange: (e) => {
            target = e.target.value;
          },
        },
        h('option', { value: 'selected', selected: target === 'selected' }, 'Vise : projecteurs sélectionnés'),
        h('option', { value: 'all', selected: target === 'all' }, 'Vise : tous les projecteurs Astera'),
      ),
      h('div', { class: `remote-body ${model.toLowerCase()}` }, grid, h('div', { class: 'remote-brand' }, h('span', {}, '▲ ASTERA'), h('em', {}, model))),
      feedback,
    );
  }
  render();
  return {
    toggle() {
      panel.classList.toggle('hidden');
    },
    get open() {
      return !panel.classList.contains('hidden');
    },
  };
}
