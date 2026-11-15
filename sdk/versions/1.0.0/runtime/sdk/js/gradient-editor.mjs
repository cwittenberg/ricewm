import { Column, Row, Button, Label, Menu, MenuItem } from './controls.mjs';
import { State } from './control.mjs';
import { Message } from './messages.mjs';
import { SceneSurface } from './scene-surface.mjs';
import { SceneBuilder } from './scene-builder.mjs';
import { NumberInput, ColorInput } from './value-editors.mjs';
import { TaskController } from './tasks.mjs';
import { checkedGradientValue, paintBackgroundFill } from './background-fills.mjs';
import { Gallery } from './property-inspector.mjs';
import { gradientPresets } from './fill-presets.mjs';
import {colorWithOpacity,paintFromColorChoice,paintSelectionKey} from './color-references.mjs';
const paintOf=({offset,...paint})=>paint;

const initial = () => ({ kind: 'linear', angle: 0, stops: [{ offset: 0, color: '#306BDD', opacity: 1 }, { offset: 1, color: '#E65C32', opacity: 1 }] });
export function checkedGradient(value) {
    return checkedGradientValue(value);
}
const kindNames={linear:'Linear',radial:'Radial',rectangular:'Rectangular',path:'Path'};
const directions=[['From center',[.5,.5,.5,.5]],['From top left',[0,0,1,1]],['From top right',[1,0,0,1]],['From bottom left',[0,1,1,0]],['From bottom right',[1,1,0,0]]];
const rgba = stop => (parseInt(stop.color.slice(1, 3), 16) | parseInt(stop.color.slice(3, 5), 16) << 8 | parseInt(stop.color.slice(5, 7), 16) << 16 | Math.round(stop.opacity * 255) << 24) >>> 0;

