// État côté interface : scène reçue du serveur, sélection, univers DMX, état réseau.
export class Store extends EventTarget {
  constructor() {
    super();
    this.clientId = null;
    this.scene = null;
    this.fixtures = new Map();
    this.selectedId = null; // projecteur principal (inspecteur, console de test) : le dernier cliqué
    this.selection = new Set(); // tous les projecteurs sélectionnés
    this.anchorId = null; // point de départ d'une sélection Maj+clic
    this.selectedObjectId = null; // élément de grip sélectionné (exclusif avec les projecteurs)
    this.dmx = new Map(); // univers -> Uint8Array(512)
    this.liveUniverses = new Set();
    this.status = null;
    this.config = {};
    this.logs = [];
    this.history = { canUndo: false, canRedo: false, undoLabel: '', redoLabel: '', dirty: false, savedName: null };
    this.plans = { current: {}, sequences: [] }; // séquences et plans du projet (shared/plans.js)
  }

  /** Séquences et plans du projet (envoyés par le serveur). */
  setPlans(plans) {
    if (!plans) return;
    this.plans = plans;
    this.emit('plans', plans);
  }

  /** Annuler / rétablir et état « enregistrée » de la scène (envoyés par le serveur). */
  setHistory(h) {
    const { t, ...rest } = h;
    this.history = { ...this.history, ...rest };
    this.emit('history', this.history);
  }

  emit(type, detail) {
    this.dispatchEvent(new CustomEvent(type, { detail }));
  }

  setScene(scene) {
    scene.groups ||= [];
    scene.objects ||= [];
    scene.plot ||= { items: [], underlay: null, title: {} };
    if (this.selectedObjectId && !scene.objects.some((o) => o.id === this.selectedObjectId)) this.selectedObjectId = null;
    this.scene = scene;
    this.fixtures = new Map(scene.fixtures.map((f) => [f.id, f]));
    for (const id of [...this.selection]) if (!this.fixtures.has(id)) this.selection.delete(id);
    if (!this.selection.has(this.selectedId)) this.selectedId = this._last();
    this.emit('scene', scene);
    this.emit('groups', scene.groups);
    this.emit('plot', scene.plot);
    this.emit('selection', this.selected());
  }

  /**
   * Annuler / rétablir : applique un état de la scène en ne touchant qu'aux éléments qui ont changé
   * (la vue 3D garde ses modèles, les menus des projecteurs leur état).
   */
  reconcileScene(next) {
    next.groups ||= [];
    next.objects ||= [];
    next.plot ||= { items: [], underlay: null, title: {} };
    const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
    const nextFx = new Set(next.fixtures.map((f) => f.id));
    for (const f of [...this.scene.fixtures]) if (!nextFx.has(f.id)) this.removeFixture(f.id);
    for (const f of next.fixtures) if (!same(this.fixtures.get(f.id), f)) this.upsertFixture(f, null);
    this.scene.fixtures = next.fixtures.map((f) => this.fixtures.get(f.id)); // ordre de la scène
    const nextObj = new Set(next.objects.map((o) => o.id));
    for (const o of [...this.scene.objects]) if (!nextObj.has(o.id)) this.removeObject(o.id);
    for (const o of next.objects) if (!same(this.object(o.id), o)) this.upsertObject(o, null);
    this.scene.objects = next.objects.map((o) => this.object(o.id));
    if (!same(this.scene.plot, next.plot)) {
      this.scene.plot = next.plot;
      this.emit('plot', next.plot);
    }
    this.setGroups(next.groups); // redessine aussi la liste de la scène (ordre)
  }

  // ---------------------------------------------------------------- grip et accessoires
  object(id) {
    return this.scene?.objects.find((o) => o.id === id) || null;
  }

  upsertObject(o, origin) {
    const i = this.scene.objects.findIndex((x) => x.id === o.id);
    if (i >= 0) this.scene.objects[i] = o;
    else this.scene.objects.push(o);
    this.emit(i >= 0 ? 'object-updated' : 'object-added', { object: o, origin });
    if (o.id === this.selectedObjectId) this.emit('object-selection', o);
  }

  removeObject(id) {
    this.scene.objects = this.scene.objects.filter((o) => o.id !== id);
    this.emit('object-removed', id);
    if (this.selectedObjectId === id) this.selectObject(null);
  }

  /** Sélectionne un élément de grip (désélectionne les projecteurs). */
  selectObject(id) {
    if (id) this.setSelection([]);
    if (this.selectedObjectId === id) return;
    this.selectedObjectId = id;
    this.emit('object-selection', this.object(id));
  }

  selectedObject() {
    return this.object(this.selectedObjectId);
  }

  // ---------------------------------------------------------------- plan de feu
  setPlotItem(item) {
    const items = this.scene.plot.items;
    const i = items.findIndex((x) => x.id === item.id);
    if (i >= 0) items[i] = item;
    else items.push(item);
    this.emit('plot', this.scene.plot);
  }

