// Panneau "Commandes du projecteur" : réplique des claviers, écrans OLED et boutons embarqués.
import { OnboardController } from '/shared/onboard/engine.js';
import { AsteraV5Controller } from '/shared/onboard/astera-v5.js';
import { AsteraAx1Controller } from '/shared/onboard/astera-ax1.js';
import { AsteraBoxController } from '/shared/onboard/astera-box.js';
import { h } from './dom.js';

export const now = () => performance.now() / 1000;

/** Registre des contrôleurs embarqués et de l'état d'exécution de chaque projecteur (partagé avec la vue 3D). */
export class OnboardRegistry {
  constructor({ store, net }) {
    this.store = store;
    this.net = net;
    this.controllers = new Map();
    this.runtimes = new Map();
    store.addEventListener('fixture-removed', (e) => {
      this.controllers.delete(e.detail);
      this.runtimes.delete(e.detail);
    });
    store.addEventListener('scene', () => {
      this.controllers.clear();
      this.runtimes.clear();
    });
  }

  /** État d'exécution local (verrou d'entrée, effets…) : non sauvegardé, propre à cette fenêtre. */
  runtime(id) {
    let rt = this.runtimes.get(id);
    if (!rt) {
      rt = {};
      this.runtimes.set(id, rt);
    }
    return rt;
  }

  get(fixture, type) {
    let c = this.controllers.get(fixture.id);
    if (c) return c;
    const id = fixture.id;
    const opts = {
      type,
      getFixture: () => this.store.fixtures.get(id),
      emit: (changes) => this._apply(id, changes),
      rt: this.runtime(id),
    };
    if (type.platform === 'astera-v5-oled') c = new AsteraV5Controller(opts);
    else if (type.platform === 'astera-v5-button') c = new AsteraAx1Controller(opts);
    else if (type.platform === 'astera-box')
      c = new AsteraBoxController({
        ...opts,
        getAll: () => this.store.scene?.fixtures || [],
        emitTo: (otherId, changes, o = {}) => {
          this._apply(otherId, changes);
          if (o.relatch) {
            const rt = this.runtime(otherId);
            rt.latch = null;
            rt.justUnlinked = !!o.unlinked;
          }
        },
        env: () => ({ appConnected: (this.store.ble?.phones?.length || 0) > 0, dmxLive: (u) => this.store.liveUniverses.has(u) }),
      });
    else if (type.controls?.buttons) c = new OnboardController(opts);
    else return null;
    this.controllers.set(id, c);
    return c;
  }

  peek(id) {
    return this.controllers.get(id) || null;
  }

  _apply(id, changes) {
    const f = this.store.fixtures.get(id);
    if (!f) return;
    if (changes.settings) Object.assign(f.settings, changes.settings);
    if (changes.state) Object.assign(f.state, changes.state);
    this.store.upsertFixture(f, this.store.clientId);
    this.net.send({ t: 'fixture.update', id, changes });
  }
}

// ------------------------------------------------------------------ écran OLED
const OLED_ON = '#d6f2ff';
const OLED_DIM = '#557482';

function drawBattery(g, x, y, level, color) {
  g.strokeStyle = color;
  g.lineWidth = 1;
  g.strokeRect(x + 0.5, y + 1.5, 6, 9);
  g.fillStyle = color;
  g.fillRect(x + 2, y, 3, 1.5);
  const hgt = Math.round((Math.max(0, Math.min(100, level)) / 100) * 8);
  g.fillRect(x + 1.5, y + 10 - hgt, 4, hgt);
}

function drawPlug(g, x, y, color) {
  g.fillStyle = color;
  g.fillRect(x + 2, y + 1, 1, 3);
  g.fillRect(x + 5, y + 1, 1, 3);
  g.fillRect(x + 1, y + 4, 6, 3);
  g.fillRect(x + 3, y + 7, 2, 3);
}

