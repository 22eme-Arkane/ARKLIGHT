// Petites fenêtres de dialogue : saisie d'un nom (Electron n'a pas window.prompt), enregistrement
// des modifications avant d'ouvrir ou de créer un autre projet.
import { h } from './dom.js';

function modal(cls) {
  const d = h('dialog', { class: `dialog ${cls}` });
  document.body.append(d);
  d.addEventListener('close', () => setTimeout(() => d.remove(), 0));
  return d;
}

/** Demande un texte. Renvoie le texte saisi (nettoyé) ou null si on annule. */
export function askText({ title, label = '', value = '', ok = 'OK', note = '' }) {
  return new Promise((resolve) => {
    const d = modal('ask');
    let result = null;
    const input = h('input', { class: 'ask-input', value, spellcheck: 'false' });
    const submit = () => {
      const v = input.value.replace(/\s+/g, ' ').trim();
      if (!v) return input.focus();
      result = v;
      d.close();
    };
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        submit();
      }
    });
    d.addEventListener('close', () => resolve(result));
    d.append(
      h('h3', {}, title),
      label ? h('label', { class: 'ask-label' }, label) : null,
      input,
      note ? h('p', { class: 'note' }, note) : null,
      h('div', { class: 'dialog-actions' }, h('button', { onclick: () => d.close() }, 'Annuler'), h('button', { class: 'primary', onclick: submit }, ok)),
    );
    d.showModal();
    input.focus();
    input.select();
  });
}

/** Demande une confirmation. Renvoie true si on confirme. */
export function askConfirm({ title, note = '', ok = 'OK', danger = false }) {
  return new Promise((resolve) => {
    const d = modal('ask');
    let result = false;
    d.addEventListener('close', () => resolve(result));
    const okBtn = h(
      'button',
      {
        class: danger ? 'danger-btn' : 'primary',
        onclick: () => {
          result = true;
          d.close();
        },
      },
      ok,
    );
    d.append(h('h3', {}, title), note ? h('p', { class: 'note' }, note) : null, h('div', { class: 'dialog-actions' }, h('button', { onclick: () => d.close() }, 'Annuler'), okBtn));
    d.showModal();
    okBtn.focus();
  });
}

/** Modifications non enregistrées : 'save' · 'discard' · 'cancel'. */
export function askSaveChanges(sceneName, action = 'continuer') {
  return new Promise((resolve) => {
    const d = modal('ask');
    let result = 'cancel';
    const done = (r) => {
      result = r;
      d.close();
    };
    d.addEventListener('close', () => resolve(result));
    d.append(
      h('h3', {}, `Enregistrer les modifications de « ${sceneName} » ?`),
      h('p', { class: 'note' }, `Elles seront perdues si tu ne les enregistres pas avant de ${action}.`),
      h(
        'div',
        { class: 'dialog-actions quit-actions' },
        h('button', { onclick: () => done('cancel') }, 'Annuler'),
        h('span', { class: 'spacer' }),
        h('button', { class: 'danger-btn', onclick: () => done('discard') }, 'Ne pas enregistrer'),
        h('button', { class: 'primary', onclick: () => done('save') }, 'Enregistrer'),
      ),
    );
    d.showModal();
  });
}
