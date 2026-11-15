import { Control, State } from './control.mjs';
import { Button, Column, Tabs } from './controls.mjs';
import { Message } from './messages.mjs';

/** Retained content for TabView. TabControl/Tabs remain the low-level strip. */
export class TabPage extends Column {
    static family = 'composite';
    static constructorOptions = Object.freeze(['id', 'title', 'icon']);
    static authoring = Object.freeze({
        constructor: 'new TabPage({id,title,icon:0,...properties},[native controls])',
        behavior:
            'Supply unique unparented pages to TabView. Content stays mounted across selection; use a ScrollView inside a page for a scrolling inspector.',
    });
    constructor({ id, title, icon = 0, ...properties } = {}, children = []) {
        if (
            typeof id !== 'string' ||
            !/^[A-Za-z][A-Za-z0-9_-]{0,47}$/.test(id) ||
            typeof title !== 'string' ||
            !title.trim() || icon !== null && (!Number.isInteger(icon) || icon < 0 || icon > 65535)
        )
            throw TypeError('TabPage needs a short id and a title');
        super({ text: 'panel:flush:gap=0', state: State.EXPANSIVE, ...properties }, children);
        this.pageId = id;
        this.title = title;
        // Native absence is null; sending icon 0 is not a valid icon patch.
        this.icon = icon || null;
    }
}

/** Native tab strip plus retained, automatically selected page content. */
export class TabView extends Column {
    static family = 'composite';
    static constructorOptions = Object.freeze(['pages', 'value', 'iconOnly']);
    static authoring = Object.freeze({
        constructor: 'new TabView({pages:[new TabPage({id,title,icon?},[content]),...],value?,iconOnly:false,...properties})',
        methods:
            'select(id), setPageState(id,{visible,enabled}); value is the selected page id. Message.SELECT detail.value reports a changed selection.',
        behavior:
            'Use for document views and inspector sections. iconOnly:true requires an icon on every page; native icon tabs retain accessible titles and tooltips. Native selected tabs and ArrowLeft/ArrowRight/Home/End navigation; hidden/disabled pages are skipped. Retains page controls, drafts, selection and scroll. Focus leaves hidden pages. Tabs/TabControl alone are only strips, not page managers.',
        example:
            "const views=new TabView({pages:[new TabPage({id:'content',title:'Content'},[editor]),new TabPage({id:'properties',title:'Properties'},[properties])]});",
    });
    constructor({ pages = [], value = pages[0]?.pageId ?? '', iconOnly = false, ...properties } = {}) {
        if (
            !Array.isArray(pages) ||
            pages.length < 1 ||
            pages.length > 32 ||
            pages.some((p) => !(p instanceof TabPage) || p.parent) ||
            new Set(pages.map((p) => p.pageId)).size !== pages.length
        )
            throw TypeError('TabView needs 1–32 unique unparented TabPages');
        if (!pages.some((p) => p.pageId === value)) throw RangeError('Unknown tab page');
        if (typeof iconOnly !== 'boolean' || iconOnly && pages.some(page => !page.icon)) throw TypeError('Icon tabs need an icon on every page');
        super({ text: 'panel:flush:gap=0', state: State.EXPANSIVE, ...properties });
        this.pages = new Map();
        this.value = value;
        this.strip = this.append(
            new Tabs({
                name: `${this.name || 'tab-view'}-tabs`,
                text: 'panel:tabs:integrated:flush:fixed:tight-stack',
            }),
        );
        this.body = this.append(new Column({ text: 'panel:flush:gap=0', state: State.EXPANSIVE }));
        for (const page of pages) {
            const button = this.strip.append(
                new Button({
                    text: iconOnly ? '' : page.title,
                    icon: page.icon,
                    name: `${this.name || 'tab-view'}-${page.pageId}`,
                    accessibleName: page.title,
                    tooltip: page.title,
                    state: State.FLAT,
                }),
            );
            const record = { page, button, visible: page.visible, enabled: page.enabled };
            this.pages.set(page.pageId, record);
            this.body.append(page);
            button.on(Message.ACTIVATE, () => this.select(page.pageId));
            button.on(Message.KEY, (e) => {
                if (e.detail?.pressed === false || !this.enabled) return;
                const key =
                    e.detail?.key ||
                    { 113: 'ArrowLeft', 114: 'ArrowRight', 110: 'Home', 115: 'End' }[e.detail?.keyCode];
                const ids = [...this.pages].filter(([, r]) => r.visible && r.enabled).map(([id]) => id),
                    i = ids.indexOf(page.pageId);
                const next =
                    key === 'Home'
                        ? 0
                        : key === 'End'
                          ? ids.length - 1
                          : key === 'ArrowLeft'
                            ? (i + ids.length - 1) % ids.length
                            : key === 'ArrowRight'
                              ? (i + 1) % ids.length
                              : null;
                if (next !== null && ids.length) {
                    this.select(ids[next], { focus: true });
                    e.preventDefault();
                }
            });
        }
        this.refresh();
    }
    select(id, { focus = false } = {}) {
        const record = this.pages.get(id);
        if (!record) throw RangeError('Unknown tab page');
        if (!this.enabled || !record.enabled || !record.visible) return false;
        const changed = this.value !== id;
        this.value = id;
        this.refresh(focus);
        if (changed) this.emit(Message.SELECT, { value: id });
        return changed;
    }
    setPageState(id, patch) {
        const record = this.pages.get(id);
        if (!record) throw RangeError('Unknown tab page');
        for (const key of ['visible', 'enabled'])
            if (Object.hasOwn(patch, key)) record[key] = Boolean(patch[key]);
        if (!this.pages.get(this.value)?.visible || !this.pages.get(this.value)?.enabled)
            this.value = [...this.pages].find(([, r]) => r.visible && r.enabled)?.[0] ?? '';
        const previous = this._selected;
        this.refresh();
        if (previous !== this.value) this.emit(Message.SELECT, { value: this.value });
    }
    refresh(focus = false) {
        let hiddenFocus = false;
        const clear = (control) => {
            if (control.getProperty('focused')) hiddenFocus = true;
            control.setProperty('focused', false);
            for (const child of control.children) clear(child);
        };
        for (const [id, r] of this.pages) {
            const selected = id === this.value && r.visible && r.enabled;
            if (!selected) {
                // Visibility removes native widgets; retain the field's actual
                // selection as an explicit request for its next native creation.
                if (r.page.visible) {
                    const retain = (control) => {
                        if (
                            [20, 24].includes(control.kind) &&
                            control.selection &&
                            typeof control.setSelection === 'function'
                        ) {
                            const { anchor, caret } = control.selection;
                            control.setSelection(anchor, caret);
                        }
                        for (const child of control.children) retain(child);
                    };
                    retain(r.page);
                }
                clear(r.page);
            }
            r.page.visible = selected;
            r.page.enabled = r.enabled;
            r.button.visible = r.visible;
            r.button.enabled = r.enabled;
            r.button.setProperty('selected', selected);
        }
        this._selected = this.value;
        if (focus || hiddenFocus) {
            let root = this;
            while (root.parent) root = root.parent;
            clear(root);
            this.pages.get(this.value)?.button.setProperty('focused', true);
        }
    }
}
