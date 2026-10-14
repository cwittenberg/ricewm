import { documentEditorLayout, documentEditorPresentation, requestDocumentSelectionPoint } from './document-editor.mjs';
import {
    CollectionControl,
    CommandControl,
    ContainerControl,
    ContentControl,
    InputControl,
    numberInRange,
    oneOf,
    optionalString,
    OverlayControl,
    SelectionControl,
    State,
    SurfaceControl,
} from "./control.mjs";
import { Message, Notification } from "./messages.mjs";
import { TextEditorController, textEditorValue } from "./text-editor.mjs";
import { copyGridFrame, gridFrameHit, validateGridReferences, DOCUMENT_ALIGNMENT_CAPABILITY, GRID_MERGES_CAPABILITY, GRID_FRACTIONAL_FONT_CAPABILITY, GRID_DOCUMENT_CANVAS_CAPABILITY, GRID_ZOOM_CAPABILITY, GRID_PAINT_CAPABILITY, GRID_REFERENCES_CAPABILITY, GRID_FILL_ENDPOINT_CAPABILITY, GRID_TEXT_STYLE_CAPABILITY, GRID_LARGE_FRAME_CAPABILITY, GRID_FILTER_HEADER_CAPABILITY, GRID_FRAME_LEGACY_MAX_BYTES, GRID_FRAME_MAX_BYTES } from "./grid-frame.mjs";
import { encodeGridDamage, GRID_DAMAGE_CAPABILITY } from "./grid-damage.mjs";
import { SceneSurface } from "./scene-surface.mjs";
export { SceneSurface } from "./scene-surface.mjs";

const bool = value => Boolean(value);
const integer = (min, max, name) => value => {
    const number = Number(value);
    if (!Number.isSafeInteger(number) || number < min || number > max) {
        throw new RangeError(`${name} must be an integer from ${min} through ${max}`);
    }
    return number;
};
const stringList = (value, name) => {
    if (!Array.isArray(value)) throw new TypeError(`${name} must be an array`);
    return Object.freeze(value.map(item => String(item)));
};
const columnWidths = value => {
    if (!Array.isArray(value) || value.length < 2 || value.length > 32) {
        throw new RangeError("columnWidths must contain between 2 and 32 relative widths");
    }
    const result = value.map(item => {
        const width = Number(item);
        if (!Number.isSafeInteger(width) || width < 10 || width > 1_000) {
            throw new RangeError("each column width must be an integer from 10 through 1000");
        }
        return width;
    });
    if (result.reduce((total, width) => total + width, 0) > 4_000) {
        throw new RangeError("column width total exceeds 4000");
    }
    return Object.freeze(result);
};

export class Container extends ContainerControl { static kind = 1; }
export class Row extends Container { static kind = 2; }
export class Column extends Container { static kind = 3; }
export class Stack extends Container { static kind = 4; }
export class SplitPane extends Container {
    static kind = 5;
    static properties = Object.freeze({
        orientation: { default: "horizontal", validate: oneOf(["horizontal", "vertical"], "orientation"), reflect: false },
        position: { default: 0.5, validate: numberInRange(0, 1, "position"), reflect: false },
    });
    windowProc(current) {
        if (current.type === Message.POINTER && ["down", "move", "up"].includes(current.detail?.kind)) {
            const permille = Math.max(100, Math.min(900, Math.round(Number(current.detail?.x) || 0)));
            const suffix = String(this.text || "").split(":").slice(2).filter(Boolean);
            this.setProperty("position", permille / 1000);
            this.setProperty("text", [`split:${permille}`, ...suffix].join(":"));
        }
        return super.windowProc(current);
    }
}

export class Label extends ContentControl { static kind = 10; static get properties(){return TextField.properties;} nativeDocumentFont(){return TextField.prototype.nativeDocumentFont.call(this);} }
export class Image extends ContentControl {
    static kind = 11;
    static properties = Object.freeze({
        source: { default: null, validate: value => optionalString(value, "source"), reflect: false },
        fit: { default: "contain", validate: oneOf(["contain", "cover", "fill", "none"], "fit"), reflect: false },
    });
}