function drawBt(g, x, y, waves, color) {
  g.strokeStyle = color;
  g.lineWidth = 1;
  g.beginPath();
  g.moveTo(x + 1, y + 3);
  g.lineTo(x + 6, y + 8);
  g.lineTo(x + 3.5, y + 10.5);
  g.lineTo(x + 3.5, y + 0.5);
  g.lineTo(x + 6, y + 3);
  g.lineTo(x + 1, y + 8);
  g.stroke();
  if (waves) {
    g.beginPath();
    g.arc(x + 7, y + 5.5, 3, -0.9, 0.9);
    g.moveTo(x + 11, y + 1.5);
    g.arc(x + 7, y + 5.5, 5.5, -0.8, 0.8);
    g.stroke();
  }
}

/** Afficheur LCD 2 lignes rétroéclairé bleu (AX10) : texte sombre, icône batterie à 3 barres. */
function drawLcd(canvas, model) {
  const g = canvas.getContext('2d');
  const W = canvas.width;
  const H = canvas.height;
  const lit = model && !model.off;
  g.fillStyle = lit ? '#8fc9f5' : '#2b3a46';
  g.fillRect(0, 0, W, H);
  if (!lit) return;
  const ink = '#0d2236';
  g.textBaseline = 'top';
  const textW = W - 12;
  (model.lines || []).slice(0, 2).forEach((ln, i) => {
    let size = 12;
    g.font = `bold ${size}px Consolas, monospace`;
    while (g.measureText(ln.text).width > textW - 2 && size > 7) g.font = `bold ${--size}px Consolas, monospace`;
    const y = 2 + i * 15;
    if (ln.inverse) {
      g.fillStyle = ink;
      g.fillRect(0, y - 1, textW, 14);
      g.fillStyle = '#8fc9f5';
    } else g.fillStyle = ink;
    g.fillText(ln.text, 2, y);
  });
  // icône batterie : 3 barres (animées en charge)
  const ic = model.icons;
  if (ic) {
    g.strokeStyle = ink;
    g.strokeRect(W - 9.5, 3.5, 7, 12);
    g.fillStyle = ink;
    g.fillRect(W - 7, 1.5, 2, 2);
    const bars = ic.plug ? Math.floor(performance.now() / 500) % 4 : ic.battery >= 80 ? 3 : ic.battery >= 50 ? 2 : ic.battery >= 15 ? 1 : 0;
    for (let i = 0; i < bars; i++) g.fillRect(W - 8, 12.5 - i * 3.5, 4, 2.5);
  }
}

export function drawOled(canvas, model, opts = {}) {
  if (opts.lcd) return drawLcd(canvas, model);
  const g = canvas.getContext('2d');
  const W = canvas.width;
  const H = canvas.height;
  g.fillStyle = '#000';
  g.fillRect(0, 0, W, H);
  if (!model || model.off) return;
  const color = model.dim ? OLED_DIM : OLED_ON;
  g.textBaseline = 'top';
  let top = 2;
  if (model.icons) {
    const ic = model.icons;
    if (ic.bt) drawBt(g, 1, 1, ic.btWaves, color);
    g.font = 'bold 10px Consolas, monospace';
    g.fillStyle = color;
    const pct = `${ic.battery}%`;
    const w = g.measureText(pct).width;
    drawBattery(g, W - 8, 1, ic.battery, color);
    g.fillText(pct, W - 11 - w, 2);
    if (ic.plug) drawPlug(g, W - 22 - w, 1, color);
    top = 16;
  }
  const big = model.size !== 'small';
  const n = model.lines?.length || 0;
  const lineH = big ? 21 : Math.floor((H - top) / Math.max(4, n));
  (model.lines || []).forEach((ln, i) => {
    let size = big ? 14 : 11;
    g.font = `bold ${size}px Consolas, monospace`;
    while (g.measureText(ln.text).width > W - 4 && size > 7) {
      size -= 1;
      g.font = `bold ${size}px Consolas, monospace`;
    }
    const y = top + i * lineH;
    if (ln.inverse) {
      g.fillStyle = color;
      g.fillRect(0, y - 1, W, lineH - 1);
      g.fillStyle = '#000';
    } else g.fillStyle = color;
    g.fillText(ln.text, 2, y + (big ? 1 : 0));
  });
}

