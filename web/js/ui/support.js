// Soutien au projet (don, comme dans PANDORA et Armana), fenêtre de fermeture avec enregistrement,
// « À propos » avec la vérification des mises à jour, bandeau « nouvelle version disponible ».
import { h, toast } from './dom.js';

// ⚠ PayPal.me et non « paypal.com/donate?business=… » : ce dernier est réservé aux associations
// enregistrées et refuse les dons sur un compte ordinaire (constaté sur PANDORA en juillet 2026).
const PAYPAL_URL = 'https://paypal.me/22emearkane';
const BTC_ADDRESS = 'bc1p386af47cm9eyk8k9cse5r72s7z9nr50q2s0ysk8d8ys5vugmnj7qgn3hca';
const USDC_ADDRESS = '0x15a5177E00eD4D916953A3492C562197E91f7045';

/** Ouvre une adresse dans le navigateur (application de bureau) ou dans un nouvel onglet. */
export function openExternal(url) {
  if (window.arkDesktop) window.arkDesktop.openExternal(url);
  else window.open(url, '_blank', 'noopener');
}

function dialog(cls) {
  const d = h('dialog', { class: `dialog ${cls}` });
  document.body.append(d);
  d.addEventListener('close', () => setTimeout(() => d.remove(), 0));
  return d;
}

function copyButton(text) {
  const b = h('button', {
    class: 'sup-copy',
    onclick: async () => {
      try {
        await navigator.clipboard.writeText(text);
        b.textContent = '✓ Copié';
        setTimeout(() => (b.textContent = '⎘ Copier'), 2000);
      } catch {
        toast('Copie impossible : sélectionne l’adresse à la main', 'error');
      }
    },
  });
  b.textContent = '⎘ Copier';
  return b;
}

/** Fenêtre « Faire un don » : PayPal ou cryptomonnaie. */
export function openSupport() {
  const d = dialog('support');
  const crypto = h(
    'div',
    { class: 'sup-crypto hidden' },
    [
      ['₿', 'Bitcoin', 'Réseau Bitcoin · Taproot', BTC_ADDRESS, '#f7931a'],
      ['$', 'USDC', 'Réseau Sonic', USDC_ADDRESS, '#2775ca'],
    ].map(([icon, name, net, addr, color]) =>
      h('div', { class: 'sup-coin' }, h('div', { class: 'sup-coin-head' }, h('span', { class: 'sup-icon', style: { background: color } }, icon), h('strong', {}, name), h('span', { class: 'sup-net' }, net)), h('code', {}, addr), copyButton(addr)),
    ),
  );
  d.append(
    h('div', { class: 'sup-head' }, h('img', { src: '/img/ark-light-logo.png', alt: '' }), h('div', {}, h('h3', {}, 'Faire un don'), h('div', { class: 'note' }, 'ARKLIGHT · simulateur lumière libre'))),
    h('p', {}, 'ARKLIGHT est libre, gratuit et sans publicité. Il est développé par 22eme ARKANE pour l’électro et la lumière, sur le temps libre et à ses frais.'),
    h('p', { class: 'note' }, 'Si ce logiciel t’est utile, un don, même modeste, aide à continuer à le développer. C’est totalement facultatif : rien n’est bloqué, rien n’est limité.'),
    h(
      'button',
      { class: 'sup-card', onclick: () => (openExternal(PAYPAL_URL), d.close()) },
      h('span', { class: 'sup-icon', style: { background: '#009cde' } }, 'P'),
      h('span', {}, h('strong', {}, 'PayPal'), h('span', { class: 'note' }, 'Don rapide en euros : carte bancaire ou compte PayPal')),
      h('span', { class: 'sup-arrow' }, '›'),
    ),
    h(
      'button',
      { class: 'sup-card', onclick: () => crypto.classList.toggle('hidden') },
      h('span', { class: 'sup-icon', style: { background: '#f7931a' } }, '₿'),
      h('span', {}, h('strong', {}, 'Cryptomonnaie'), h('span', { class: 'note' }, 'Bitcoin (Taproot) · USDC (réseau Sonic)')),
      h('span', { class: 'sup-arrow' }, '›'),
    ),
    crypto,
    h('div', { class: 'sup-thanks' }, 'Merci pour votre soutien. ◈ 22eme ARKANE'),
    h('div', { class: 'dialog-actions' }, h('button', { onclick: () => d.close() }, 'Fermer')),
  );
  d.showModal();
}

/**
 * Fenêtre de fermeture (application de bureau), comme dans PANDORA.
 * dirty : le projet a des modifications non enregistrées.
 * @returns {Promise<0|1|2>} 0 : annuler · 1 : enregistrer et quitter · 2 : quitter sans enregistrer
 */