export class Button extends CommandControl { static kind = 12; static get properties(){return TextField.properties;} nativeDocumentFont(){return TextField.prototype.nativeDocumentFont.call(this);} }
export class Toggle extends CommandControl {
    static kind = 13;
    windowProc(current) {
        if (current.type === Message.ACTIVATE && this.values.enabled) {
            this.setProperty("checked", !this.values.checked);
        }
        return super.windowProc(current);
    }
}
export class CheckBox extends Toggle { static kind = 14; }
export class RadioButton extends Toggle {
    static kind = 15;
    static properties = Object.freeze({
        group: { default: "default", validate: value => String(value), reflect: false },
    });
    windowProc(current) {
        if (current.type === Message.ACTIVATE && this.values.enabled && !this.values.checked) {
            for (const sibling of this.parent?.children || []) {
                if (sibling !== this && sibling instanceof RadioButton && sibling.values.group === this.values.group) {
                    sibling.setProperty("checked", false);
                }
            }
            this.setProperty("checked", true);
            return CommandControl.prototype.windowProc.call(this, current);
        }
        if (current.type === Message.ACTIVATE && this.values.checked) return false;
        return super.windowProc(current);
    }
}
export class ProgressBar extends ContentControl {
    static kind = 16;
    static properties = Object.freeze({
        value: { default: 0, validate: numberInRange(0, 1, "value"), reflect: false },
        indeterminate: { default: false, validate: bool, stateBit: State.INDETERMINATE },
    });
    toWidget() {
        const widget = super.toWidget();
        const completed = Math.round(this.values.value * 1_000);
        return Object.freeze({ ...widget, text: `${completed}/1000` });
    }
}
export class Spinner extends ContentControl {
    static kind = 17;
    static properties = Object.freeze({ active: { default: true, validate: bool, stateBit: State.BUSY } });
}

