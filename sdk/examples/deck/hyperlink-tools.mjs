import { Button, Dialog, Label, Menu, MenuItem, Message, Row, State, TextField } from '@ricewm/app';
import { checkedHyperlinkCommand, supportedLink } from './hyperlink-contract.mjs';

/** The document adapter owns links; this dialog holds only an uncommitted choice. */
export function createHyperlinkTools({ root, apply, open, slides, notice = () => {} }) {
    let state = { ready: false, selected: [], scene: null, revision: null }, dialog = null;
    const control = new Button({ name: 'insert-hyperlink', text: 'Link', icon: 80, state: State.START_ALIGNED,
        tooltip: 'Insert or edit link (Ctrl+K)', accessibleName: 'Insert or edit link', enabled: false });
    const current = () => state.selected.length === 1 ? state.selected[0] : null;
    const usable = () => state.ready && current()?.canEditHyperlink === true;
    const close = () => { if (dialog) root.remove(dialog, { destroy: true }); dialog = null; };
    function edit() {
        if (!state.ready || !current() || (!usable() && !current().hyperlinks?.length) || dialog) return;
        const object = current(), slide = state.scene.slideId, revision = state.revision;
        const links = object.hyperlinks || [], runs = object.text?.runs || [];
        let run = links.length === 1 ? links[0].run : null, kind = 'url', destination = null;
        const scope = new Menu({ name: 'hyperlink-scope', text: 'Whole object' });
        const type = new Menu({ name: 'hyperlink-type', text: 'Web address' });
        const address = new TextField({ name: 'hyperlink-address', value: '', placeholder: 'https://example.com' });
        const slideChoice = new Menu({ name: 'hyperlink-slide', text: 'Choose a slide', visible: false });
        const error = new Label({ name: 'hyperlink-error', text: '', visible: false });
        const remove = new Button({ name: 'hyperlink-remove', text: 'Remove link', enabled: false });
        const visit = new Button({ name: 'hyperlink-open', text: 'Open link', enabled: false });
        const save = new Button({ name: 'hyperlink-apply', text: 'Apply', accent: true, enabled: usable() });
        const cancel = new Button({ name: 'hyperlink-cancel', text: 'Cancel' });
        const choices = slides();
        function updateType(value) {
            kind = value; type.text = value === 'url' ? 'Web address' : 'Slide in this presentation';
            address.setProperty('visible', value === 'url'); slideChoice.setProperty('visible', value === 'slide');
        }
        function chooseScope(value, label) {
            run = value; scope.text = label;
            const link = links.find(link => link.run === run);
            remove.setProperty('enabled', usable() && Boolean(link)); visit.setProperty('enabled', supportedLink(link));
            address.setProperty('value', link?.target.kind === 'url' ? link.target.value : '');
            destination = link?.target.kind === 'slide' ? link.target.value : null;
            slideChoice.text = choices.find(item => item.id === destination)?.label || 'Choose a slide';
            updateType(link?.target.kind === 'slide' ? 'slide' : 'url');
            error.text = link?.target.kind === 'unsupported' ? 'This link type cannot be opened here. You can replace or remove it.' : '';
            error.setProperty('visible', Boolean(error.text));
        }
        function item(menu, text, action) { const item = new MenuItem({ text }); item.on(Message.ACTIVATE, action); menu.append(item); }
        item(scope, 'Whole object', () => chooseScope(null, 'Whole object'));
        // Keep a large imported object bounded without omitting its existing links.
        const scopes = runs.length <= 128 ? runs : runs.filter(r => links.some(link => link.run === r.id));
        for (const entry of scopes.slice(0,128)) {
            const label = entry.text.replace(/\s+/gu, ' ').slice(0,70) || '(Empty text run)';
            item(scope, label, () => chooseScope(entry.id,label));
        }
        item(type, 'Web address', () => updateType('url'));
        item(type, 'Slide in this presentation', () => updateType('slide'));
        for (const entry of choices) item(slideChoice, entry.label, () => { destination = entry.id; slideChoice.text = entry.label; });
        chooseScope(run, run ? runs.find(r => r.id === run)?.text.slice(0,70) || 'Linked text' : 'Whole object');
        async function commit(target) {
            if (!usable() || state.revision !== revision || current()?.id !== object.id || state.scene?.slideId !== slide) { close(); return; }
            try {
                const command = checkedHyperlinkCommand({ type: 'set-hyperlink', slide, object: object.id, run, target });
                close(); await apply([command]);
            } catch (cause) { if (dialog) { error.text = String(cause.message); error.setProperty('visible',true); } else notice(String(cause.message)); }
        }
        save.on(Message.ACTIVATE, () => void commit({ kind, value: kind === 'url' ? address.getProperty('value').trim() : destination }));
        remove.on(Message.ACTIVATE, () => void commit(null));
        visit.on(Message.ACTIVATE, () => { const link = links.find(link => link.run === run); if (supportedLink(link)) { close(); void open(link); } });
        cancel.on(Message.ACTIVATE, close);
        dialog = new Dialog({ name: 'hyperlink-dialog', text: links.length ? 'Edit hyperlink' : 'Insert hyperlink', modal: true }, [
            new Label({ text: 'Apply link to' }), scope, new Label({ text: 'Link to' }), type, address, slideChoice, error,
            new Row({ text: 'panel:actions' }, [remove,visit,cancel,save]),
        ]);
        dialog.on(Message.KEY, event => { if (event.detail?.pressed && event.detail.keyCode === 9) { close(); event.preventDefault(); } });
        root.append(dialog);
    }
    control.on(Message.ACTIVATE, edit);
    return { controls: [control], edit, close,
        open() { const links = current()?.hyperlinks?.filter(supportedLink) || []; if (links.length === 1) void open(links[0]); else edit(); },
        remove() { const object = current(); if (!usable()) return;
            const links = object.hyperlinks || []; if (links.length !== 1) return edit();
            void apply([{ type: 'set-hyperlink', slide: state.scene.slideId, object: object.id, run: links[0].run, target: null }]); },
        setState(next) { if (dialog && (next.revision !== state.revision || next.selected?.[0]?.id !== state.selected[0]?.id || !next.ready)) close();
            state = { ...state,...next }; control.setProperty('enabled', usable()); },
    };
}
