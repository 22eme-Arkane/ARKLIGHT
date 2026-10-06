// Onglets "Journal" et "Réseau" + pastilles d'état de la barre du haut.
import { h } from './dom.js';

export function mountLog({ store }) {
  const body = document.querySelector('[data-body="log"]');
  const add = (e) => {
    const t = new Date(e.at || Date.now()).toLocaleTimeString('fr-FR');
    body.append(h('div', { class: `log-line ${e.level || ''}` }, h('time', {}, t), e.text));
    while (body.children.length > 300) body.firstChild.remove();
    body.scrollTop = body.scrollHeight;
  };
  store.addEventListener('log', (e) => add(e.detail));
  return { add };
}

export function mountNetwork({ store, net }) {
  const body = document.querySelector('[data-body="network"]');
  const pills = {
    artnet: document.getElementById('st-artnet'),
    sacn: document.getElementById('st-sacn'),
    ws: document.getElementById('st-ws'),
  };
  const ipEl = document.getElementById('st-ip');
  let lastCounts = { artnet: 0, sacn: 0 };

  const render = () => {
    const s = store.status;
    if (!s) return;
    const offset = store.config.artnetUniverseOffset ?? 1;
    const ifaces = s.interfaces || [];
    ipEl.textContent = ifaces.length ? `IP ${ifaces.map((i) => i.address).join(' / ')}` : 'Pas de réseau';

    for (const k of ['artnet', 'sacn']) {
      const info = s[k];
      const active = info.packets > lastCounts[k];
      lastCounts[k] = info.packets;
      pills[k].className = `pill ${info.error ? 'err' : active ? 'ok' : info.listening ? 'idle' : ''}`;
      pills[k].title = info.error ? `Erreur : ${info.error}` : active ? 'Données reçues' : 'En écoute, aucune donnée';
    }

    body.replaceChildren(
      h(
        'div',
        { class: 'net-grid' },
        h(
          'div',
          {},
          h('h4', {}, 'Connecter une application'),
          h(
            'div',
            { class: 'note' },
            h('p', {}, 'Le PC et le téléphone/tablette doivent être sur le même réseau Wi-Fi.'),
            h('p', {}, 'Dans l’application (Luminair, Blackout, console…), choisis Art-Net ou sACN et vise une de ces adresses :'),
            h('p', { class: 'mono' }, ifaces.map((i) => `${i.address}  (${i.name})`).join('\n') || '—'),
            h('p', {}, 'Art-Net : le simulateur répond à la découverte (ArtPoll) sous le nom « ARKLIGHT ». sACN : multicast ou unicast, port 5568.'),
            h(
              'p',
              {},
              'Rien ne s’affiche dans « Sources DMX actives » alors que l’appli émet ? Le pare-feu Windows bloque sans doute Node.js : Paramètres › Confidentialité et sécurité › Sécurité Windows › Pare-feu › Autoriser une application → cocher Node.js pour les réseaux privés, et vérifier que le Wi-Fi est en profil « Privé ».',
            ),
          ),
          h(
            'div',
            { class: 'row' },
            h('span', {}, 'Numérotation Art-Net'),
            h(
              'select',
              { onchange: (e) => net.send({ t: 'config.update', changes: { artnetUniverseOffset: Number(e.target.value) } }) },
              h('option', { value: 1, selected: offset === 1 }, 'Art-Net 0 = Univers 1'),
              h('option', { value: 0, selected: offset === 0 }, 'Art-Net 0 = Univers 0'),
            ),
          ),
        ),
        h(
          'div',
          {},
          h('h4', {}, 'Sources DMX actives'),
          h(
            'table',
            {},
            h('tr', {}, h('th', {}, 'Protocole'), h('th', {}, 'Adresse'), h('th', {}, 'Nom'), h('th', {}, 'Univers')),
            ...(s.sources.length
              ? s.sources.map((src) => h('tr', {}, h('td', {}, src.protocol), h('td', { class: 'mono' }, src.address), h('td', {}, src.name || ''), h('td', { class: 'mono' }, src.universes.join(', '))))
              : [h('tr', {}, h('td', { colspan: 4, class: 'empty' }, 'Aucune source pour le moment'))]),
          ),
          h('p', { class: 'note' }, `Art-Net : ${s.artnet.packets} paquets, ${s.artnet.polls} découvertes · sACN : ${s.sacn.packets} paquets`),
        ),
      ),
    );
  };

  store.addEventListener('status', render);
  net.addEventListener('open', () => (pills.ws.className = 'pill ok'));
  net.addEventListener('close', () => (pills.ws.className = 'pill err'));
}
