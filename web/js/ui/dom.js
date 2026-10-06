// Petits utilitaires DOM.

/** Crée un élément : h('div', { class: 'x', onclick: fn }, 'texte', enfant) */
export function h(tag, attrs = {}, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v === undefined || v === null || v === false) continue;
    if (k === 'class') el.className = v;
    else if (k === 'style' && typeof v === 'object') {
      for (const [p, x] of Object.entries(v)) {
        if (p.startsWith('--')) el.style.setProperty(p, x); // variables CSS (couleur d'une touche…)
        else el.style[p] = x;
      }
    }
    else if (k.startsWith('on') && typeof v === 'function') el.addEventListener(k.slice(2), v);
    else if (k === 'value') el.value = v;
    else if (k === 'checked') el.checked = !!v;
    else if (k === 'html') el.innerHTML = v;
    else el.setAttribute(k, v === true ? '' : v);
  }
  for (const c of children.flat(Infinity)) {
    if (c === undefined || c === null || c === false) continue;
    el.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
  return el;
}

let toastTimer = null;
export function toast(text, kind = 'info', ms = 2600) {
  const el = document.getElementById('toast');
  el.textContent = text;
  el.className = `toast ${kind === 'error' ? 'error' : ''}`;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.add('hidden'), ms);
}

/** Met à jour la valeur d'un champ sauf si l'utilisateur est en train de le modifier. */
export function setFieldValue(input, value) {
  if (document.activeElement === input) return;
  if (input.type === 'checkbox') input.checked = !!value;
  else input.value = value;
}

export const deg = (rad) => Math.round((rad * 180) / Math.PI * 10) / 10;
export const rad = (d) => (Number(d) * Math.PI) / 180;
export const pad3 = (n) => String(n).padStart(3, '0');