  removePlotItem(id) {
    this.scene.plot.items = this.scene.plot.items.filter((x) => x.id !== id);
    this.emit('plot', this.scene.plot);
  }

  setPlotMeta({ underlay, title, background }) {
    this.scene.plot.underlay = underlay;
    this.scene.plot.title = title;
    if (background !== undefined) this.scene.plot.background = background;
    this.emit('plot', this.scene.plot);
  }

  upsertFixture(f, origin) {
    const isNew = !this.fixtures.has(f.id);
    this.fixtures.set(f.id, f);
    const i = this.scene.fixtures.findIndex((x) => x.id === f.id);
    if (i >= 0) this.scene.fixtures[i] = f;
    else this.scene.fixtures.push(f);
    this.emit(isNew ? 'fixture-added' : 'fixture-updated', { fixture: f, origin });
    if (f.id === this.selectedId) this.emit('selection', f);
  }

  removeFixture(id) {
    this.fixtures.delete(id);
    this.scene.fixtures = this.scene.fixtures.filter((f) => f.id !== id);
    for (const g of this.scene.groups) g.ids = g.ids.filter((x) => x !== id);
    this.emit('fixture-removed', id);
    if (this.selection.delete(id)) {
      if (this.selectedId === id) this.selectedId = this._last();
      this.emit('selection', this.selected());
    }
  }

  setGroups(groups) {
    this.scene.groups = groups;
    this.emit('groups', groups);
  }

  group(id) {
    return this.scene?.groups.find((g) => g.id === id) || null;
  }

  // ---------------------------------------------------------------- sélection
  _last() {
    return [...this.selection].pop() ?? null;
  }

  /** Remplace la sélection. primary : projecteur affiché dans l'inspecteur (par défaut le dernier). */
  setSelection(ids, primary) {
    const next = new Set(ids.filter((id) => this.fixtures.has(id)));
    if (next.size && this.selectedObjectId) {
      this.selectedObjectId = null;
      this.emit('object-selection', null);
    }
    const p = primary && next.has(primary) ? primary : [...next].pop() ?? null;
    if (p === this.selectedId && next.size === this.selection.size && [...next].every((id) => this.selection.has(id))) return;
    this.selection = next;
    this.selectedId = p;
    this.emit('selection', this.selected());
  }

  /** Sélection simple (null : tout désélectionner). */
  select(id) {
    this.anchorId = id;
    this.setSelection(id ? [id] : [], id);
  }

  /** Ctrl+clic : ajoute ou retire un projecteur. */
  toggleSelect(id) {
    this.anchorId = id;
    if (this.selection.has(id)) {
      const rest = [...this.selection].filter((x) => x !== id);
      this.setSelection(rest, this.selectedId === id ? null : this.selectedId);
    } else this.setSelection([...this.selection, id], id);
  }

  /** Maj+clic : plage depuis le dernier projecteur cliqué, dans l'ordre affiché (par défaut, celui de la scène). */
  selectRange(id, order = this.scene.fixtures.map((f) => f.id)) {
    const a = order.indexOf(this.anchorId ?? this.selectedId);
    const b = order.indexOf(id);
    if (a < 0 || b < 0) return this.select(id);
    this.setSelection(order.slice(Math.min(a, b), Math.max(a, b) + 1), id);
  }

  isSelected(id) {
    return this.selection.has(id);
  }

  /** Identifiants sélectionnés, dans l'ordre de la scène. */
  selectedIds() {
    return (this.scene?.fixtures || []).filter((f) => this.selection.has(f.id)).map((f) => f.id);
  }

  selectedFixtures() {
    return (this.scene?.fixtures || []).filter((f) => this.selection.has(f.id));
  }

  /** Projecteur principal de la sélection. */
  selected() {
    return this.selectedId ? this.fixtures.get(this.selectedId) || null : null;
  }

  // ---------------------------------------------------------------- environnement, DMX, réseau
  setEnvironment(env) {
    this.scene.environment = env;
    this.emit('env', env);
  }

  setDmx(universe, data) {
    let buf = this.dmx.get(universe);
    if (!buf) {
      buf = new Uint8Array(512);
      this.dmx.set(universe, buf);
    }
    buf.set(data);
    this.emit('dmx', universe);
  }

  /** Univers dont on oublie le dernier état reçu : les projecteurs en auto reviennent à leurs réglages. */
  forgetDmx(universes) {
    for (const u of universes) if (this.dmx.delete(u)) this.emit('dmx', u);
    this.emit('status', this.status);
  }

  setStatus(status) {
    this.status = status;
    this.liveUniverses = new Set((status.sources || []).flatMap((s) => s.universes));
    this.emit('status', status);
  }

  addLog(entry) {
    this.logs.push(entry);
    if (this.logs.length > 300) this.logs.shift();
    this.emit('log', entry);
  }
}