export function confirmQuit(sceneName, dirty = true) {
  return new Promise((resolve) => {
    const d = dialog('quit');
    let result = 0;
    const done = (r) => {
      result = r;
      d.close();
    };
    d.addEventListener('close', () => resolve(result));
    d.append(
      h('h3', {}, 'Voulez-vous vraiment quitter ARKLIGHT ?'),
      h(
        'p',
        { class: 'note' },
        dirty
          ? `Le projet « ${sceneName} » a des modifications non enregistrées. Il est gardé automatiquement et sera rouvert au prochain lancement ; « Enregistrer et quitter » l’enregistre aussi dans ton dossier de projets.`
          : `Le projet « ${sceneName} » est enregistré. Il sera rouvert au prochain lancement.`,
      ),
      h(
        'div',
        { class: 'quit-support' },
        h('div', { class: 'quit-support-title' }, '❤  ARKLIGHT est gratuit. Il fonctionne grâce au soutien de la communauté.'),
        h('div', { class: 'note' }, 'Si ce logiciel t’est utile, un don, même modeste, nous aide à continuer à le développer.'),
        h('div', { class: 'quit-support-row' }, h('button', { class: 'support-btn', onclick: () => openSupport() }, '❤  Faire un don  →')),
      ),
      h(
        'div',
        { class: 'dialog-actions quit-actions' },
        h('button', { onclick: () => done(0) }, 'Annuler'),
        h('span', { class: 'spacer' }),
        dirty ? h('button', { class: 'danger-btn', onclick: () => done(2) }, 'Quitter sans enregistrer') : null,
        dirty ? h('button', { class: 'primary', onclick: () => done(1) }, 'Enregistrer et quitter') : h('button', { class: 'primary', onclick: () => done(2) }, 'Quitter'),
      ),
    );
    d.showModal();
  });
}

/** « Copyright © 2026 22eme ARKANE » (2026–année en cours les années suivantes). */
export function copyright() {
  const first = 2026;
  const now = new Date().getFullYear();
  return `Copyright © ${now > first ? `${first}–${now}` : first} 22eme ARKANE`;
}

/** Fenêtre « À propos » : version, licence, mises à jour, soutien. check : vérifier tout de suite. */
export function openAbout({ store, net, repo, check = false }) {
  const d = dialog('about');
  const status = h('div', { class: 'note' }, '');
  const onUpdate = (e) => {
    const m = e.detail;
    if (m.t !== 'update') return;
    status.textContent =
      m.status === 'available'
        ? `Nouvelle version disponible : ${m.latest} (tu as ${m.current}).`
        : m.status === 'up-to-date'
          ? `Tu as la dernière version (${m.current}).`
          : m.status === 'no-repo'
            ? 'Aucune version publiée pour l’instant.'
            : m.status === 'none'
              ? 'Aucune version publiée pour l’instant.'
              : `Vérification impossible (${m.message || 'pas de connexion'}).`;
  };
  net.addEventListener('message', onUpdate);
  d.addEventListener('close', () => net.removeEventListener('message', onUpdate));
  d.append(
    h('div', { class: 'sup-head' }, h('img', { src: '/img/ark-light-logo.png', alt: '' }), h('div', {}, h('h3', {}, `ARKLIGHT ${store.version || ''}`), h('div', { class: 'note' }, 'Version alpha : des choses bougent et cassent encore.'))),
    h('p', {}, 'Simulateur d’entraînement pour l’électro et la lumière : projecteurs et leurs menus, DMX (Art-Net, sACN), plan de feu, liste du matériel.'),
    h('p', { class: 'note' }, 'Logiciel libre sous licence Apache-2.0, par 22eme ARKANE. Profils DMX des autres marques : Open Fixture Library (MIT). Non affilié aux fabricants cités.'),
    h('p', { class: 'note copyright' }, copyright()),
    status,
    h(
      'div',
      { class: 'dialog-actions' },
      repo ? h('button', { onclick: () => openExternal(`https://github.com/${repo}`) }, 'Code source') : null,
      h(
        'button',
        {
          onclick: () => {
            status.textContent = 'Vérification…';
            net.send({ t: 'update.check' });
          },
        },
        'Rechercher une mise à jour',
      ),
      h('button', { class: 'support-btn', onclick: () => openSupport() }, '❤ Faire un don'),
      h('button', { class: 'primary', onclick: () => d.close() }, 'Fermer'),
    ),
  );
  d.showModal();
  if (check) {
    status.textContent = 'Vérification…';
    net.send({ t: 'update.check' });
  }
}

/** Bandeau « nouvelle version disponible » sous la barre du haut. */
export function mountUpdateBanner() {
  const bar = h('div', { class: 'update-banner hidden' });
  document.querySelector('.topbar').after(bar);
  return {
    show(m) {
      if (m.status !== 'available') return;
      bar.replaceChildren(
        h('span', { class: 'ub-icon' }, '✦'),
        h('span', {}, `ARKLIGHT ${m.latest} est disponible (tu as ${m.current}).`),
        h('button', { onclick: () => openExternal(m.download || m.url) }, 'Télécharger →'),
        m.url ? h('button', { onclick: () => openExternal(m.url) }, 'Nouveautés') : null,
        h('button', { class: 'ub-close', title: 'Masquer', onclick: () => bar.classList.add('hidden') }, '✕'),
      );
      bar.classList.remove('hidden');
    },
  };
}
