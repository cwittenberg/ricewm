import { Column, Label, TextField, Button, ScrollView, CheckBox, GroupBox, MenuItem, Menu } from "./controls.mjs";
import { State } from './control.mjs';
import { NumberInput, EditableChoice, ColorInput } from './value-editors.mjs';
import { FormModel, Form } from "./forms.mjs";
import { Message } from "./messages.mjs";
import { TaskController } from "./tasks.mjs";
import { WrapPanel, wrapLayout } from './wrap-panel.mjs';
import { Control } from './control.mjs';

// Transient gallery artwork has one owner per retained window root. Other
// windows keep independent previews and keyboard scopes.
const galleryPreviews = new WeakMap();

/** Transaction boundaries remain in the app's shared command/domain model. */
export class PropertyInspector extends Column {
  static family = "composite";
  static constructorOptions = Object.freeze([
    "properties",
    "selection",
    "read",
    "apply",
    "commitMode",
    "rows",
  ]);
  static authoring = Object.freeze({
    constructor:
      'new PropertyInspector({properties:[{id,label,type:"text"|"number"|"boolean"|"enum"|"color"|"custom",group?,layout?:"stacked"|"inline",labelWidth?,controlWidth?,readOnly?,choices?,unit?,step?,precision?,minimum?,maximum?,createEditor?,validate?}],rows?:[["first","second"],["name"]],selection:[ids],read:(id,property)=>value,apply:async({ids,values})=>{...},commitMode:"explicit"|"field"})',
    methods:
      "setSelection(ids,{preserveDraft:true}), commit(propertyId?), cancel(); grouped native fields, mixed values, read-only properties and custom editors. Default explicit mode validates all changed properties with one Apply transaction. Field mode hides Apply and commits each completed field: Enter/blur, number steppers or completed choices. Other drafts/errors remain independent. Numeric parsing uses NumberInput's decimal/scientific rules. Pending work cannot submit twice. Rejected changes retain drafts and inline errors. Selection changes invalidate pending validation and never retarget an in-flight apply.",
  });
  constructor({
    properties = [],
    selection = [],
    read,
    apply,
    commitMode = 'explicit',
    rows = null,
    ...options
  } = {}) {
    if (
      !Array.isArray(properties) ||
      properties.length > 128 ||
      typeof read !== "function" ||
      typeof apply !== "function" || !['explicit', 'field'].includes(commitMode) || rows!==null&&properties.some(p=>p.group)
    )
      throw TypeError("Invalid property inspector");
    const fields = properties.map((p) => {
      if (!["text", "number", "boolean", "enum", "color", "custom"].includes(p.type))
        throw TypeError("Unsupported property type");
      if (p.group != null && (typeof p.group !== 'string' || p.group.length > 80)) throw TypeError('Invalid property group');
      if (p.type === 'enum' && (!Array.isArray(p.choices) || !p.choices.length || p.choices.some(v => typeof v !== 'string' || !v))) throw TypeError('Enum properties require choices');
      if (p.type === 'custom' && (typeof p.createEditor !== 'function' || !Object.hasOwn(p, 'initial'))) throw TypeError('Custom properties need createEditor and an initial value');
      return {
        id: p.id,
        label: p.label,
        initial: Object.hasOwn(p, 'initial') ? structuredClone(p.initial) : p.type === "boolean" ? false : p.type === 'number' ? 0 : p.type === 'color' ? '#000000' : "",
        validate: (value) =>
          p.readOnly ? 'This property is read-only' : p.type === 'enum' && !p.choices.includes(value) ? 'Select a listed value' : p.type === 'color' && value !== null && !/^#[0-9a-f]{6}$/iu.test(value) ? 'Enter a color as #RRGGBB' : p.type === "number" &&
          (!Number.isFinite(Number(value)) ||
            Number(value) < (p.minimum ?? -Infinity) ||
            Number(value) > (p.maximum ?? Infinity))
            ? "Enter a number within the permitted range"
          : p.type === 'number' ? (() => { try { NumberInput.parse(value, { minimum: p.minimum ?? -1e6, maximum: p.maximum ?? 1e6 }); } catch (error) { return error.message; } return p.validate?.(value) ?? null; })()
          : p.validate?.(value) ?? null,
      };
    });
    const model = new FormModel({ fields });
    let owner;
    const completedValue = (id, value) => {
      const input = owner.form.fields.get(id).input;
      const draft = input instanceof NumberInput ? input.input.value : value;
      if (!Object.is(model.get(id), draft)) model.set(id, draft);
      return owner.commit(id);
    };
    const controls = Object.fromEntries(properties.map(p => [p.id,
      p.type === 'number' ? new NumberInput({ name: `property-${p.id}`, minimum: p.minimum ?? -1e6, maximum: p.maximum ?? 1e6, step: p.step ?? 1, precision: p.precision ?? 2, unit: p.unit ?? '', onCommit: commitMode === 'field' ? value => completedValue(p.id, value) : null }) :
      p.type === 'enum' ? new EditableChoice({ name: `property-${p.id}`, choices: p.choices, validate: value => p.choices.includes(value) ? null : 'Select a listed value', onCommit: commitMode === 'field' ? value => completedValue(p.id, value) : null }) :
      p.type === 'color' ? new ColorInput({ name: `property-${p.id}` }) :
      p.type === 'custom' ? p.createEditor({ property: p, model }) : p.type === 'boolean' ? new CheckBox() : new TextField()
    ]));
    const fieldOptions=Object.fromEntries(properties.map(p=>[p.id,Object.fromEntries(['layout','labelWidth','controlWidth'].filter(key=>p[key]!==undefined).map(key=>[key,p[key]]))]));
    const form = new Form({ model, controls, rows, fieldOptions }),
      status = new Label({ text: "" }),
      button = new Button({ text: "Apply properties", visible: commitMode === 'explicit' });
    super({ text: "panel:fixed:gap=8", ...options }, [form, status, button]);
    owner = this;
    Object.assign(this, {
      model,
      form,
      status,
      button,
      properties,
      read,
      apply,
      commitMode,
    });
    this.changedIds = new Set();
    this.loading = false;
    this.selectionEpoch = 0;
    this.groups = new Map();
    for (const spec of properties) {
      if (!spec.group) continue;
      let group = this.groups.get(spec.group);
      if (!group) { group = new GroupBox({ text: spec.group }); this.groups.set(spec.group, group); form.append(group); }
      const field = form.fields.get(spec.id); form.remove(field); group.append(field);
    }
    for (const spec of properties) {
      const field = model.fields.get(spec.id),
        validate = field.validate;
      field.validate = (value) =>
        !this.changedIds.has(spec.id)
          ? null
          : spec.required && value === ""
            ? "This field is required"
            : spec.type === "number" && (value === "" || value === null)
              ? "Enter a number"
              : validate(value);
    }
    let previousRevision = model.revision;
    this.stop = model.subscribe((snapshot) => {
      if (
        !this.loading &&
        snapshot.revision !== previousRevision &&
        snapshot.lastChangedField !== null
      ) {
        const id = snapshot.lastChangedField, spec = properties.find(p => p.id === id);
        let same = JSON.stringify(snapshot.values[id]) === JSON.stringify(model.initial[id]);
        if (!same && spec.type === 'number') {
          try { const bounds = { minimum: spec.minimum ?? -1e6, maximum: spec.maximum ?? 1e6 }; same = NumberInput.parse(snapshot.values[id], bounds) === NumberInput.parse(model.initial[id], bounds); } catch {}
        }
        if (same) this.changedIds.delete(id); else this.changedIds.add(id);
      }
      previousRevision = snapshot.revision;
      this.syncAvailability();
    });
    button.on(Message.ACTIVATE, () => this.commit());
    this.retries = new Map();
    if (commitMode === 'field') for (const spec of properties) {
      const input = controls[spec.id];
      const complete = () => this.changedIds.has(spec.id) && !this.model.snapshot.busy
        ? this.commit(spec.id).catch(() => false) : false;
      const retry = new Button({ text: 'Retry', accessibleName: `Retry ${spec.label || spec.id}`, visible: false });
      retry.on(Message.ACTIVATE, () => input instanceof NumberInput || input instanceof EditableChoice ? input.commit() : complete());
      form.fields.get(spec.id).append(retry); this.retries.set(spec.id, retry);
      if (input instanceof NumberInput || input instanceof EditableChoice) {
        input.input.on(Message.BLUR, () => Promise.resolve().then(() => input.dirty && !this.model.snapshot.busy ? input.commit() : false));
      } else if (input instanceof TextField) {
        input.on(Message.BLUR, complete); input.on(Message.ACTIVATE, complete);
      } else input.on(spec.type === 'boolean' ? Message.ACTIVATE : Message.VALUE_CHANGED, complete);
    }
    this.setSelection(selection);
  }
  syncAvailability() {
    const available = Boolean(this.enabled && this.visible && !(this.state & State.BUSY) && this.selection?.length && !this.model.snapshot.busy);
    if (this.button) this.button.enabled = available;
    if (this.commitMode === 'field') for (const spec of this.properties) {
      this.form.fields.get(spec.id).input.enabled = available && !spec.readOnly;
      const retry = this.retries?.get(spec.id);
      if (retry) { retry.visible = Boolean(this.model.errors[spec.id] && this.changedIds.has(spec.id)); retry.enabled = available && !spec.readOnly; }
    }
  }
  setProperty(name, value, options) {
    const changed = super.setProperty(name, value, options);
    if (changed && ['enabled', 'visible', 'state', 'busy'].includes(name)) this.syncAvailability();
    return changed;
  }
  commit(propertyId) {
    if (propertyId !== undefined && !this.properties.some(p => p.id === propertyId)) throw TypeError('Unknown inspector property');
    if (this.destroyed || !this.enabled || !this.visible || this.state & State.BUSY || !this.selection.length) return Promise.resolve(false);
    if (propertyId !== undefined && !this.changedIds.has(propertyId)) return Promise.resolve(true);
    const { model, properties, apply } = this;
    const epoch = this.selectionEpoch, ids = [...this.selection], revision = model.revision;
    return model.submit(async (values) => {
        if (this.destroyed || epoch !== this.selectionEpoch) throw Error('Selection changed before properties could be applied');
        const changed = Object.fromEntries(
          (propertyId === undefined ? [...this.changedIds] : [propertyId]).map((id) => {
            const p = properties.find((p) => p.id === id);
            return [id, p.type === "number" ? NumberInput.parse(values[id], { minimum: p.minimum ?? -1e6, maximum: p.maximum ?? 1e6 }) : values[id]];
          }),
        );
        if (!Object.keys(changed).length) return false;
        const result = await apply({ ids, values: changed });
        if (result === false) throw Error('Properties were not applied');
        if (propertyId === undefined && !this.destroyed && epoch === this.selectionEpoch && model.revision === revision) this.setSelection(ids);
        return true;
      }, propertyId === undefined ? {} : { fields: [propertyId] }).then(result => {
        if (propertyId !== undefined && !this.destroyed && epoch === this.selectionEpoch && model.revision === revision) {
          this.changedIds.delete(propertyId); this.mixedProperties?.delete(propertyId);
          this.status.text = Object.keys(model.errors).length ? 'Correct the form errors' : this.mixedProperties?.size
            ? 'Mixed values: ' + [...this.mixedProperties.values()].join(', ') : this.selection.length + ' selected';
          this.syncAvailability();
        }
        return result;
      }).catch(error => {
        if (!this.destroyed && epoch === this.selectionEpoch) {
          this.status.text = String(error.message || 'Properties could not be applied');
          if (propertyId === undefined) this.form.focusFirstError();
          else {
            if (!model.errors[propertyId]) { model.errors[propertyId] = this.status.text; model.changed(); }
            this.form.fields.get(propertyId).focus();
          }
        }
        throw error;
      });
  }
  cancel() { this.setSelection(this.selection); }
  setSelection(ids, { preserveDraft = false } = {}) {
    if (
      !Array.isArray(ids) ||
      ids.length > 4096 ||
      ids.some((id) => typeof id !== "string")
    )
      throw TypeError("Invalid property selection");
    const next = [...new Set(ids)], same = JSON.stringify(next) === JSON.stringify(this.selection);
    if (same && preserveDraft && this.changedIds.size) return;
    this.selection = next; this.selectionEpoch++;
    this.loading = true;
    try {
      this.mixedProperties = new Map();
      const values = {},
        mixed = [];
      for (const p of this.properties) {
        const all = this.selection.map((id) => this.read(id, p.id)),
          different = all.some(
            (v) => JSON.stringify(v) !== JSON.stringify(all[0]),
          );
        if (different) { mixed.push(p.label || p.id); this.mixedProperties.set(p.id, p.label || p.id); }
        values[p.id] = different
          ? p.type === "boolean"
            ? false
            : p.type === 'custom' ? structuredClone(p.initial) : p.type === 'number' || p.type === 'color' ? null : ""
          : (all[0] ?? (Object.hasOwn(p, 'initial') ? structuredClone(p.initial) : p.type === "boolean" ? false : p.type === 'number' || p.type === 'color' ? null : ""));
        this.form.fields.get(p.id).input.enabled = this.selection.length > 0 && !p.readOnly;
      }
      this.model.reset(values);
      for (const p of this.properties) {
        const input = this.form.fields.get(p.id).input;
        const different = this.mixedProperties.has(p.id);
        if (p.type === 'boolean') input.setProperty('state', different ? input.state | State.INDETERMINATE : input.state & ~State.INDETERMINATE);
        if (p.type === 'color') input.setValue(values[p.id], { mixed: different });
      }
      this.changedIds.clear();
      this.status.text = mixed.length
        ? "Mixed values: " + mixed.join(", ")
        : this.selection.length + " selected";
      this.syncAvailability();
    } finally {
      this.loading = false;
    }
  }
  destroy() {
    this.stop?.();
    this.model?.destroy();
    return super.destroy();
  }
}