/** Native gradient stop authoring. Explicit Apply or completed field/drag commits. */
export class GradientEditor extends Column {
    static family = 'composite';
    static properties = Object.freeze({ readOnly: { default: false, reflect: false, validate: Boolean }, value: { default: initial(), reflect: false, validate: checkedGradient } });
    static constructorOptions = Object.freeze(['value', 'onPreview', 'onCommit', 'readOnly', 'commitMode', 'themeColors', 'standardColors', 'columns', 'normalizeValue']);
    static authoring = Object.freeze({
        constructor: 'new GradientEditor({value:{kind:"linear",angle:45,stops:[{offset:0,color:"#306BDD",opacity:1},{offset:1,color:"#E65C32",opacity:1}]},readOnly:false,onPreview:async(value,{signal})=>{...},onCommit:async(value)=>{...}})',
        methods: 'setThemeColors(swatches), setValue(value,{preserveDraft:true}), updateStop(index,patch), addStop(offset), removeStop(), commit(), cancel(). Linear, radial, rectangular and path paints support focus/direction and six presets. commitMode:"explicit" (default) uses Apply/Cancel; "field" commits completed fields and pointer release once, keeping rejected drafts with retry controls. Click the bar to add a stop; drag markers or use arrows to move them. dirty includes typed numeric/custom-color drafts. Routine same-selection refreshes preserve drafts. Escape cancels local previews. readOnly permits inspection while disabling edits; busy/pending also block edits. Preview never writes history. Optional source descriptors on stop paints retain theme identity through reordering and alpha changes; explicit custom RGB clears the source reference. themeColors, standardColors and columns forward to ColorInput. Optional synchronous normalizeValue(value) applies the owning format precision policy before onCommit. For DrawingML use checkedDrawingmlGradientValue to normalize angles, positions and path geometry while retaining source colours. Rejected normalization retains the draft; a canonical no-op creates no history. Caller owns persistence, revision checks and undo.',
    });
    constructor({ value = initial(), onPreview = null, onCommit = null, normalizeValue = null, commitMode = 'explicit', themeColors=[], standardColors, columns=8, ...options } = {}) {
        if(!['explicit','field'].includes(commitMode))throw TypeError('Invalid gradient commit mode');
        if (onPreview !== null && typeof onPreview !== 'function' || onCommit !== null && typeof onCommit !== 'function' || normalizeValue !== null && typeof normalizeValue !== 'function') throw TypeError('Invalid gradient callbacks');
        const original = checkedGradient(value);
        super({ text: 'panel:fixed:gap=4', ...options, value: original });
        Object.assign(this, { original, draft: structuredClone(original), onPreview, onCommit, normalizeValue, selected: 0, width: 280, dragging: false, dirty: false, pending: false, valueEpoch: 0 });
        this.commitMode=commitMode;
        this.previewTask = new TaskController({ mode: 'latest' });
        this.presets=this.append(new Gallery({name:`${this.name||'gradient'}-presets`,presentation:'tiles',tileWidth:36,tileHeight:28,showSearch:false,showTitles:false,
            provider:async()=>({items:gradientPresets(paintOf(this.draft.stops[0])),next:null}),
            renderPreview:item=>{const surface=new SceneSurface({accessibleName:item.title});surface.submit(paintBackgroundFill(new SceneBuilder(32,24),{kind:'gradient',gradient:item.gradient},[0,0,32,24]).finish());return surface;},
            onSelect:item=>{if(!this.canEdit()||!this.flushFieldDrafts())return false;this.draft=checkedGradient(item.gradient);this.selected=0;this.changed();return true;}}));
        this.presets.status.visible=false;
        void this.presets.search('').catch(()=>{});
        const field=(label,control)=>{this.append(new Row({text:'panel:form-control'},[new Label({text:label}),control]));return control;};
        this.kindMenu = field('Type',new Menu({ text: 'Linear', accessibleName: 'Gradient type', state: State.START_ALIGNED | State.TRAILING_ICON, icon: 74 }));
        for (const [kind,text] of Object.entries(kindNames)) this.kindMenu.append(new MenuItem({ text })).on(Message.ACTIVATE, () => {
            if (!this.canEdit() || !this.flushFieldDrafts()) return false;
            this.draft.kind = kind;if(kind!=='linear')this.draft.focus??=[.5,.5,.5,.5];return this.changed();
        });
        this.directionMenu=field('Direction',new Menu({text:'Direction',accessibleName:'Gradient direction',state:State.START_ALIGNED|State.TRAILING_ICON,icon:74}));
        this.directionItems=[];
        for(const [text,focus] of directions){const item=this.directionMenu.append(new MenuItem({text}));this.directionItems.push({item,path:true});item.on(Message.ACTIVATE,()=>{
            if(!this.canEdit()||!this.flushFieldDrafts())return false;this.draft.focus=[...focus];this.changed();
        });}
        for(let angle=0;angle<360;angle+=45){const item=this.directionMenu.append(new MenuItem({text:`${angle}°`}));this.directionItems.push({item,path:false});item.on(Message.ACTIVATE,()=>{
            if(!this.canEdit()||!this.flushFieldDrafts())return false;this.draft.angle=angle;this.draft.scaled=false;this.changed();
        });}
        this.angle = field('Angle',new NumberInput({ name: `${this.name || 'gradient'}-angle`, value: original.angle, minimum: -36000, maximum: 36000, unit: '°', accessibleName: 'Gradient angle', onCommit: value => { if (!this.canEdit()) return false; this.draft.angle = value; this.changed(); } }));
        this.surface = new SceneSurface({ name: `${this.name || 'gradient'}-stops`, accessibleName: 'Gradient stops: drag to change position', accessibleRole: 'slider', cursor: 'col-resize' });
        this.append(new Column({ text: 'panel:fixed-height:64:flush' }, [this.surface]));
        this.surface.on(Message.SIZE_CHANGED, event => { if (event.detail.width > 0) { this.width = Math.min(16384, Math.round(event.detail.width)); this.paint(); } });
        this.surface.on(Message.POINTER, event => this.pointer(event));
        this.position = field('Position',new NumberInput({ name: `${this.name || 'gradient'}-position`, minimum: 0, maximum: 100, unit: '%', accessibleName: 'Stop position', onCommit: value => this.updateStop(this.selected, { offset: value / 100 }) }));
        this.color = this.append(new ColorInput({ name: `${this.name || 'gradient'}-color`, value: original.stops[0].color, allowNone: false, themeColors, standardColors, columns, accessibleName: 'Stop color' }));
        this.color.palette.setProperty('state',State.START_ALIGNED|State.TRAILING_ICON);
        this.themeColors=structuredClone(themeColors);
        this.color.on(Message.VALUE_CHANGED, event => {const chosen=paintFromColorChoice(paintOf(this.draft.stops[this.selected]),event.detail);this.updateStop(this.selected,{...chosen,source:chosen.source});});
        this.transparency = field('Transparency',new NumberInput({ name: `${this.name || 'gradient'}-transparency`, minimum: 0, maximum: 100, unit: '%', accessibleName: 'Stop transparency', onCommit: value => this.updateStop(this.selected, { opacity: 1 - value / 100 }) }));
        this.numericFields = [this.angle, this.position, this.transparency];
        for (const field of this.numericFields) {
            field.on(Message.CHANGE, event => { if (event.detail?.draft) this.syncAvailability(); });
            field.on(Message.VALUE_CHANGED, () => {this.syncAvailability();this.requestFieldCommit();});
            // The numeric editor owns Escape while its local text is dirty.
            // A clean field lets Escape cancel the enclosing paint preview.
            field.input.on(Message.KEY, event => {
                if (event.detail?.pressed === false || !this.canInspect() || field.dirty || field.pending) return;
                if (event.detail?.key === 'Escape' || event.detail?.keyCode === 9) {
                    event.preventDefault(); event.stopPropagation(); this.cancel();
                }
            });
        }
        const palette = this.color.palette.palette;
        for (const control of [palette.moreButton, palette.cancelButton]) control.on(Message.ACTIVATE, () => this.syncAvailability());
        palette.hex.on(Message.VALUE_CHANGED, () => this.syncAvailability());
        const add = new Button({ text: 'Add stop' }), remove = new Button({ text: 'Remove stop' }); this.removeButton = remove; this.addButton = add;
        add.on(Message.ACTIVATE, () => this.addStop(.5)); remove.on(Message.ACTIVATE, () => this.removeStop());
        this.append(new Row({}, [add, remove]));
        this.status = this.append(new Label({ text: '', visible: false, accessibleLive: 'polite' }));
        this.applyButton = new Button({ text: 'Apply gradient', accent: true }); this.cancelButton = new Button({ text: 'Cancel preview' });
        this.applyButton.on(Message.ACTIVATE, () => this.commit()); this.cancelButton.on(Message.ACTIVATE, () => this.cancel());
        this.append(new Row({}, [this.cancelButton, this.applyButton]));
        this.on(Message.KEY, event => {
            if (event.detail?.pressed === false || !this.canInspect()) return;
            const key = event.detail?.key, code = event.detail?.keyCode;
            if (key === 'Escape' || code === 9) { event.preventDefault(); event.stopPropagation(); this.cancel(); }
            if (event.target !== this.surface) return;
            if (key === 'ArrowLeft' || key === 'ArrowRight' || code === 113 || code === 114) {
                event.preventDefault(); event.stopPropagation();
                const delta = key === 'ArrowLeft' || code === 113 ? -.01 : .01;
                this.updateStop(this.selected, { offset: Math.max(0, Math.min(1, this.draft.stops[this.selected].offset + delta)) });
            } else if (key === 'Delete' || code === 119) { event.preventDefault(); this.removeStop(); }
            else if (key === 'Tab' || code === 23) {
                if (!this.flushFieldDrafts()) return;
                this.selected = (this.selected + (event.detail.modifiers & 1 ? this.draft.stops.length - 1 : 1)) % this.draft.stops.length;
                this.refresh({ preserveDraft: false });
            }
        });
        this.refresh();
    }
    get value() { return structuredClone(this.original); }
    set value(value) { this.setValue(value); }
    get dirty() {
        const palette = this.color?.palette.palette;
        return Boolean(this.paintDirty || this.numericFields?.some(field => field.dirty || field.pending)
            || palette?.moreOpen && palette.hex.value.toUpperCase() !== palette.value);
    }
    set dirty(value) { this.paintDirty = Boolean(value); }
    hasFieldDrafts() { return this.numericFields?.some(field => field.dirty || field.pending) || this.color?.palette.palette.moreOpen || false; }
    setProperty(name, value, options) {
        if (name === 'value' && this.original) { this.setValue(value); return true; }
        const changed = super.setProperty(name, value, options);
        if (this.applyButton && ['readOnly', 'enabled', 'busy', 'state'].includes(name)) this.syncAvailability();
        return changed;
    }
    canInspect() { return Boolean(this.state & State.ENABLED) && !(this.state & State.BUSY) && !this.pending && !this.destroyed; }
    canEdit() { return this.canInspect() && !this.getProperty('readOnly'); }
    syncAvailability() {
        const inspect = this.canInspect(), edit = this.canEdit(), readOnly = this.getProperty('readOnly');
        if (!edit) { this.dragging = false; this.previewTask.cancel(); }
        for (const field of [this.angle, this.position, this.transparency, this.color]) {
            field.enabled = inspect && (field !== this.angle || this.draft.kind === 'linear');
            field.setProperty('readOnly', readOnly);
        }
        this.kindMenu.enabled = this.directionMenu.enabled = this.presets.enabled = edit; this.surface.enabled = inspect;
        for(const {item,path}of this.directionItems)item.visible=path===(this.draft.kind!=='linear');
        this.angle.visible=this.angle.parent.visible=this.draft.kind==='linear';
        this.removeButton.enabled = edit && this.draft.stops.length > 2;
        this.addButton.enabled = edit && this.draft.stops.length < 64;
        this.applyButton.enabled = edit && this.dirty;
        this.cancelButton.enabled = inspect && this.dirty;
        this.applyButton.visible=this.cancelButton.visible=this.commitMode==='explicit'||Boolean(this.status.text);
    }
    setFormValue(value, options) { this.setValue(value, options); }
    setValue(value, { preserveDraft = false } = {}) {
        const original = checkedGradient(value);
        const keep = preserveDraft && (this.dirty || this.hasFieldDrafts() || this.pending);
        if (!preserveDraft || JSON.stringify(original) !== JSON.stringify(this.original)) this.valueEpoch++;
        this.original = original; super.setProperty('value', original);
        if (!keep) {
            this.previewTask.cancel(); this.draft = structuredClone(original); this.selected = 0; this.dirty = false; this.dragging = false;
            this.color?.palette.palette.closeEditor(); this.notice(''); this.refresh({ preserveDraft: false });
        } else { this.dirty = JSON.stringify(this.draft) !== JSON.stringify(original); this.syncAvailability(); }
    }
    flushFieldDrafts() {
        if (!this.hasFieldDrafts()) return true;
        if (!this.canEdit() || this.numericFields.some(field => field.pending)) { this.notice('Finish or cancel the current value before changing stops'); return false; }
        const plan = [], palette = this.color.palette.palette, epoch = this.valueEpoch;
        for (const field of this.numericFields.filter(field => field.dirty)) {
            try { plan.push({ field, value: field.read() }); }
            catch (error) { void field.commit(); this.notice(error.message); return false; }
        }
        // Validate every local draft before changing any part of the paint.
        if (palette.moreOpen && !/^#[0-9a-f]{6}$/iu.test(palette.hex.value)) {
            palette.hex.setProperty('invalid', true); this.notice('Enter a stop color as #RRGGBB'); return false;
        }
        const customColor = palette.moreOpen ? palette.hex.value.toUpperCase() : null;
        if (plan.length || customColor) {
            const stop = { ...this.draft.stops[this.selected] };
            for (const { field, value } of plan) {
                if (field === this.angle) this.draft.angle = value;
                else if (field === this.position) stop.offset = value / 100;
                else Object.assign(stop,colorWithOpacity(paintOf(stop),1-value/100));
            }
            if (customColor) {stop.color = customColor;delete stop.source;}
            this.draft.stops[this.selected] = stop; this.draft.stops.sort((a, b) => a.offset - b.offset); this.selected = this.draft.stops.indexOf(stop);
            for (const { field, value } of plan) field.setValue(value);
            // Accept the owned palette only after the complete paint is ready;
            // its change callback must never observe half-applied numeric data.
            if (customColor) palette.commit(customColor);
            else this.changed();
        }
        if (epoch !== this.valueEpoch || !this.canEdit()) return false;
        this.notice(''); return true;
    }
    pointer(event) {
        const d = event.detail || {};
        if (!this.canInspect()) return;
        const offset = Math.max(0, Math.min(1, (d.x - 8) / Math.max(1, this.width - 16)));
        if (d.kind === 'down' && d.button === 1) {
            if (!this.flushFieldDrafts()) return;
            if(d.y<40 && this.canEdit() && this.draft.stops.length<64){this.dragging=true;this.addStop(offset);event.preventDefault();return;}
            this.selected = this.draft.stops.reduce((best, stop, index, all) => Math.abs(stop.offset - offset) < Math.abs(all[best].offset - offset) ? index : best, 0);
            this.dragging = this.canEdit(); this.refresh({ preserveDraft: false }); event.preventDefault();
        } else if (d.kind === 'move' && this.dragging) { this.updateStop(this.selected, { offset }); event.preventDefault(); }
        else if (d.kind === 'up') { this.dragging = false; this.requestFieldCommit(); }
        else if (d.kind === 'cancel') { this.dragging = false; this.cancel(); }
    }
    updateStop(index, patch) {
        if (!this.canEdit()) return false;
        if (!Number.isInteger(index) || !this.draft.stops[index] || Object.keys(patch).some(key => !['offset', 'color', 'opacity', 'source'].includes(key))) throw TypeError('Invalid stop edit');
        const merge=stop=>{let next={...stop,...patch};if(Object.hasOwn(patch,'color')&&!patch.source)delete next.source;if(Object.hasOwn(patch,'opacity')&&!Object.hasOwn(patch,'source'))next={offset:next.offset,...colorWithOpacity(paintOf({...next,opacity:stop.opacity}),patch.opacity)};return next;};
        checkedGradient({ ...this.draft, stops: this.draft.stops.map((stop, i) => i === index ? merge(stop) : stop) });
        if (index !== this.selected && this.hasFieldDrafts()) {
            const target = this.draft.stops[index];
            if (!this.flushFieldDrafts()) return false;
            index = this.draft.stops.indexOf(target);
            if (index < 0) return false;
        }
        const next = merge(this.draft.stops[index]);
        checkedGradient({ ...this.draft, stops: this.draft.stops.map((stop, i) => i === index ? next : stop) });
        this.draft.stops[index] = next; this.draft.stops.sort((a, b) => a.offset - b.offset); this.selected = this.draft.stops.indexOf(next);
        this.changed(); return true;
    }
    addStop(offset = .5) {
        if (!this.canEdit() || this.draft.stops.length >= 64) return false;
        if (!Number.isFinite(offset) || offset < 0 || offset > 1) throw RangeError('Stop position must be 0..1');
        if (!this.flushFieldDrafts()) return false;
        const left = [...this.draft.stops].reverse().find(stop => stop.offset <= offset) || this.draft.stops[0];
        const right = this.draft.stops.find(stop => stop.offset >= offset) || this.draft.stops.at(-1);
        const t = right.offset === left.offset ? 0 : (offset - left.offset) / (right.offset - left.offset);
        const channels = [1, 3, 5].map(i => Math.round(parseInt(left.color.slice(i, i + 2), 16) * (1 - t) + parseInt(right.color.slice(i, i + 2), 16) * t).toString(16).padStart(2, '0'));
        const stop = { offset, color: '#' + channels.join('').toUpperCase(), opacity: left.opacity * (1 - t) + right.opacity * t };
        this.draft.stops.push(stop); this.draft.stops.sort((a, b) => a.offset - b.offset); this.selected = this.draft.stops.indexOf(stop); this.changed(); return true;
    }
    removeStop() { if (!this.canEdit() || this.draft.stops.length <= 2 || !this.flushFieldDrafts()) return false; this.draft.stops.splice(this.selected, 1); this.selected = Math.min(this.selected, this.draft.stops.length - 1); this.changed(); return true; }
    changed() {
        this.dirty = JSON.stringify(this.draft) !== JSON.stringify(this.original); this.refresh();
        const value = checkedGradient(this.draft);
        this.emit(Message.CHANGE, { value, preview: true });
        if (this.onPreview && this.canEdit()) void this.previewTask.run(({ signal }) => this.onPreview(structuredClone(value), { signal })).catch(error => { if (error.code !== 'CANCELLED' && !this.destroyed) this.notice(error.message); });
        this.requestFieldCommit();
    }
    requestFieldCommit() {
        if(this.commitMode!=='field'||this.fieldCommitQueued)return;
        this.fieldCommitQueued=true;
        queueMicrotask(()=>{this.fieldCommitQueued=false;if(this.canEdit()&&this.dirty&&!this.dragging&&!this.hasFieldDrafts())void this.commit();});
    }
    notice(text) { this.status.text = text; this.status.visible = Boolean(text); }
    setThemeColors(values) { this.color.palette.palette.setThemeColors(values);this.themeColors=structuredClone(values);this.refresh(); }
    refresh({ preserveDraft = true } = {}) {
        const stop = this.draft.stops[this.selected];
        if (!this.position) return;
        for (const [field, value] of [[this.position, stop.offset * 100], [this.transparency, (1 - stop.opacity) * 100], [this.angle, this.draft.angle]]) {
            // The pending numeric commit owns its accepted scalar. A unit
            // conversion echo can differ by rounding and must not retarget it.
            if (!preserveDraft || !field.pending) field.setValue(value, { preserveDraft });
        }
        if (!this.color.palette.palette.moreOpen || !preserveDraft) this.color.setValue(stop.color,{selectionKey:paintSelectionKey(stop,this.themeColors)});
        const presetKey=JSON.stringify(paintOf(this.draft.stops[0]));
        if(this.presetKey!==presetKey){this.presetKey=presetKey;void this.presets.search('').catch(()=>{});}
        this.kindMenu.text = kindNames[this.draft.kind];
        this.directionMenu.text=this.draft.kind==='linear'?`${this.draft.angle}°`:directions.find(([,f])=>f.every((n,i)=>Math.abs(n-(this.draft.focus||[.5,.5,.5,.5])[i])<1e-6))?.[0]||'Custom direction';
        this.syncAvailability(); this.paint();
    }
    paint() {
        const w = this.width, frame = new SceneBuilder(w, 64);
        // Scene colors pack red in the low byte and alpha in the high byte.
        for (let x = 0; x < w; x += 8) for (let y = 0; y < 40; y += 8) frame.rect([x, y, Math.min(8, w - x), 8], (x / 8 + y / 8) % 2 ? 0xffdddddd : 0xffffffff);
        paintBackgroundFill(frame,{kind:'gradient',gradient:this.draft},[8,0,Math.max(1,w-16),40]);
        this.draft.stops.forEach((stop, index) => { const x = 8 + stop.offset * Math.max(1, w - 16); frame.rect([x - 5, 43, 10, 18], index === this.selected ? 0xff325ce6 : 0xff555555).rect([x - 3, 45, 6, 14], rgba({ ...stop, opacity: 1 })); });
        this.surface.submit(frame.finish());
    }
    async commit() {
        if (!this.canEdit() || !this.dirty) return false;
        const sourceEpoch = this.valueEpoch;
        if (!this.flushFieldDrafts() || sourceEpoch !== this.valueEpoch || !this.canEdit()) return false;
        if (!this.dirty) return true; // Valid text normalization alone creates no history entry.
        let value;
        try { value=checkedGradient(this.normalizeValue?this.normalizeValue(checkedGradient(this.draft)):this.draft); }
        catch(error){this.notice(String(error.message));this.syncAvailability();return false;}
        if(sourceEpoch!==this.valueEpoch||!this.canEdit())return false;
        this.previewTask.cancel();this.draft=structuredClone(value);this.selected=Math.min(this.selected,value.stops.length-1);
        if(JSON.stringify(value)===JSON.stringify(this.original)){
            this.dirty=false;this.notice('');this.refresh({preserveDraft:false});
            this.emit(Message.CHANGE,{value:this.value,preview:true,normalized:true});
            if(this.onPreview)void this.previewTask.run(({signal})=>this.onPreview(this.value,{signal})).catch(error=>{if(error.code!=='CANCELLED'&&!this.destroyed)this.notice(error.message);});
            return true;
        }
        const epoch = this.valueEpoch; this.pending = true; this.refresh();
        try {
            const result = await this.onCommit?.(structuredClone(value));
            if (result === false) throw Error('Gradient was not applied');
            if (this.destroyed || epoch !== this.valueEpoch && JSON.stringify(value) !== JSON.stringify(this.original)) return false;
            this.original = value; super.setProperty('value', value); this.dirty = false; this.notice(''); this.emit(Message.VALUE_CHANGED, { value: this.value }); return true;
        } catch (error) { if (!this.destroyed && epoch === this.valueEpoch) this.notice(String(error.message)); return false; }
        finally { this.pending = false; if (!this.destroyed) this.refresh(); }
    }
    cancel() {
        if (this.pending) return false;
        this.valueEpoch++;
        this.previewTask.cancel(); this.draft = structuredClone(this.original); this.selected = 0; this.dirty = false; this.dragging = false; this.notice('');
        this.color.palette.palette.closeEditor(); this.refresh({ preserveDraft: false });
        this.emit(Message.CHANGE, { value: this.value, preview: true, cancelled: true });
        if (this.onPreview) void this.previewTask.run(({ signal }) => this.onPreview(this.value, { signal })).catch(() => {});
        return true;
    }
    destroy() { this.previewTask.close(); return super.destroy(); }
}