/** TouchSlider de l'HydraPanel : appui en haut = +1, au milieu = ENTER, en bas = −1 ; glisser = défilement. */
function touchSlider(ctrl) {
  const strip = h('div', { class: 'touch-slider', title: 'TouchSlider : haut = +1, milieu = ENTER, bas = −1, glisser pour défiler' }, h('span', { class: 'ts-plus' }, '+'), h('span', { class: 'ts-ticks' }), h('span', { class: 'ts-minus' }, '−'));
  let drag = null;
  strip.addEventListener('pointerdown', (e) => {
    strip.setPointerCapture(e.pointerId);
    drag = { y0: e.clientY, last: e.clientY, moved: 0 };
  });
  strip.addEventListener('pointermove', (e) => {
    if (!drag) return;
    const dy = e.clientY - drag.last;
    if (Math.abs(dy) >= 8) {
      const steps = Math.floor(Math.abs(dy) / 8);
      ctrl.slider(dy < 0 ? 'top' : 'bottom', now(), steps);
      drag.last += Math.sign(dy) * steps * 8;
      drag.moved += steps;
    }
  });
  strip.addEventListener('pointerup', (e) => {
    if (drag && !drag.moved) {
      const r = strip.getBoundingClientRect();
      const y = (e.clientY - r.top) / r.height;
      ctrl.slider(y < 0.33 ? 'top' : y > 0.67 ? 'bottom' : 'middle', now());
    }
    drag = null;
  });
  return strip;
}