/** Provider-backed paging for template/media/asset pickers, with native buttons. */
export class Gallery extends Column {
  static family = "composite";
  static constructorOptions = Object.freeze([
    "provider",
    "pageSize",
    "onSelect",
    "presentation", "renderPreview", "onPreview", "selectedId", "tileWidth", "tileHeight", "showSearch", "showTitles", "itemRole", "gap", "chrome", "previewPadding",
  ]);
  static authoring = Object.freeze({
    constructor:
      "new Gallery({provider:async({query,cursor,limit,signal})=>({items:[{id,title,description?}],next:null|string}),pageSize:24,onSelect:(item)=>{...}})",
    methods:
      "search(query), loadMore(), showPage(start), setSelection(id|null), preview(id), cancelPreview(), activate(id), menu(properties); menu creates a native Menu for an unparented itemRole:menuitem gallery, routing scoped popup navigation keys to the current or first enabled choice through the matching browser runtime. Card activation and dismissal remain native; the gallery releases its key subscription on destruction. selectedId is caller-owned and may name an unloaded item. Activation invokes onSelect without assuming success; update selection after the domain command succeeds. presentation:'tiles' accepts renderPreview(item)=>unparented Control. Visible cards retain identity/focus on loadMore. Arrows follow wrapped rows; Home/End and PageUp/PageDown navigate loaded choices. Page changes, Escape, focus departure, hide/disable, search and destruction cancel previews. Providers and onPreview(item,{signal}) receive AbortSignal; onPreview may return synchronous cleanup to restore transient artwork. itemRole:'menuitem' uses native menu commands, so activation participates in the containing Menu popup lifecycle; their preview roots and captions are display-only and do not intercept the command's pointer target. Provider items may specify enabled:false; disabled choices cannot preview/activate and keyboard navigation skips them. gap:0–64 sets tile spacing; chrome:false supplies a flat, transparent viewport and cards for inline galleries. previewPadding:null|0–32 optionally reserves native inset space around opaque tile previews; selected inset button cards expose the native selected face while idle flat cards remain flat. Empty page navigation takes no layout space. Domain data and activation remain caller-owned.",
  });
  constructor({
    provider,
    pageSize = 24,
    onSelect = () => {},
    presentation = 'list', renderPreview = null, onPreview = null, selectedId = null,
    tileWidth = 160, tileHeight = null, showSearch = true, showTitles = true, itemRole = 'button', gap=8, chrome=true, previewPadding=null,
    ...options
  } = {}) {
    if (
      typeof provider !== "function" ||
      typeof onSelect !== "function" ||
      !Number.isInteger(pageSize) ||
      pageSize < 1 ||
      pageSize > 100 || !['list', 'tiles'].includes(presentation) || renderPreview !== null && typeof renderPreview !== 'function' || onPreview !== null && typeof onPreview !== 'function'
      || !Gallery.validSelection(selectedId) || !Number.isFinite(tileWidth) || tileWidth < 24 || tileWidth > 1024 ||
      tileHeight !== null && (!Number.isFinite(tileHeight) || tileHeight < 24 || tileHeight > 1024) ||
      typeof showSearch !== 'boolean' || typeof showTitles !== 'boolean' || !['button','menuitem'].includes(itemRole)||typeof chrome!=='boolean'||!Number.isFinite(gap)||gap<0||gap>64||previewPadding!==null&&(!Number.isInteger(previewPadding)||previewPadding<0||previewPadding>32||tileWidth<=previewPadding*2||tileHeight!==null&&tileHeight<=previewPadding*2)
    )
      throw TypeError("Invalid gallery");
    const search = new TextField({ accessibleName: "Search collection", visible: showSearch }),
      items = presentation === 'tiles' ? new WrapPanel({ items: [], gap, transparent:!chrome }) : new Column({ text: "panel:fixed:gap=4" }),
      status = new Label({ text: "" }),
      more = new Button({ text: "Load more", visible: false }),
      viewport = new ScrollView(chrome?{}:{text:'panel:flush:padding=0',state:State.ENABLED|State.FLAT}, [items]);
    super({ text: "panel:gap=8", ...options }, [
      new Label({ text: "Search", visible: showSearch }),
      search,
      viewport,
      status,
      more,
    ]);
    Object.assign(this, { provider, pageSize, onSelect, items, status, more, viewport, gap, chrome, previewPadding });
    Object.assign(this, { presentation, renderPreview, onPreview, selectedId, pageStart: 0 });
    Object.assign(this, { tileWidth, tileHeight:tileHeight??(renderPreview?132:64), showTitles, itemRole });
    this.cards = new Map(); this.previewTask = new TaskController({ mode: 'latest' });
    if (presentation === 'tiles') {
      this.previous = new Button({ text: 'Previous choices', visible: false });
      this.next = new Button({ text: 'Next choices', visible: false });
      this.previous.on(Message.ACTIVATE, () => this.showPage(this.pageStart - this.pageSize));
      this.next.on(Message.ACTIVATE, () => this.showPage(this.pageStart + this.pageSize));
      this.navigation=this.append(new Column({ text: 'panel:fixed:gap=2',visible:false }, [this.previous, this.next]));
    }
    this.task = new TaskController({ mode: "latest" });
    this.query = "";
    this.cursor = null;
    this.loaded = new Map();
    search.on(Message.VALUE_CHANGED, () => this.search(search.value));
    more.on(Message.ACTIVATE, () => this.loadMore());
  }
  /** Keep the native popup/leaf owners while the SDK owns gallery navigation. */
  menu(properties = {}) {
    if (this.itemRole !== 'menuitem' || this.parent || this.destroyed ||
        !this.text.startsWith('panel:') || this.text.length > 240)
      throw TypeError('Gallery menu requires unparented menuitem cards and a bounded panel layout');
    const menu = new Menu(properties, [this]);
    this.text += ':menu-gallery';
    const navigate = event => {
      if (event.defaultPrevented || ![menu,this].includes(event.target) || event.detail?.pressed === false ||
          event.detail?.modifiers || !menu.enabled || !menu.visible || !this.enabled || !this.visible) return;
      const focused = [...this.cards].find(([,card]) => card.getProperty('focused'))?.[0];
      const selected = this.loaded.get(focused ?? this.selectedId);
      const id = selected?.enabled !== false && selected ? selected.id :
        [...this.loaded.values()].find(item => item.enabled !== false)?.id;
      if (id) return this.navigate(id, event);
    };
    const stopMenu = menu.on(Message.KEY, navigate), stopGallery = this.on(Message.KEY, navigate);
    this.menuKeyDisposer = () => { stopMenu(); stopGallery(); };
    return menu;
  }
  search(query) {
    if (typeof query !== "string" || query.length > 512)
      throw TypeError("Invalid gallery query");
    this.query = query;
    this.cursor = null;
    this.cancelPreview();
    this.loaded.clear();
    this.clearCards();
    this.more.visible = false;
    if (this.previous) {this.previous.visible = this.next.visible = false;this.navigation.visible=false;}
    this.pageStart = 0;
    return this.load(false);
  }
  loadMore() {
    if (this.cursor === null) return Promise.resolve(false);
    return this.load(true);
  }
  load(append) {
    const query = this.query,
      cursor = append ? this.cursor : null;
    return this.task.run(async ({ signal }) => {
      this.status.text = "Loading…";
      this.more.enabled = false;
      try {
        const page = await this.provider({
          query,
          cursor,
          limit: this.pageSize,
          signal,
        });
        signal.throwIfAborted();
        if (
          !page ||
          !Array.isArray(page.items) ||
          page.items.length > this.pageSize ||
          (page.next !== null &&
            (typeof page.next !== "string" || page.next.length > 1024)) ||
          this.loaded.size + page.items.length > 1000
        )
          throw TypeError("Invalid or oversized gallery page");
        const seen = new Set(this.loaded.keys());
        for (const item of page.items) {
          if (
            !item ||
            typeof item.id !== "string" ||
            !item.id ||
            item.id.length > 128 ||
            typeof item.title !== "string" ||
            !item.title ||
            item.title.length > 256 ||
            seen.has(item.id) || item.enabled !== undefined && typeof item.enabled !== 'boolean'
          )
            throw TypeError("Gallery items need unique ids and titles");
          seen.add(item.id);
        }
        for (const item of page.items) {
          this.loaded.set(item.id, structuredClone(item));
          if (this.presentation === 'list') {
            const button = this.createCard(item);
            this.cards.set(item.id, button);
            this.items.append(button);
          }
        }
        if (this.presentation === 'tiles') this.showPage(this.pageStart);
        this.cursor = page.next;
        this.more.visible = this.cursor !== null;
        this.status.text = this.loaded.size
          ? this.loaded.size + " items"
          : "No results";
        return true;
      } catch (error) {
        if (!signal.aborted) this.status.text = String(error.message);
        throw error;
      } finally {
        if (!signal.aborted) this.more.enabled = true;
      }
    });
  }
  clearCards() {
    if (this.presentation === 'tiles') this.items.setItems([]);
    for (const control of this.cards.values()) control.destroy();
    this.cards.clear();
  }
  static validSelection(id) { return id === null || typeof id === 'string' && id.length > 0 && id.length <= 128; }
  setSelection(id) {
    if (!Gallery.validSelection(id)) throw TypeError('Gallery selection requires an item id or null');
    this.selectedId = id;
    for (const [key, card] of this.cards) {
      card.setProperty('selected', key === id);
      // An opaque inset preview leaves the native selected face visible.
      // FLAT suppresses that face in retained renderers, including selection.
      if(!this.chrome&&this.previewPadding>0&&this.itemRole==='button')card.setProperty('state',(card.getProperty('state')&~State.FLAT)|(key===id?0:State.FLAT));
    }
    return this;
  }
  createCard(item) {
    const tile = this.presentation === 'tiles';
    const Card=this.itemRole==='menuitem'?MenuItem:Button;
    const card = new Card({ accessibleName: item.title, tooltip: item.description || item.title, enabled:item.enabled!==false,
      text: tile ? (this.previewPadding!==null?`panel:fixed:gap=0:padding=${this.previewPadding}:flush`:this.itemRole==='menuitem'||!this.chrome?'panel:fixed:gap=0:padding=0:flush':'') : item.title, selected: item.id === this.selectedId, state: State.START_ALIGNED|(!this.chrome&&!(this.previewPadding>0&&this.itemRole==='button'&&item.id===this.selectedId)?State.FLAT:0) });
    try {
      if (tile) {
        if (this.renderPreview) {
          const preview = this.renderPreview(structuredClone(item));
          if (!(preview instanceof Control) || preview.parent) throw TypeError('Gallery preview must be an unparented native control');
          if(this.itemRole==='menuitem')preview.enabled=false;
          card.append(preview);
        }
        if(this.showTitles)card.append(new Label({ text: item.title,enabled:this.itemRole!=='menuitem' }));
        if (item.group) card.append(new Label({ text: String(item.group).slice(0, 128),enabled:this.itemRole!=='menuitem', state: State.MUTED }));
      }
      const preview = () => { if (this.previewId !== item.id) return this.preview(item.id).catch(() => {}); };
      card.on(Message.ACTIVATE, () => this.activate(item.id));
      card.on(Message.POINTER, event => { if (event.detail?.kind === 'move') return preview(); });
      card.on(Message.FOCUS, preview);
      card.on(Message.BLUR, () => { if (this.previewId === item.id) this.cancelPreview(); });
      card.on(Message.KEY, event => this.navigate(item.id, event));
      return card;
    } catch (error) { card.destroy(); throw error; }
  }
  navigate(id, event) {
    if (!this.enabled || !this.visible || event.detail?.pressed === false) return;
    const key = event.detail?.key || ({9:'Escape',110:'Home',115:'End',112:'PageUp',117:'PageDown',113:'ArrowLeft',114:'ArrowRight',111:'ArrowUp',116:'ArrowDown'})[event.detail?.keyCode];
    if (key === 'Escape') { event.preventDefault(); event.stopPropagation(); this.cancelPreview(); return; }
    const entries = [...this.loaded.values()], index = entries.findIndex(item => item.id === id);
    if (index < 0) return;
    let next;
    if (key === 'Home') next = 0;
    else if (key === 'End') next = entries.length - 1;
    else if (key === 'PageUp') next = index - this.pageSize;
    else if (key === 'PageDown') next = index + this.pageSize;
    else if (key === 'ArrowLeft') next = index - 1;
    else if (key === 'ArrowRight') next = index + 1;
    else if (key === 'ArrowUp' || key === 'ArrowDown') {
      if (this.presentation === 'list') next = index + (key === 'ArrowUp' ? -1 : 1);
      else {
        // Use the same layout and current native width as the visible WrapPanel.
        // Stack page geometries so partial last rows retain their visual column.
        const positions = []; let y = 0;
        for (let start = 0; start < entries.length; start += this.pageSize) {
          const page = entries.slice(start, start + this.pageSize);
          const layout = wrapLayout(page.map(() => ({ width: this.tileWidth, height: this.tileHeight })), { width: this.items.width, gap: this.items.gap });
          positions.push(...layout.bounds.map(b => ({ x: b.x + b.width / 2, y: b.y + y })));
          y += layout.height + this.items.gap;
        }
        const current = positions[index], down = key === 'ArrowDown';
        const candidates = positions.map((p, i) => ({ ...p, index: i })).filter(p => entries[p.index].enabled!==false&&(down ? p.y > current.y : p.y < current.y));
        candidates.sort((a, b) => Math.abs(a.y - current.y) - Math.abs(b.y - current.y) || Math.abs(a.x - current.x) - Math.abs(b.x - current.x));
        next = candidates[0]?.index ?? index;
      }
    } else return;
    event.preventDefault(); event.stopPropagation();
    next = Math.max(0, Math.min(entries.length - 1, next));
    const step=key==='Home'?1:key==='End'?-1:next>=index?1:-1;
    while(next>=0&&next<entries.length&&entries[next].enabled===false)next+=step;
    if(next<0||next>=entries.length)return;
    if (this.presentation === 'tiles' && (next < this.pageStart || next >= this.pageStart + this.pageSize)) this.showPage(Math.floor(next / this.pageSize) * this.pageSize);
    for (const [key, card] of this.cards) card.setProperty('focused', key === entries[next].id);
    if (this.previewId !== entries[next].id) return this.preview(entries[next].id).catch(() => {});
  }
  showPage(start = 0) {
    if (this.presentation !== 'tiles') return false;
    if (!Number.isInteger(start)) throw TypeError('Gallery page requires an integer');
    const entries = [...this.loaded.values()];
    start = Math.max(0, Math.min(Math.floor(start / this.pageSize) * this.pageSize, Math.max(0, Math.ceil(entries.length / this.pageSize) - 1) * this.pageSize));
    const page = entries.slice(start, start + this.pageSize), cards = [], retained = new Map(), created = [];
    try {
      for (const [index, item] of page.entries()) {
        let card = this.cards.get(item.id);
        if (!card) { card = this.createCard(item); created.push(card); }
        retained.set(item.id, card);
        // Provider ids may contain spaces/Unicode and use the full 128-character
        // contract. Native layout keys are private, bounded identifiers.
        cards.push({ id: 'gallery-' + (start + index), control: card, width: this.tileWidth, height: this.tileHeight });
      }
      if (start !== this.pageStart) this.cancelPreview();
      this.items.setItems(cards);
    } catch (error) { for (const card of created) card.destroy(); throw error; }
    for (const [id, card] of this.cards) if (!retained.has(id)) card.destroy();
    this.cards = retained; this.pageStart = start;
    this.previous.visible = start > 0; this.next.visible = start + this.pageSize < entries.length;
    this.navigation.visible=this.previous.visible||this.next.visible;
    return true;
  }
  activate(id) {
    if (!this.enabled || !this.visible || this.destroyed || !this.loaded.has(id) || this.loaded.get(id).enabled===false) return false;
    this.cancelPreview(); return this.onSelect(structuredClone(this.loaded.get(id)));
  }
  preview(id) {
    if (!this.enabled || !this.visible || !this.loaded.has(id) || this.loaded.get(id).enabled===false || this.destroyed) return Promise.resolve(false);
    this.cancelPreview(); this.previewId = id;
    if (!this.onPreview) return Promise.resolve(false);
    let root = this;
    while (root.parent) root = root.parent;
    galleryPreviews.get(root)?.cancelPreview();
    galleryPreviews.set(root, this); this.previewRoot = root;
    // Hover does not steal editing focus. An unconsumed Escape can still
    // restore its artwork when keyboard focus is elsewhere in this window.
    this.stopPreviewKey = root.on(Message.KEY, event => {
      if (event.defaultPrevented || event.detail?.pressed === false) return;
      if (event.detail?.key === 'Escape' || event.detail?.keyCode === 9) {
        event.preventDefault(); event.stopPropagation(); this.cancelPreview();
      }
    });
    return this.previewTask.run(async ({ signal }) => {
      const cleanup = await this.onPreview(structuredClone(this.loaded.get(id)), { signal });
      if (typeof cleanup === 'function') {
        if (signal.aborted || this.destroyed) cleanup();
        else this.previewCleanup = cleanup;
      }
      signal.throwIfAborted(); return true;
    });
  }
  cancelPreview() {
    this.previewId = null; this.previewTask?.cancel(); this.stopPreviewKey?.(); this.stopPreviewKey = null;
    if (galleryPreviews.get(this.previewRoot) === this) galleryPreviews.delete(this.previewRoot);
    this.previewRoot = null;
    const cleanup = this.previewCleanup; this.previewCleanup = null; cleanup?.();
  }
  setProperty(name, value, options) {
    const changed = super.setProperty(name, value, options);
    if (changed && (name === 'enabled' || name === 'visible') && !value) this.cancelPreview();
    return changed;
  }
  destroy() {
    if (this.destroyed) return;
    this.menuKeyDisposer?.(); this.menuKeyDisposer = null;
    this.task.close();
    this.cancelPreview(); this.previewTask.close();
    this.clearCards();
    return super.destroy();
  }
}
