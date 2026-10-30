// Dedolight DLED4-BI / DLED7-BI : tout se règle sur le ballast DT4-BI-DMX / DT7-BI-DMX
// (docs/recherche/menus/dedo_dmg_kino_litegear.md). Pas de menu : afficheur 3 chiffres ambre,
// boutons jaunes ↓ et ↑, molettes DIM et COLOR (potentiomètres), voyants HEAT et ON.
//  - Mode DMX : l'adresse (001–512) ; point allumé = signal DMX valide.
//  - ↓ + ↑ ensemble : l'adresse clignote, ↑ / ↓ la changent ; sous 001 → mode manuel.
//  - Mode manuel : les molettes agissent (d.00–d.99, c.27–c.67) ; ↓ + ↑ puis ↓ = adresse 512, ↑ = 001.
import { pad } from '../panel.js';
import { LED, cctRange, clamp } from './common.js';

const EDIT_S = 6; // « quelques secondes » sans appui : l'adresse est enregistrée

const manual = (c) => c.panel('manual', false);
function setManualMode(c, on) {
  c.setPanel({ manual: on });
  c.setSettings({ source: on ? 'manual' : 'auto' });
}

export const dedolight = {
  id: 'dedolight-dt-dmx',
  display: { w: 60, h: 26, zoom: 2.4, palette: { bg: '#0b0604', fg: '#ffa21f', dim: '#3a2209', accent: '#ffa21f', off: '#0b0604' } },
  face: {
    style: 'dedo',
    rows: [
      [{ el: 'led', id: 'heat', label: 'HEAT' }, { el: 'led', id: 'on', label: 'ON' }],
      [{ el: 'knob', id: 'dim', label: 'DIM', pointer: true }, { el: 'knob', id: 'color', label: 'COLOR', pointer: true }],
      [{ el: 'text', text: 'DMX 512 / 1990' }],
      [{ el: 'key', id: 'down', label: '↓', shape: 'small yellow' }, { el: 'display' }, { el: 'key', id: 'up', label: '↑', shape: 'small yellow' }],
      [{ el: 'text', text: '◄ SIGNAL' }, { el: 'key', id: 'both', label: '↓ + ↑ ensemble', shape: 'small' }, { el: 'key', id: 'power', label: 'I / O', shape: 'rect' }],
    ],
    note: '« ↓ + ↑ ensemble » remplace l’appui simultané sur les deux boutons jaunes : réglage de l’adresse. Sous 001 : mode manuel, les molettes agissent.',
  },
  init: () => ({ edit: 0, exiting: 0, show: 'd', bothAt: -9 }),
  down(c, id) {
    // les deux boutons ensemble (ou l'un juste après l'autre)
    const other = id === 'up' ? 'down' : id === 'down' ? 'up' : null;
    if (other && (c.pressed.has(other) || c.t - (c.st.lastKey?.t ?? -9) < 0.25)) {
      c.st.bothAt = c.t;
      c.st.skip = true;
    }
    c.st.lastKey = { id, t: c.t };
  },
  key(c, id) {
    if (id === 'power') return c.setState({ power: !c.on });
    if (!c.on) return;
    if (id === 'both') {
      c.st.skip = true;
      c.st.bothAt = c.t;
    }
    if (c.st.skip && c.t - c.st.bothAt < 0.6) {
      // ↓ + ↑ : réglage de l'adresse, ou sortie du mode manuel
      c.st.skip = false;
      if (manual(c)) c.st.exiting = c.t + EDIT_S;
      else c.st.edit = c.t + EDIT_S;
      return;
    }
    c.st.skip = false;
    if (c.st.exiting > c.t) {
      // sortie du mode manuel : ↓ = adresse 512, ↑ = adresse 001
      c.st.exiting = 0;
      setManualMode(c, false);
      c.setSettings({ dmxAddress: id === 'down' ? Math.min(512, 513 - 2) : 1 });
      return c.say(id === 'down' ? 'Mode DMX, adresse 512 (511 avec 2 canaux)' : 'Mode DMX, adresse 001');
    }
    if (c.st.edit > c.t) {
      c.st.edit = c.t + EDIT_S;
      const a = c.s.dmxAddress + (id === 'up' ? 1 : -1);
      if (a < 1) {
        c.st.edit = 0;
        setManualMode(c, true);
        return c.say('Mode manuel : les molettes DIM et COLOR agissent');
      }
      return c.setSettings({ dmxAddress: clamp(a, 1, 511) });
    }
    c.say('↓ et ↑ ensemble pour régler l’adresse');
  },
  turn(c, id, steps) {
    if (!c.on) return;
    if (!manual(c)) return c.say('Molettes inactives en mode DMX (adresse sous 001 pour le manuel)');
    if (id === 'dim') {
      c.st.show = 'd';
      return c.setManual({ intensity: clamp(Math.round((c.m.intensity ?? 0) * 100 + steps) / 100, 0, 0.99) });
    }
    const [lo, hi] = cctRange(c, [2700, 6500]);
    c.st.show = 'c';
    c.setManual({ mode: 'cct', cct: clamp(Math.round(((c.m.cct || 5600) + steps * 100) / 100) * 100, Math.max(2700, lo), Math.min(6700, hi)) });
  },
  draw(c) {
    const blink = (c.st.edit > c.t || c.st.exiting > c.t) && Math.floor(c.t * 3) % 2 === 0;
    let text;
    let dot = false;
    if (manual(c) && !(c.st.exiting > c.t)) {
      text = c.st.show === 'c' ? `c.${String(Math.round((c.m.cct || 5600) / 100)).padStart(2, '0')}` : `d.${String(Math.round((c.m.intensity ?? 0) * 100)).padStart(2, '0')}`;
    } else {
      text = manual(c) ? 'd.--' : pad(c.s.dmxAddress);
      dot = c.dmxLive() && !manual(c);
    }
    if (blink) return { prims: [] };
    return { prims: [{ t: 'text', x: 30, y: 2, text: `${text}${dot ? '.' : ''}`, s: 19, align: 'c', bold: true, font: 'seg', spacing: 2 }] };
  },
  leds(c) {
    // ON : jaune en DMX 8 bits (vert en 16 bits) ; HEAT : surchauffe (jamais simulée)
    return { on: c.on ? { color: LED.yellow } : null, heat: null };
  },
};