export class TextField extends InputControl {
    static kind = 20;
    static properties = Object.freeze({ documentPaint: { default: null, reflect: true, validate(value) {
        if (value === null) return null;
        const color = color => typeof color === "string" && /^#[0-9a-f]{6}$/iu.test(color);
        if (!value || typeof value !== "object" || Array.isArray(value) || Object.keys(value).some(key => !["foreground", "background", "vertical"].includes(key))
            || (Object.hasOwn(value,"vertical") && !["top","center","bottom"].includes(value.vertical))
            || !Object.hasOwn(value,"background") || !color(value.background) || (Object.hasOwn(value,"foreground") && !color(value.foreground))) throw new TypeError("Invalid document paint");
        return Object.freeze({ background: value.background, ...(Object.hasOwn(value,"foreground") ? {foreground:value.foreground} : {}), ...(Object.hasOwn(value,"vertical") ? {vertical:value.vertical} : {}) });
    } }, documentZoom: { default: 100, reflect: true, validate: integer(25,400,"documentZoom") }, documentFont: { default: null, reflect: true, validate(value) {
        if (value === null) return null;
        if (!value || typeof value !== "object" || Array.isArray(value) || Object.keys(value).some(key => !["family","size_pt","weight","italic","underline"].includes(key)) || typeof value.family !== "string" || !value.family || new TextEncoder().encode(value.family).length > 128 || /[\u0000-\u001f\u007f-\u009f]/u.test(value.family) || !Number.isFinite(value.size_pt) || value.size_pt < 1 || value.size_pt > 409 || !Number.isInteger(value.weight) || value.weight < 100 || value.weight > 900 || value.weight % 100 || typeof value.italic !== "boolean" || typeof value.underline !== "boolean") throw new TypeError("Invalid document font");
        return Object.freeze({family:value.family,size_pt:value.size_pt,weight:value.weight,italic:value.italic,underline:value.underline});
    } } });
    nativeDocumentFont() {
        const capabilities = BigInt(this.application?.hostCapabilities || 0);
        if (![10,12,20,21].includes(this.constructor.kind) || !(capabilities & (1n << 18n)) || !this.values.documentFont) return null;
        if([10,12].includes(this.constructor.kind)&&!(capabilities&(1n<<29n)))throw Error('Native host needs scoped control typography support; refresh RiceWM.');
        if (!Number.isInteger(this.values.documentFont.size_pt) && !(capabilities & GRID_FRACTIONAL_FONT_CAPABILITY)) throw new RangeError("Host does not support fractional document fonts");
        const {vertical, ...legacyPaint} = this.values.documentPaint || {};
        const paint = capabilities & DOCUMENT_ALIGNMENT_CAPABILITY ? this.values.documentPaint : legacyPaint;
        return JSON.stringify({ ...this.values.documentFont,
            ...(capabilities & GRID_DOCUMENT_CANVAS_CAPABILITY && this.values.documentPaint ? {paint} : {}),
            ...(this.constructor.kind === 20 && capabilities & GRID_ZOOM_CAPABILITY && this.values.documentZoom !== 100 ? {zoom_percent:this.values.documentZoom} : {}) });
    }
    get selection() {
        return (BigInt(this.application?.hostCapabilities || 0) & (1n << 17n)) ? this.textSelection || null : null;
    }
    /** Unicode scalar offsets, matching TextEditor and native selection events. */
    setSelection(anchor, caret = anchor) {
        const length = Array.from(this.getProperty('value')).length;
        if (![anchor, caret].every(value => Number.isSafeInteger(value) && value >= 0 && value <= length)) throw new RangeError('Text selection exceeds field value');
        this.textSelection = Object.freeze({ anchor, caret });
        this.selectionRequest = Object.freeze({ anchor, caret, revision: (this.selectionRequest?.revision || 0) + 1 });
        this.application?.invalidate(this);
        return this.selection !== null;
    }
    nativeSelectionRequest() {
        if (!(BigInt(this.application?.hostCapabilities || 0) & (1n << 17n)) || !this.selectionRequest) return null;
        const length = Array.from(this.getProperty('value')).length;
        const selection = this.textSelection || { anchor: length, caret: length };
        return Object.freeze({ anchor: Math.min(selection.anchor, length), caret: Math.min(selection.caret, length), revision: this.selectionRequest.revision });
    }
    windowProc(current) {
        if (current.type === Message.VALUE_CHANGED) this.textSelection = null;
        if (current.type === Message.TEXT_SELECTION) {
            const { anchor, caret } = current.detail || {}, length = Array.from(this.getProperty('value')).length;
            if (![anchor, caret].every(value => Number.isSafeInteger(value) && value >= 0 && value <= length)) return false;
            this.textSelection = Object.freeze({ anchor, caret });
        }
        return super.windowProc(current);
    }
}
export class TextEditor extends TextField {
    static kind = 21;
    static properties = Object.freeze({
        value: { default: "", validate: textEditorValue },
        documentLayout: { default:null,reflect:true,validate:documentEditorLayout },
        documentZoom: { default:100,reflect:true,validate(value) {if(!Number.isFinite(value)||value<1||value>1600)throw new RangeError('Document editor zoom must be 1–1600 percent');return value;} },
        rows: { default: 5, validate: integer(1, 10_000, "rows"), reflect: false },
        wrap: { default: true, validate: bool },
        acceptTab: { default: false, validate: bool },
    });
    get selection() { const controller=this.editorController();controller.synchronize();return Object.freeze({anchor:controller.anchor,caret:controller.caret}); }
    setSelection(anchor,caret=anchor) { const controller=this.editorController(),length=controller.synchronize().length;if(![anchor,caret].every(value=>Number.isSafeInteger(value)&&value>=0&&value<=length))throw new RangeError('Text selection exceeds editor value');controller.selection(caret,anchor);return true; }
    setSelectionAt(point) { return requestDocumentSelectionPoint(this,point); }
    nativeSelectionRequest() { return null; }
    editorController() { return this.plainEditorController ||= new TextEditorController(this); }
    windowProc(current) {
        if ([Message.KEY, Message.TEXT_SELECTION].includes(current.type) && this.editorController().handle(current)) { current.preventDefault(); return true; }
        return super.windowProc(current);
    }
    toWidget() { const widget=super.toWidget();return Object.freeze({ ...widget, state:this.values.acceptTab?widget.state:widget.state|State.KEY_NAVIGATION, text: documentEditorPresentation(this,this.editorController().presentation()) }); }
}
export class ScrollView extends ContainerControl {
    static kind = 22;
    /** Observe the leading document section using the host's rendered geometry. */
    trackSections(controls) {
        if (!Array.isArray(controls) || controls.length > 64) throw new TypeError("Expected up to 64 sections");
        for (const control of controls) {
            let parent = control?.parent;
            while (parent && parent !== this) parent = parent.parent;
            if (parent !== this) throw new TypeError("Section must be a scroll descendant");
        }
        this.sections = [...controls];
        this.application?.invalidate(this);
    }
    toWidget() {
        const widget = super.toWidget();
        const ids = this.sections?.map(control => control.id).filter(Boolean);
        return ids?.length ? Object.freeze({ ...widget, text: `sections:${ids.join(",")}|${widget.text || "0/0"}` }) : widget;
    }
    /** Bring an attached descendant into view without measuring its geometry. */
    scrollTo(control) {
        let parent = control?.parent;
        while (parent && parent !== this) parent = parent.parent;
        if (parent !== this || !control.id || control.application !== this.application) {
            throw new TypeError("Scroll target must be an attached descendant");
        }
        this.setScrollPosition(`anchor:${control.id}`);
    }
    setScrollPosition(position) {
        if (typeof position !== "string" || !/^(?:\d+\/\d+|anchor:\d+)$/u.test(position)) return false;
        // Viewport bounds are layout metadata, not part of the changing offset.
        const viewport = /^viewport:(\d+)\|/u.exec(this.text || "");
        const height = viewport ? Number(viewport[1]) : 0;
        const metadata = (this.text || "").split('|')[0];
        const prefix = metadata.startsWith('panel:')
            ? metadata.replace(/:+$/u,'') + ':|'
            : metadata.startsWith('virtual-window:') ? metadata + '|'
            : Number.isSafeInteger(height) && height >= 64 && height <= 800 ? `viewport:${height}|` : "";
        this.setProperty("text", prefix + position);
        this.notifyParent(Notification.SCROLLED, { value: position });
        return true;
    }
    windowProc(current) {
        if (current.type === Message.VALUE_CHANGED) {
            // Wheel motion and scrollbar dragging are owned by RiceUI.
            const [position, section] = String(current.detail?.value ?? "").split(";section:");
            if (this.setScrollPosition(position)) {
                const control = this.sections?.find(control => String(control.id) === section) || null;
                this.emit(Message.SCROLL, { position, section: control });
            }
        }
        return super.windowProc(current);
    }
}
export class ScrollBar extends InputControl {
    static kind = 23;
    static properties = Object.freeze({
        value: { default: 0, validate: numberInRange(0, 1, "value"), reflect: false },
        orientation: { default: "vertical", validate: oneOf(["horizontal", "vertical"], "orientation"), reflect: false },
    });
    windowProc(current) {
        if (current.type === Message.VALUE_CHANGED && typeof current.detail?.value === 'string') {
            const match = /^(\d+)\/(\d+)$/u.exec(current.detail.value);
            if (!match || !Number.isSafeInteger(Number(match[1])) || !Number.isSafeInteger(Number(match[2])) || Number(match[2]) < 1 || Number(match[1]) > Number(match[2])) return false;
            current.detail = { ...current.detail, value: Number(match[1]) / Number(match[2]) };
        }
        const result = super.windowProc(current);
        if (current.type === Message.VALUE_CHANGED && result !== false) this.notifyParent(Notification.SCROLLED, current.detail);
        return result;
    }
    toWidget() {
        return Object.freeze({ ...super.toWidget(), text: `${Math.round(this.values.value * 1000000)}/1000000` });
    }
}
export class PasswordField extends TextField { static kind = 24; }
export class Markdown extends ContentControl {
    static kind = 25;
    static properties = Object.freeze({ selectable: { default: true, validate: bool, reflect: false } });
}

