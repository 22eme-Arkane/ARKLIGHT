// Onglet "Bluetooth" : pont BLE (fausse AsteraBox), émulateur actif et trafic échangé avec le téléphone.
import { h } from './dom.js';

export function mountBluetooth({ store, net }) {
  const body = document.querySelector('[data-body="bluetooth"]');
  const pill = document.getElementById('st-box');
  let status = null;
  const traffic = [];

  const head = h('div', { class: 'ble-head' });
  const list = h('div', { class: 'ble-traffic' });
  body.append(head, list);

  function renderHead() {
    const s = status;
    if (!s) return;
    const b = s.bridge;
    const adapter = b?.adapter;
    const emu = s.emulators.find((e) => e.id === s.emulator);
    let state;
    let cls = '';
    if (!s.process && !b) state = 'Pont arrêté';
    else if (!b) state = 'Démarrage du pont…';
    else if (!adapter?.radioOn) {
      state = `Bluetooth ${adapter?.radio || 'indisponible'} dans Windows — active-le dans Paramètres › Bluetooth et appareils`;
      cls = 'err';
    } else if (s.phones.length) {
      state = `Téléphone connecté (${s.phones.length})`;
      cls = 'ok';
    } else if (b.published && b.services === 0) {
      state = `Rien n’est annoncé : le profil « ${b.published} » est vide (protocole pas encore capturé)`;
      cls = 'err';
    } else if (b.published) {
      state = `Visible sous forme de « ${b.published} », en attente d’un téléphone`;
      cls = 'idle';
    } else state = 'Pont connecté';

    pill.className = `pill ${cls}`;
    pill.title = state;

    head.replaceChildren(
      h(
        'div',
        { class: 'console-head' },
        h('strong', {}, 'Fausse AsteraBox'),
        h('span', {}, state),
        h('span', { class: 'spacer' }),
        h(
          'select',
          { title: 'Profil publié en Bluetooth', onchange: (e) => net.send({ t: 'ble.emulator', id: e.target.value }) },
          s.emulators.map((e) => h('option', { value: e.id, selected: e.id === s.emulator }, e.name)),
        ),
        s.process || b
          ? h('button', { onclick: () => net.send({ t: 'ble.stop' }) }, 'Arrêter le Bluetooth')
          : h('button', { class: 'primary', onclick: () => net.send({ t: 'ble.start' }) }, 'Démarrer le Bluetooth'),
      ),
      emu?.note ? h('div', { class: emu.ready ? 'note' : 'warnbox' }, emu.note) : null,
      adapter ? h('div', { class: 'note mono' }, `Adaptateur ${adapter.address} · rôle périphérique : ${adapter.peripheralRole ? 'oui' : 'NON'} · radio : ${adapter.radio}`) : null,
    );
  }

  function addTraffic(e) {
    traffic.push(e);
    if (traffic.length > 300) traffic.shift();
    const t = new Date(e.at).toLocaleTimeString('fr-FR');
    const ascii = e.hex
      .match(/../g)
      ?.map((x) => {
        const c = parseInt(x, 16);
        return c >= 32 && c < 127 ? String.fromCharCode(c) : '.';
      })
      .join('');
    list.append(
      h(
        'div',
        { class: 'log-line' },
        h('time', {}, t),
        `${e.dir === 'in' ? 'tél → PC' : 'PC → tél'}  ${e.op.padEnd(9)} ${e.char.slice(0, 8)}…  ${e.hex.replace(/(..)/g, '$1 ')} ${ascii ? ` « ${ascii} »` : ''}`,
      ),
    );
    while (list.children.length > 300) list.firstChild.remove();
    list.scrollTop = list.scrollHeight;
  }

  net.addEventListener('message', (ev) => {
    const msg = ev.detail;
    if (msg.t === 'ble.status') {
      status = msg;
      store.ble = msg;
      if (msg.traffic) msg.traffic.forEach(addTraffic);
      renderHead();
    } else if (msg.t === 'ble.traffic') addTraffic(msg.entry);
  });
  net.addEventListener('close', () => (pill.className = 'pill'));
}
