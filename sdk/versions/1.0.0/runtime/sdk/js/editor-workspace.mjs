import { Column, SplitPane } from './controls.mjs';
import { Control, State } from './control.mjs';
import { Message } from './messages.mjs';

const modes = new Set(['normal', 'outline', 'sorter', 'notesPage', 'reading']);
/** Retained editor panes and view transitions. Documents and selection remain caller-owned. */
export class EditorWorkspace extends Column {
    static family = 'composite';
    static constructorOptions = Object.freeze(['navigation', 'document', 'inspector', 'outline', 'notesPage', 'beforeChange', 'onChange', 'navigationWidth', 'inspectorWidth']);
    static authoring = Object.freeze({ constructor: 'new EditorWorkspace({navigation,document,inspector,outline,notesPage,beforeChange:async()=>true,onChange:async(mode)=>{...}})', methods: 'setMode("normal"|"outline"|"sorter"|"notesPage"|"reading"), setInspectorVisible(boolean); retained panes and splitter positions. Changes wait for caller draft validation. The same inspector remains available in normal, outline, sorter and notesPage views. Compact layouts retain explicit access to the inspector.' });
    constructor({ navigation, document, inspector = null, outline = null, notesPage = null, beforeChange = async () => true, onChange = async () => {}, navigationWidth = 236, inspectorWidth = 300, ...options } = {}) {
        const controls = [navigation, document, inspector, outline, notesPage].filter(Boolean);
        if (!(navigation instanceof Control) || !(document instanceof Control) || controls.some(control => !(control instanceof Control) || control.parent) || new Set(controls).size !== controls.length || typeof beforeChange !== 'function' || typeof onChange !== 'function' || ![navigationWidth, inspectorWidth].every(n => Number.isFinite(n) && n >= 100 && n <= 1000)) throw TypeError('Invalid editor workspace');
        super({ text: 'panel:flush:gap=0', state: State.EXPANSIVE, ...options });
        Object.assign(this, { navigation, document, inspector, outline, notesPage, beforeChange, onChange, mode: 'normal', pending: false, inspectorVisible: true, width: 1600, navigationWidth, inspectorWidth });
        this.navigationHost = new Column({ name: 'navigation-host', text: 'panel:flush:gap=0' }, [navigation]);
        if (outline) { outline.visible = false; this.navigationHost.append(outline); }
        this.documentHost = new Column({ text: 'panel:flush:gap=0', state: State.EXPANSIVE }, [document]);
        this.inner = new SplitPane({ name: 'slides-canvas', text: 'split:150:min-width=160:max-width=400', state: State.EXPANSIVE }, [this.navigationHost, this.documentHost]);
        this.sorterHost = new Column({ name: 'slide-overview', text: 'panel:flush:gap=4', state: State.EXPANSIVE, visible: false });
        if (notesPage) notesPage.visible = false;
        // All editing views share this split. Hiding the entire split in sorter
        // or notes mode also hid an explicitly opened task pane.
        this.main = new SplitPane({ name: 'editor-panes', text: 'split:190:trailing:min-width=240:max-width=520', state: State.EXPANSIVE }, [this.inner, this.sorterHost, ...(notesPage ? [notesPage] : [])]);
        this.bindWidth(this.inner, 'navigationWidth', false, 160, 400);
        this.bindWidth(this.main, 'inspectorWidth', true, 240, 520);
        if (inspector) this.main.append(inspector);
        this.append(this.main);
        this.on(Message.SIZE_CHANGED, event => { if (event.target !== this || !(event.detail?.width > 0)) return; this.width = event.detail.width; this.layout(); });
    }
    bindWidth(split, property, trailing, minimum, maximum) {
        let available = 0, sizing = false;
        split.on(Message.SIZE_CHANGED, event => {
            if (event.target !== split || !(event.detail?.width > 0)) return;
            available = Math.max(1, event.detail.width - 4);
            const permille = Math.max(100, Math.min(900, Math.round(this[property] / available * 1000)));
            sizing = true;
            try { split.text = `split:${permille}${trailing ? ':trailing' : ''}:min-width=${minimum}:max-width=${maximum}`; }
            finally { sizing = false; }
        });
        split.on(Message.CHANGE, event => {
            if (sizing || !available || event.target !== split || event.detail?.property !== 'text') return;
            const value = Number(/^split:(\d+)/u.exec(split.text)?.[1]);
            if (value) this[property] = Math.max(minimum, Math.min(maximum, available * value / 1000));
        });
    }
    async setMode(mode) {
        if (!modes.has(mode)) throw RangeError('Unknown editor view');
        if (mode === 'outline' && !this.outline || mode === 'notesPage' && !this.notesPage) return false;
        if (this.pending || this.destroyed) return false;
        if (mode === this.mode) return true;
        this.pending = true;
        const previous = this.mode;
        try {
            if (await this.beforeChange(mode, previous) === false || this.destroyed) return false;
            this.mode = mode; this.layout(); await this.onChange(mode, previous);
            if (!this.destroyed) this.emit(Message.CHANGE, { mode, previous });
            return true;
        } catch (error) { if (!this.destroyed) { this.mode = previous; this.layout(); } throw error; }
        finally { this.pending = false; }
    }
    setInspectorVisible(value) { this.inspectorVisible = Boolean(value); this.inspectorExplicit = this.inspectorVisible; this.layout(); }
    layout() {
        if (this.destroyed) return;
        const sorter = this.mode === 'sorter', reading = this.mode === 'reading', notes = this.mode === 'notesPage';
        const host = sorter ? this.sorterHost : this.navigationHost;
        if (this.navigation.parent !== host) this.navigation.moveTo(host);
        this.navigation.visible = this.mode !== 'outline';
        if (this.outline) this.outline.visible = this.mode === 'outline';
        this.main.visible = true; this.inner.visible = !sorter && !notes; this.sorterHost.visible = sorter;
        if (this.notesPage) this.notesPage.visible = notes;
        const inspect = Boolean(this.inspector && this.inspectorVisible && !reading && (this.width >= 900 || this.inspectorExplicit));
        if (this.inspector) this.inspector.visible = inspect;
        this.navigationHost.visible = !reading && !(inspect && this.width < 700);
        this.documentHost.visible = !(inspect && this.width < 500);
    }
}
