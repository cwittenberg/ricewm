import { Column, TextField, ScrollView, Button, Label } from './controls.mjs';
import { CommandRegistry } from './commands.mjs';
import { Message } from './messages.mjs';
import { State } from './control.mjs';

/** Search and keyboard execution over the same commands as native menu/ribbon UI. */
export class CommandPalette extends Column {
    static family = 'composite';
    static constructorOptions = Object.freeze(['commands', 'onClose', 'limit']);
    static authoring = Object.freeze({ constructor: 'new CommandPalette({commands:new CommandRegistry([...]),onClose:()=>{...},limit:40})', methods: 'search(query), activate(id); displays disabled reasons and shortcuts. Arrow keys navigate, Enter invokes the authoritative command, Escape closes. Destroy releases command subscriptions.' });
    constructor({ commands, onClose = () => {}, limit = 40, ...options } = {}) {
        if (!(commands instanceof CommandRegistry) || typeof onClose !== 'function' || !Number.isInteger(limit) || limit < 1 || limit > 100) throw TypeError('Invalid command palette');
        super({ text: 'panel:fixed:gap=4', ...options });
        Object.assign(this, { commands, onClose, limit, query: '', selected: 0, rows: new Map() });
        this.input = this.append(new TextField({ name: `${this.name || 'commands'}-search`, accessibleName: 'Search commands' }));
        this.results = new Column({ text: 'panel:flush:fixed:gap=2' });
        this.scroll = this.append(new ScrollView({ text: 'viewport:340|0/0' }, [this.results]));
        this.status = this.append(new Label({ text: '', accessibleLive: 'polite' }));
        this.input.on(Message.VALUE_CHANGED, () => this.search(this.input.value));
        this.input.on(Message.ACTIVATE, () => this.activate(this.matches[this.selected]?.id));
        this.on(Message.KEY, event => {
            const d = event.detail;
            if (d?.pressed === false) return;
            if (event.target === this.input && (d?.key === 'Enter' || [36, 104].includes(d?.keyCode))) { event.preventDefault(); event.stopPropagation(); return this.activate(this.matches[this.selected]?.id); }
            if (d?.key === 'Escape' || d?.keyCode === 9) { event.preventDefault(); event.stopPropagation(); this.onClose(); }
            const delta = d?.key === 'ArrowUp' || d?.keyCode === 111 ? -1 : d?.key === 'ArrowDown' || d?.keyCode === 116 ? 1 : 0;
            if (delta && this.matches.length) {
                event.preventDefault(); event.stopPropagation(); this.selected = Math.max(0, Math.min(this.matches.length - 1, this.selected + delta));
                this.select(); this.scroll.scrollTo(this.rows.get(this.matches[this.selected].id));
            }
        });
        this.stopCommands = commands.subscribe(() => this.refresh());
        this.search('');
    }
    search(query) { this.query = query; this.selected = 0; this.refresh(); }
    refresh() {
        const previous = this.matches?.[this.selected]?.id;
        const all = this.commands.list({ query: this.query }); this.matches = all.slice(0, this.limit);
        const wanted = new Set(this.matches.map(item => item.id));
        for (const [id, row] of this.rows) if (!wanted.has(id)) { this.results.remove(row, { destroy: true }); this.rows.delete(id); }
        for (const item of this.matches) {
            let row = this.rows.get(item.id);
            if (!row) {
                row = new Button({ name: `palette-${item.id}`, state: State.START_ALIGNED, accessibleName: item.label });
                row.on(Message.ACTIVATE, () => this.activate(item.id)); this.rows.set(item.id, row);
            }
            row.text = item.shortcut ? `${item.label} (${item.shortcut})` : item.label;
            // Keep unavailable rows focusable so their reason is discoverable.
            row.setProperty('tooltip', item.disabledReason || item.label); row.setProperty('state', State.START_ALIGNED | (item.enabled ? 0 : State.MUTED));
            if (!row.parent) this.results.append(row);
        }
        const order = new Map(this.matches.map((item, index) => [this.rows.get(item.id), index]));
        if (this.results.children.some((row, index) => order.get(row) !== index)) {
            this.results.children.sort((a, b) => order.get(a) - order.get(b));
            this.application?.invalidate(this.results);
        }
        const retained = this.matches.findIndex(item => item.id === previous);
        this.selected = retained >= 0 ? retained : Math.min(this.selected, Math.max(0, this.matches.length - 1));
        this.status.text = all.length ? `${all.length} commands${all.length > this.limit ? '; refine your search' : ''}` : 'No matching commands'; this.select();
    }
    select() { for (const [id, row] of this.rows) row.setProperty('selected', id === this.matches[this.selected]?.id); }
    async activate(id) {
        const item = this.commands.list().find(item => item.id === id);
        if (!item || !this.enabled) return false;
        if (!item.enabled) { this.status.text = item.disabledReason; return false; }
        try {
            const result = await this.commands.execute(id);
            if (result !== false && !this.destroyed) this.onClose();
            return result;
        } catch (error) { if (!this.destroyed) this.status.text = String(error.message); return false; }
    }
    destroy() { this.stopCommands?.(); return super.destroy(); }
}