export class MenuBar extends ContainerControl { static kind = 30; }
export class Menu extends ContainerControl {
    static kind = 31;
    static properties = Object.freeze({ open: { default: false, validate: bool, reflect: false } });
}
export class MenuItem extends CommandControl {
    static kind = 32;
    static properties = Object.freeze({
        shortcut: { default: null, validate: value => optionalString(value, "shortcut"), reflect: false },
    });
}
export class ToolBar extends ContainerControl {
    static kind = 33;
    static authoring = Object.freeze({
        constructor: 'ToolBar(properties,[Button,Toggle,Menu,TextField,...]). Place the toolbar above content in a Column; use native layout tokens in text and State.EXPANSIVE for flexible content.',
        behavior: 'Wire Message.ACTIVATE on commands and Message.VALUE_CHANGED on inputs. Use State.START_ALIGNED for text-labeled buttons; give icon-only commands accessibleName and a Tooltip child. Keep accessories within the toolbar height and disable unavailable actions. Ribbon adds responsive overflow for command groups.',
        example: "const undo=new Button({text:'Undo',state:State.START_ALIGNED}); undo.on(Message.ACTIVATE,undoChange); const tools=new ToolBar({},[undo]);",
    });
}
export class Tabs extends CollectionControl { static kind = 34; }
export class Breadcrumb extends CollectionControl { static kind = 35; }
export class Slider extends InputControl {
    static kind = 36;
    static properties = Object.freeze({
        value: { default: 0, validate: value => Number(value) },
        min: { default: 0, validate: value => Number(value), reflect: false },
        max: { default: 100, validate: value => Number(value), reflect: false },
        step: { default: 1, validate: numberInRange(Number.MIN_VALUE, Number.MAX_VALUE, "step"), reflect: false },
    });
    setProperty(name, value, options) {
        if (name === "value") {
            const min = this.values?.min ?? 0;
            const max = this.values?.max ?? 100;
            value = Math.max(min, Math.min(max, Number(value)));
        }
        return super.setProperty(name, value, options);
    }
    windowProc(current) {
        if (current.type === Message.VALUE_CHANGED) {
            if (this.values.readOnly) return false;
            const match = /^(\d+)\/(\d+)$/u.exec(String(current.detail?.value || ""));
            if (!match || Number(match[2]) <= 0) return false;
            const fraction = Number(match[1]) / Number(match[2]);
            const raw = this.values.min + fraction * (this.values.max - this.values.min);
            const stepped = this.values.min +
                Math.round((raw - this.values.min) / this.values.step) * this.values.step;
            this.setProperty("value", Math.max(this.values.min, Math.min(this.values.max, stepped)));
            return true;
        }
        return super.windowProc(current);
    }
    toWidget() {
        const widget = super.toWidget();
        const range = this.values.max - this.values.min;
        const position = range > 0
            ? Math.round((this.values.value - this.values.min) * 1_000 / range)
            : 0;
        return Object.freeze({ ...widget, text: `${position}/1000` });
    }
}