// ------------------------------------------------------------------ panneau
export function createControlPanel({ registry }) {
  let mounted = null;
  let raf = 0;

  function loop() {
    raf = requestAnimationFrame(loop);
    if (!mounted) return;
    const { ctrl, canvas, holdBars, msg, leds } = mounted;
    const t = now();
    if (canvas) drawOled(canvas, ctrl.display(t), { lcd: mounted.type.controls?.display?.kind === 'lcd' });
    for (const [id, bar] of holdBars) {
      const held = ctrl.heldFor(id, t);
      bar.style.width = `${Math.min(100, (held / 5) * 100)}%`;
      bar.nextSibling.textContent = held ? `${held.toFixed(1)} s` : '';
    }
    if (leds) {
      const states = ctrl.leds?.(t) || {};
      for (const [id, el] of leds) {
        const s = states[id];
        el.style.background = s?.color || '#1b1f24';
        el.style.boxShadow = s?.color ? `0 0 8px ${s.color}` : 'none';
        el.style.animation = s?.blinkHz ? `ledblink ${1 / s.blinkHz}s steps(2, start) infinite` : 'none';
      }
    }
    msg.textContent = ctrl.hint?.() || ctrl.message?.text || '';
  }

  function keyButton(ctrl, holdBars, k, extraClass = '') {
    const bar = h('i');
    holdBars.set(k.id, bar);
    const btn = h('button', { class: `device-btn ${k.id} ${extraClass}`, title: k.title || k.label }, k.label, h('span', { class: 'hold' }, bar, h('em')));
    const down = (e) => {
      e.preventDefault();
      btn.setPointerCapture?.(e.pointerId);
      btn.classList.add('down');
      ctrl.press(k.id, now());
    };
    const up = () => {
      if (!btn.classList.contains('down')) return;
      btn.classList.remove('down');
      ctrl.release(k.id, now());
    };
    btn.addEventListener('pointerdown', down);
    btn.addEventListener('pointerup', up);
    btn.addEventListener('pointercancel', up);
    return btn;
  }

  function render(f, type) {
    cancelAnimationFrame(raf);
    mounted = null;
    const box = h('div', { class: 'group onboard' }, h('h4', {}, 'Commandes du projecteur'));
    const ctrl = registry.get(f, type);
    if (!ctrl) {
      const text =
        type.platform === 'astera-v5-none'
          ? 'Aucune touche sur l’ampoule : elle s’allume dès que la douille est alimentée et ne peut pas être éteinte tant qu’elle l’est. BlueMode : télécommande ARC3 (BLUE MODE 1/2 puis 2/2 en moins de 5 s) ou PrepInlay / PrepCase.'
          : type.controlsPending
            ? 'Clavier et menus de ce modèle : documentation en cours d’intégration.'
            : type.technology === 'tungsten'
              ? 'Projecteur tungstène : pas de menu. Il se pilote par un gradateur (1 canal DMX) ; plus on baisse, plus la lumière se réchauffe, avec un léger retard du filament.'
              : type.technology === 'hmi'
                ? 'HMI : allumage et gradation (50 à 100 %) par le ballast. Après l’allumage, la lampe met environ une minute à monter en régime. Ces comportements sont approximatifs.'
                : type.platform === 'generic'
                  ? 'Menus embarqués de ce modèle : pas encore simulés. Il se pilote en DMX avec ses profils officiels.'
                  : 'Pas de commandes embarquées sur ce modèle.';
      box.append(h('div', { class: 'note' }, text));
      return box;
    }
    const def = type.controls || {};
    const keys = new Map((def.keys || def.buttons || []).map((k) => [k.id, k]));
    const holdBars = new Map();
    const ledEls = new Map();
    const btn = (id, cls) => (keys.has(id) ? keyButton(ctrl, holdBars, keys.get(id), cls) : null);
    let canvas = null;
    const oled = () => {
      canvas = h('canvas', { class: 'device-display', width: def.display?.width || 128, height: def.display?.height || 64 });
      return canvas;
    };
    let face;
    if (def.keyfoil === 'tube') {
      face = h(
        'div',
        { class: 'keyfoil tube' },
        h('div', { class: 'kf-row' }, btn('power', 'round'), h('span', { class: 'ir-window', title: 'Récepteur infrarouge' }), oled(), h('div', { class: 'kf-col' }, btn('palette', 'icon'), btn('sun', 'icon'))),
        h('div', { class: 'kf-row keys4' }, h('span', { class: 'astera-logo' }, '▲ ASTERA'), btn('menu'), btn('minus'), btn('plus'), btn('enter')),
        h('div', { class: 'kf-url' }, 'www.astera-led.com'),
      );
    } else if (def.keyfoil === 'ax2') {
      face = h(
        'div',
        { class: 'keyfoil ax2' },
        h('div', { class: 'kf-row' }, btn('power', 'round'), btn('astera', 'logo'), btn('sun', 'icon')),
        h('div', { class: 'kf-row' }, h('span', { class: 'ir-window', title: 'Récepteur infrarouge (probable)' }), oled(), btn('palette', 'icon')),
        h('div', { class: 'kf-row keys4' }, btn('menu'), btn('minus'), btn('plus'), btn('enter')),
        h('div', { class: 'kf-url' }, 'www.astera-led.com'),
      );
    } else if (def.keyfoil === 'v5square') {
      // AX9, PixelBrick, HydraPanel : même clavier que les tubes, disposition carrée (+ TouchSlider sur l'HydraPanel)
      let slider = null;
      if (def.slider) slider = touchSlider(ctrl);
      face = h(
        'div',
        { class: 'kf-with-slider' },
        h(
          'div',
          { class: 'keyfoil ax2' },
          h('div', { class: 'kf-row' }, btn('power', 'round'), h('span', { class: 'astera-logo' }, '▲ ASTERA'), btn('sun', 'icon')),
          h('div', { class: 'kf-row' }, h('span', { class: 'ir-window', title: 'Récepteur infrarouge (probable)' }), oled(), btn('palette', 'icon')),
          h('div', { class: 'kf-row keys4' }, btn('menu'), btn('minus'), btn('plus'), btn('enter')),
          h('div', { class: 'kf-url' }, 'www.astera-led.com'),
        ),
        slider,
      );
    } else if (def.keyfoil === 'quik') {
      const status = h('span', { class: 'led', title: 'Voyant d’état (documenté seulement dans un ChargingPlate avec PrepBox)' });
      ledEls.set('status', status);
      face = h(
        'div',
        { class: 'keyfoil ax2' },
        h('div', { class: 'kf-row' }, btn('power', 'round'), btn('astera', 'round'), status, btn('sun', 'icon')),
        h('div', { class: 'kf-row' }, h('span', { class: 'ir-window', title: 'Récepteur infrarouge' }), oled(), btn('palette', 'icon')),
        h('div', { class: 'kf-row keys4' }, btn('menu'), btn('minus'), btn('plus'), btn('enter')),
        h('div', { class: 'kf-url' }, 'www.astera-led.com'),
      );
    } else if (def.keyfoil === 'dual') {
      // AX5, AX7, AX10 : LCD/afficheur 2 lignes, voyant de batterie, ⏻ et 4 touches à double légende
      const ledDot = h('span', { class: 'led' });
      ledEls.set('battery', ledDot);
      const dualKey = (id) => {
        const k = keys.get(id);
        const b = btn(id, 'dual-key');
        if (b && k.alt) b.insertBefore(h('small', { class: 'alt' }, k.alt), b.querySelector('.hold'));
        return b;
      };
      face = h(
        'div',
        { class: 'keyfoil ax2 dual' },
        h('span', { class: 'astera-logo' }, '▲ ASTERA'),
        h('div', { class: 'kf-row' }, oled(), h('div', { class: 'kf-col' }, h('span', { class: 'led-wrap', title: 'Voyant de batterie' }, ledDot, h('small', {}, 'BAT')), h('span', { class: 'ir-window', title: 'Récepteur infrarouge' })), btn('power', 'round')),
        h('div', { class: 'kf-row keys4' }, dualKey('menu'), dualKey('minus'), dualKey('plus'), dualKey('enter')),
      );
    } else if (def.keyfoil === 'art7') {
      // Face supérieure : côté BTB (⏻ + APP) | jauge | colonne LINK/DMX/RX/TX | touche verte CRMX
      const led = (id, label) => {
        const dot = h('span', { class: 'led' });
        ledEls.set(id, dot);
        return h('span', { class: 'led-wrap' }, dot, h('small', {}, label));
      };
      const gauge = h('div', { class: 'bat-gauge', title: 'Batterie (5 segments)' });
      for (let i = 4; i >= 0; i--) {
        const seg = h('span', { class: 'seg' });
        ledEls.set(`bat${i}`, seg);
        gauge.append(seg);
      }
      face = h(
        'div',
        { class: 'art7-top' },
        h('div', { class: 'art7-side' }, h('small', { class: 'art7-title' }, 'BTB'), led('app', 'APP'), btn('power', 'round')),
        gauge,
        h('div', { class: 'art7-leds' }, led('link', 'LINK'), led('dmx', 'DMX'), led('rx', 'RX'), led('tx', 'TX')),
        h('div', { class: 'art7-side' }, h('small', { class: 'art7-title' }, 'CRMX'), btn('link', 'round link-key')),
      );
    } else if (def.keyfoil === 'bulb') {
      // NYX / SolaBulb : prise DC et touche ON/OFF dans un renfoncement du culot
      face = h('div', { class: 'bulb-strip' }, h('span', { class: 'dc-socket', title: 'Prise DC' }), btn('power', 'round small'));
    } else if (def.keyfoil === 'ax3') {
      face = h(
        'div',
        { class: 'endcap' },
        h('div', { class: 'cap-face ax3-back', title: 'Face arrière : aimant et bouton ON/OFF central' }, h('span', { class: 'magnet' }), btn('power', 'round small center')),
        h('div', { class: 'kf-row' }, h('small', { class: 'note' }, 'Bouton latéral :'), btn('power-side', 'round small')),
      );
    } else if (def.keyfoil === 'ax1') {
      face = h(
        'div',
        { class: 'endcap' },
        h('div', { class: 'cap-face' }, h('span', { class: 'cap-plug', title: 'Prise de charge (bouchon IP65)' }), h('span', { class: 'cap-thread', title: 'Filetage M5' }), btn('power', 'round small')),
        h('button', { class: 'shake-btn', title: 'Simule l’accéléromètre : secouer le tube de haut en bas', onclick: () => ctrl.shake() }, '↕ Secouer le tube'),
      );
    } else {
      // Contrôleur générique
      face = h('div', { class: 'device-face' }, h('div', { class: 'device-buttons' }, [...keys.keys()].map((id) => btn(id, keys.get(id).shape || ''))));
    }
    const msg = h('div', { class: 'device-msg' });
    box.append(face, msg);
    if (def.keyfoil === 'tube') box.append(h('div', { class: 'note' }, 'Disposition schématique : sur le tube, le clavier est une bande au dos, près du pixel 1.'));
    mounted = { f, type, ctrl, canvas, holdBars, msg, leds: ledEls.size ? ledEls : null };
    loop();
    return box;
  }

  return {
    render,
    refresh() {},
    unmount() {
      cancelAnimationFrame(raf);
      mounted = null;
    },
  };
}