export class List extends CollectionControl { static kind = 40; }
export class Table extends CollectionControl {
    static kind = 41;
    static properties = Object.freeze({
        columns: { default: Object.freeze([]), validate: value => stringList(value, "columns"), reflect: false },
        columnWidths: { default: Object.freeze([]), validate: value => value.length === 0 ? Object.freeze([]) : columnWidths(value), reflect: false },
        resizable: { default: false, validate: bool, reflect: false },
    });
    constructor(properties = {}, children = []) {
        super(properties, children);
        this.dragColumn = null;
        this.syncColumnMetadata({ render: false });
    }
    setProperty(name, value, options = {}) {
        const changed = super.setProperty(name, value, options);
        if (changed && (name === "columnWidths" || name === "resizable") && this.values?.columnWidths) {
            this.syncColumnMetadata(options);
        }
        return changed;
    }
    syncColumnMetadata({ render = true } = {}) {
        const widths = this.values.columnWidths || [];
        if (widths.length < 2) return;
        super.setProperty(
            "text",
            `table-columns:${widths.join(",")}${this.values.resizable ? ":resizable" : ""}`,
            { notify: false, render },
        );
    }
    windowProc(current) {
        if (current.type === Message.POINTER && this.values.resizable && this.values.columnWidths.length > 1) {
            const kind = current.detail?.kind;
            const position = Math.max(0, Math.min(1_000, Math.round(Number(current.detail?.x) || 0)));
            const widths = [...this.values.columnWidths];
            const total = widths.reduce((sum, width) => sum + width, 0);
            const scaled = Math.round(position * total / 1_000);
            if (kind === "down") {
                let boundary = 0;
                let nearest = { index: 0, distance: Number.POSITIVE_INFINITY };
                for (let index = 0; index + 1 < widths.length; index += 1) {
                    boundary += widths[index];
                    const distance = Math.abs(boundary - scaled);
                    if (distance < nearest.distance) nearest = { index, distance };
                }
                this.dragColumn = nearest.index;
            }
            if (["down", "move", "up"].includes(kind) && this.dragColumn !== null) {
                const index = this.dragColumn;
                const before = widths.slice(0, index).reduce((sum, width) => sum + width, 0);
                const pair = widths[index] + widths[index + 1];
                const minimum = Math.min(40, Math.floor(pair / 2));
                const first = Math.max(minimum, Math.min(pair - minimum, scaled - before));
                widths[index] = first;
                widths[index + 1] = pair - first;
                this.setProperty("columnWidths", widths);
            }
            if (kind === "up") this.dragColumn = null;
        }
        return super.windowProc(current);
    }
}
export class Tree extends CollectionControl { static kind = 42; }
export class IconGrid extends CollectionControl { static kind = 43; }
export class TreeItem extends CollectionControl {
    static kind = 44;
    static properties = Object.freeze({ expanded: { default: false, validate: bool, reflect: false } });
}

export class Dialog extends OverlayControl {
    static kind = 50;
    static properties = Object.freeze({
        open: { default: true, validate: bool, reflect: false },
        modal: { default: true, validate: bool, reflect: false },
    });
}
export class Popover extends OverlayControl { static kind = 51; }
export class Tooltip extends OverlayControl { static kind = 52; }
export class NotificationBanner extends OverlayControl {
    static kind = 53;
    static properties = Object.freeze({
        timeout: { default: 5_000, validate: integer(0, 86_400_000, "timeout"), reflect: false },
    });
}
export class Accordion extends CommandControl {
    static kind = 54;
    static properties = Object.freeze({ expanded: { default: false, validate: bool, reflect: false } });
    windowProc(current) {
        if (current.type === Message.ACTIVATE) this.setProperty("expanded", !this.values.expanded);
        return super.windowProc(current);
    }
}
export class GroupBox extends ContainerControl { static kind = 55; }

/** One native virtual viewport. Frame values are copied and frozen at ingress. */
export class VirtualGrid extends SurfaceControl {
    static kind = 45;
    static properties = Object.freeze({
        frame: { default: null, validate: value => value === null ? null : copyGridFrame(value) },
        canvas: { default: null, validate(value) { if (value !== null && value !== "document") throw new TypeError("Invalid document canvas"); return value; } },
        references: { default: null, validate: value => value === null ? null : Object.freeze(validateGridReferences(value).map(reference => Object.freeze({ ...reference }))) },
    });
    append() { throw new TypeError("VirtualGrid is a leaf; cells belong in its frame"); }
    hitTest(x, y) { return this.values.frame ? gridFrameHit(this.values.frame, x, y) : null; }
    setProperty(name, value, options) {
        const result = super.setProperty(name, value, options);
        // A caller using the ordinary property API supplies no complete damage
        // hint. Never reuse a preceding hint for this replacement frame.
        if (name === "frame" || name === "canvas") this._pendingGridDamage = null;
        return result;
    }
    presentFrame(frame, damage = null) {
        const previous = this.values.frame;
        let pending = null;
        if (damage?.fullPaint === false && (!Array.isArray(damage.changedCells) || damage.changedCells.length > 4096)) throw new RangeError("invalid native grid damage slots");
        if (previous && damage?.fullPaint === false && Array.isArray(damage.changedCells)) {
            const base = this._pendingGridDamage?.base || previous;
            const slots = new Map((this._pendingGridDamage?.slots || []).map(slot => [slot[0] * 512 + slot[1], slot]));
            for (const slot of damage.changedCells) slots.set(slot[0] * 512 + slot[1], slot);
            const ordered = [...slots].sort(([a], [b]) => a - b).map(([, slot]) => slot);
            // Validate eagerly so invalid hints cannot advance the local frame.
            if (encodeGridDamage(base, frame, ordered) !== null) pending = { base, slots: ordered };
        }
        this.setProperty("frame", frame);
        this._pendingGridDamage = pending;
        return this;
    }
    setFormulaReferences(references) {
        if (!this.values.frame) return this;
        validateGridReferences(references, this.values.frame.row_count, this.values.frame.column_count);
        if (JSON.stringify(this.values.references || []) === JSON.stringify(references)) return this;
        // Reference pointing is presentation damage, not a workbook mutation.
        // Preserve any pending engine damage and send no cells for a pure overlay.
        this._pendingGridDamage ||= { base: this.values.frame, slots: [] };
        this.setProperty("references", references);
        return this;
    }
    nativeFrame(frame = this.values.frame, references = this.values.references) {
        if (!frame) return frame;
        const capabilities = BigInt(this.application?.hostCapabilities || 0);
        let native = frame;
        if(frame.merges?.length && !(capabilities & GRID_MERGES_CAPABILITY)) throw new RangeError("Host does not support merged spreadsheet cells");
        const canvas = this.values.canvas ?? frame.canvas;
        if (canvas && capabilities & GRID_DOCUMENT_CANVAS_CAPABILITY) native = { ...native, canvas };
        else if (Object.hasOwn(native,"canvas")) { const {canvas, ...legacy} = native; native = legacy; }
        if (Object.hasOwn(native, "zoom_percent") && !(capabilities & GRID_ZOOM_CAPABILITY)) {
            if (native.zoom_percent !== 100) throw new RangeError("Host does not support document zoom");
            const {zoom_percent, ...legacy} = native; native = legacy;
        }
        const overlays = references ?? frame.references ?? [];
        validateGridReferences(overlays, frame.row_count, frame.column_count);
        if (capabilities & GRID_REFERENCES_CAPABILITY && overlays.length) native = { ...native, references: overlays };
        else if (Object.hasOwn(native, "references")) { const { references, ...base } = native; native = base; }
        if (Object.hasOwn(native, "fill_endpoint") && !(capabilities & GRID_FILL_ENDPOINT_CAPABILITY)) {
            const { fill_endpoint, ...legacy } = native;
            native = legacy;
        }
        if (!(capabilities & GRID_PAINT_CAPABILITY) && native.styles.some(style=>Object.hasOwn(style,"paint"))) native={...native,styles:native.styles.map(({paint,...style})=>style)};
        if (!(capabilities & GRID_TEXT_STYLE_CAPABILITY) && (capabilities & GRID_PAINT_CAPABILITY)) {
            // Paint and typography are independently negotiated. A paint host
            // understands the extended palette but need not accept font hints.
            native = { ...native, styles: native.styles.map(({font, ...style}) => style) };
        } else if (!(capabilities & GRID_TEXT_STYLE_CAPABILITY)
            && (native.styles.length > 64 || native.styles.some(style => Object.hasOwn(style, "font")))) {
            // Old hosts have a 64-style limit. Once optional fonts are removed,
            // the alignment/role palette has only twelve distinct entries.
            const styles = [], ids = new Map();
            const remap = native.styles.map(({ alignment, role }) => {
                const key = alignment * 4 + role;
                if (!ids.has(key)) { ids.set(key, styles.length); styles.push({ alignment, role }); }
                return ids.get(key);
            });
            native = { ...native, styles, cells: native.cells.map(([r, c, text, style]) => [r, c, text, remap[style]]) };
        }
        if (!(capabilities & GRID_FRACTIONAL_FONT_CAPABILITY) && native.styles.some(style => style.font && !Number.isInteger(style.font.size_pt))) throw new RangeError("Host does not support fractional document fonts");
        if (Object.hasOwn(native, "filter_header_row") && !(capabilities & GRID_FILTER_HEADER_CAPABILITY)) {
            const { filter_header_row, ...legacy } = native; native = legacy;
        }
        if (!(capabilities & GRID_LARGE_FRAME_CAPABILITY) && new TextEncoder().encode(JSON.stringify(native)).length > GRID_FRAME_LEGACY_MAX_BYTES) {
            throw new RangeError("Host does not support expanded grid frames");
        }
        return native;
    }
    nativeGridDamage(previousText) {
        const pending = this._pendingGridDamage;
        if (!(BigInt(this.application?.hostCapabilities || 0) & GRID_DAMAGE_CAPABILITY)
            || !pending || JSON.stringify(this.nativeFrame(pending.base, this._presentedReferences || [])) !== previousText) return null;
        return encodeGridDamage(this.nativeFrame(pending.base, this._presentedReferences || []), this.nativeFrame(), pending.slots,
            BigInt(this.application?.hostCapabilities || 0) & GRID_LARGE_FRAME_CAPABILITY ? GRID_FRAME_MAX_BYTES : GRID_FRAME_LEGACY_MAX_BYTES);
    }
    commitPresentation() { this._pendingGridDamage = null; this._presentedReferences = this.values.references ?? this.values.frame?.references ?? []; }
    toWidget() {
        if (!this.values.frame) throw new TypeError("VirtualGrid requires a frame");
        return Object.freeze({ ...super.toWidget(), text: JSON.stringify(this.nativeFrame()) });
    }
}

export class WebSurface extends SurfaceControl {
    static controlAvailability = 'host-owned; unavailable to generated apps';
    static kind = 60;
    static properties = Object.freeze({ url: { default: null, validate: value => optionalString(value, "url"), reflect: false } });
}
export class TerminalSurface extends SurfaceControl {
    static controlAvailability = 'host-owned; unavailable to generated apps';
    static kind = 61;
    static properties = Object.freeze({ session: { default: null, validate: value => optionalString(value, "session"), reflect: false } });
}
export class HostSurface extends SurfaceControl {
    static controlAvailability = 'host-owned; unavailable to generated apps';
    static kind = 62;
    static properties = Object.freeze({ service: { default: null, validate: value => optionalString(value, "service"), reflect: false } });
}

export const CONTROL_CLASSES = Object.freeze({
    Container, Row, Column, Stack, SplitPane, Label, Image, Button, Toggle, CheckBox,
    RadioButton, ProgressBar, Spinner, TextField, TextEditor, ScrollView, ScrollBar,
    PasswordField, Markdown, MenuBar, Menu, MenuItem, ToolBar, Tabs, Breadcrumb, Slider,
    List, Table, Tree, IconGrid, TreeItem, VirtualGrid, Dialog, Popover, Tooltip,
    Notification: NotificationBanner, Accordion, GroupBox, WebSurface, TerminalSurface,
    HostSurface, SceneSurface,
});

export function createControl(type, properties = {}, children = []) {
    const Constructor = CONTROL_CLASSES[type];
    if (!Constructor) throw new TypeError(`unknown RiceUI control ${type}`);
    return new Constructor(properties, children);
}

export const ui = Object.freeze(Object.fromEntries(
    Object.entries(CONTROL_CLASSES).map(([name, Constructor]) => [
        name[0].toLowerCase() + name.slice(1),
        (properties = {}, children = []) => new Constructor(properties, children),
    ]),
));
