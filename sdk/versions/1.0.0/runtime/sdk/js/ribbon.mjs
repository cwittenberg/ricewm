import { Control, State } from './control.mjs';
import { Button, Column, Label, Menu, MenuBar, MenuItem, Row, Tabs, TextField, PasswordField, ToolBar, Tooltip } from './controls.mjs';
import { Message } from './messages.mjs';

const idPattern = /^[A-Za-z][A-Za-z0-9_-]{0,47}$/u;
function id(value) {
    if (typeof value !== 'string' || !idPattern.test(value)) throw new TypeError('Ribbon IDs must be short identifiers');
    return value;
}
function title(value) {
    if (typeof value !== 'string' || !value.trim() || value.length > 80) throw new TypeError('Ribbon titles must contain 1–80 characters');
    return value;
}
function walk(control, callback) {
    callback(control);
    for (const child of [...control.children]) walk(child, callback);
}
/** Materialize the existing native passive-hover primitive. */
function tooltips(control) {
    walk(control, item => {
        if (item.kind === Tooltip.kind) return;
        const text = item.getProperty('tooltip') || (!item.text && item.getProperty('accessibleName'));
        if (text && !item.children.some(child => child.kind === Tooltip.kind)) item.append(new Tooltip({ text }));
    });
}
/** Reuse caller controls; only relocate after editing focus has left the group. */
function moveRetained(control, destination) {
    const previous = control.parent;
    if (previous === destination) return;
    if (!previous || previous.application !== destination.application) throw new TypeError('Ribbon relocation must retain its owner');
    previous.remove(control); destination.append(control);
}
// Command popups size to their contents. START_ALIGNED | TRAILING_ICON is
// reserved for native choice selectors whose popup deliberately matches the anchor.
const overflowLabels = new WeakMap();
function moveCommand(control, destination) {
    const popup = destination.kind === Menu.kind;
    if (popup && !control.text && !overflowLabels.has(control)) {
        overflowLabels.set(control, control.text);
        control.text = control.getProperty('accessibleName') || control.getProperty('tooltip') || 'Command';
    } else if (!popup && overflowLabels.has(control)) {
        control.text = overflowLabels.get(control); overflowLabels.delete(control);
    }
    moveRetained(control, destination);
}
function focused(control) {
    return control.getProperty('focused') || control.children.some(focused);
}
function labelWidth(text) { return [...(text || '')].length * 8 + 32; }
function commandWidth(control, height) {
    const explicit = /(?:^|:)width=(\d+)(?=:|$)/u.exec(control.text)?.[1];
    if (explicit) return Number(explicit);
    if (control.getProperty('state') & State.LARGE_ICON) return Math.max(64, [...(control.text || '')].length * 8 + 20);
    if ((control.kind === Menu.kind || control.kind === Button.kind) && (control.getProperty('state') & State.START_ALIGNED)) {
        return labelWidth(control.text) + (control.getProperty('icon') == null ? 0 : 24)
            + (control.kind === Menu.kind && control.getProperty('icon') != null && ![75,74].includes(control.getProperty('icon')) ? 12 : 0);
    }
    if (control.kind === Menu.kind) return height + (control.getProperty('icon') == null ? 0 : 12);
    if (control.kind === Button.kind) return height;
    return Math.max(64, labelWidth(control.text));
}
function groupWidth(group) {
    if (group.rows) {
        const width = commands => commands.reduce((sum, control) => sum + commandWidth(control, group.toolHeight), 0) + Math.max(0, commands.length - 1) * 4;
        return group.minWidth ?? Math.max(labelWidth(group.title) - 16,
            width(group.primary) + (group.primary.length && group.rows.some(row => row.length) ? 8 : 0) + Math.max(...group.rows.map(width))) + 20;
    }
    return group.minWidth ?? Math.max(labelWidth(group.title) - 16,
        group.commands.reduce((sum, control) => sum + commandWidth(control, group.toolHeight), 0) + Math.max(0, group.commands.length - 1) * 8);
}
/** Shared retained native ribbon. Caller controls own application commands.
 * `presentation: 'office'` joins flat tabs to a fixed-height command surface.
 * Each group's `primary` count puts its leading commands above their captions;
 * `rowBreak` splits the remaining commands between the two compact tool rows.
 * Overflow always restores each command to its original row. */
export class Ribbon extends Column {
    static constructorOptions = Object.freeze(["tabs", "activeTab", "controlHeight", "presentation"]);
    static authoring = Object.freeze({
        constructor: 'Ribbon({presentation:"office"|"toolbar",tabs:[{id,title,groups:[{id,title,controls:[native controls],primary?,rowBreak?,minWidth?,visible?,enabled?}],visible?,enabled?}],activeTab?,controlHeight?}). IDs must be short identifiers; each command must be a unique unparented Control. primary and rowBreak are optional office layout counts. controlHeight is 24–64 pixels.',
        behavior: 'Native tabs, group captions, keyboard navigation, collapse and responsive overflow retain the original controls and callbacks. Wire Button/MenuItem Message.ACTIVATE and input Message.VALUE_CHANGED before adopting commands. For text-labeled toolbar commands use State.START_ALIGNED; icon-only actions need accessibleName and tooltip. Ribbon materializes Tooltip children.',
        methods: 'selectTab(id), setTabState(id,{visible,enabled}), setGroupState(tabId,groupId,{visible,enabled}), setCollapsed(boolean), setHeader({leading:[unparented controls],trailing:[unparented controls]}). Header can be set once; trailing ToolBar controls provide quick access with overflow. Native size events measure available width automatically.',
        validation: 'Place Ribbon above content in a Column. Test tab selection, contextual state, collapse and overflow at wide and narrow widths using native input and visual inspection. Commands in overflow must still perform their actions.',
        example: "const save=new Button({text:'Save',state:State.START_ALIGNED}); save.on(Message.ACTIVATE,saveDocument); const ribbon=new Ribbon({presentation:'office',tabs:[{id:'home',title:'Home',groups:[{id:'document',title:'Document',controls:[save]}]}]});",
    });
    constructor({ tabs = [], activeTab = null, controlHeight = null, presentation = 'toolbar', ...properties } = {}) {
        if (!['toolbar', 'office'].includes(presentation)) throw new RangeError('Unknown ribbon presentation');
        if (controlHeight !== null && (!Number.isInteger(controlHeight) || controlHeight < 24 || controlHeight > 64)) throw new RangeError('Ribbon controlHeight must be 24–64 pixels');
        if (!Array.isArray(tabs) || tabs.length > 12) throw new RangeError('Ribbon supports at most 12 tabs');
        // Validate ownership before adopting any caller control.
        const tabIds = new Set(), controls = new Set();
        for (const tab of tabs) {
            id(tab.id); title(tab.title);
            if (tabIds.has(tab.id)) throw new TypeError('Duplicate ribbon tab');
            tabIds.add(tab.id);
            if (!Array.isArray(tab.groups) || tab.groups.length > 12) throw new RangeError('Ribbon supports at most 12 groups per tab');
            const groups = new Set();
            for (const group of tab.groups) {
                id(group.id); title(group.title);
                if (group.minWidth !== undefined && (!Number.isFinite(group.minWidth) || group.minWidth < 48 || group.minWidth > 4096)) throw new RangeError('Ribbon group minWidth must be 48–4096 pixels');
                if (groups.has(group.id)) throw new TypeError('Duplicate ribbon group');
                groups.add(group.id);
                if (!Array.isArray(group.controls) || group.controls.length > 64) throw new RangeError('Ribbon group supports at most 64 controls');
                if (group.primary !== undefined && (!Number.isInteger(group.primary) || group.primary < 0 || group.primary > group.controls.length)) throw new RangeError('Ribbon primary count exceeds its controls');
                if (group.rowBreak !== undefined && (!Number.isInteger(group.rowBreak) || group.rowBreak < 0 || group.rowBreak > group.controls.length - (group.primary ?? 0))) throw new RangeError('Invalid ribbon row break');
                for (const control of group.controls) {
                    if (!(control instanceof Control) || control.parent || controls.has(control)) throw new TypeError('Ribbon controls must be unique unparented controls');
                    controls.add(control);
                }
            }
        }
        if (activeTab !== null && !tabIds.has(activeTab)) throw new RangeError('Unknown active ribbon tab');
        super({ text: 'panel:ribbon:flush:fixed:tight-stack', ...properties });
        this.presentation = presentation;
        this.collapsed = false; this.revealed = false;
        this.tabRecords = new Map(); this.activeTab = null;
        this.header = this.append(new Row({ name: 'ribbon-heading', text: `panel:ribbon-heading:flush:fixed:gap=0${presentation === 'office' ? ':office-ribbon-heading:accessory-height=38' : ''}` }));
        this.tabHost = this.header.append(new Column({ text: 'panel:flush:fixed:tight-stack', state: State.EXPANSIVE }));
        this.tabMenu = this.tabHost.append(new Menu({ name: 'ribbon-tab-menu', text: 'Ribbon tabs', icon: 74, state: State.START_ALIGNED | State.TRAILING_ICON, accessibleName: 'Ribbon tabs', visible: false }));
        this.tabMenu.on(Message.KEY, event => this.dismissKey(event) || this.navigateTabs(this.activeTab, event));
        this.compactTabs = false;
        this.collapseButton = presentation === 'office' ? this.header.append(new Button({ name: 'ribbon-collapse', text: '', icon: 73, state: State.FLAT, accessibleName: 'Collapse ribbon', tooltip: 'Collapse ribbon' })) : null;
        if (this.collapseButton) {
            tooltips(this.collapseButton);
            this.collapseButton.on(Message.ACTIVATE, () => this.setCollapsed(!this.collapsed));
            this.on(Message.CREATED, () => {
                let root = this; while (root.parent) root = root.parent;
                const observed = new WeakSet();
                const observe = branch => walk(branch, control => {
                    if (!this.contains(control) && !observed.has(control)) {
                        observed.add(control);
                        control.on(Message.CHILD_ADDED, event => observe(event.detail.child));
                        control.on(Message.FOCUS, () => this.dismiss());
                        control.on(Message.TEXT_SELECTION, () => this.dismiss());
                        control.on(Message.POINTER, event => {
                            if (event.detail?.button === 1 && (event.detail?.kind === 'up' ||
                                event.detail?.kind === 'down' && [TextField.kind, PasswordField.kind].includes(control.kind))) this.dismiss();
                        });
                    }
                });
                observe(root);
            });
        }
        this.tabs = this.tabHost.append(new Tabs({ name: 'ribbon-tabs', text: `panel:tabs:integrated:flush:fixed:tight-stack${presentation === 'office' ? ':accessory-height=38' : ''}` }));
        for (const tab of tabs) {
            const button = new Button({ name: `ribbon-tab-${tab.id}`, text: tab.title, state: presentation === 'office' ? State.FLAT : 0, icon: presentation === 'office' ? null : tab.icon ?? null, tooltip: tab.tooltip ?? null, accessibleName: tab.title });
            tooltips(button);
            const page = new Row({ name: `ribbon-page-${tab.id}`, text: `panel:ribbon-page:flush:fixed${presentation === 'office' ? ':office-ribbon-page:gap=0:accessory-height=112' : ''}`, visible: false });
            const groups = new Map();
            const compactMenu = new Menu({ name: `ribbon-commands-${tab.id}`, text: 'Commands', icon: 74, state: State.START_ALIGNED, accessibleName: `${tab.title} commands`, visible: false });
            for (const group of tab.groups) {
                group.controls.forEach(tooltips);
                group.controls.forEach(command => walk(command, control => {
                    if (control.kind === Button.kind || control.kind === MenuItem.kind) control.on(Message.ACTIVATE, () => this.dismiss());
                    control.on(Message.KEY, event => { if (event.detail?.pressed !== false && (event.detail?.key === 'Escape' || event.detail?.keyCode === 9)) this.dismiss(); });
                }));
                const destinations = new Map(), primary = [], rows = [];
                let tools;
                if (presentation === 'office') {
                    primary.push(...group.controls.slice(0, group.primary ?? 0));
                    const small = group.controls.slice(primary.length), split = group.rowBreak ?? Math.ceil(small.length / 2);
                    rows.push(small.slice(0, split), small.slice(split));
                    const bar = (commands, height) => {
                        const toolbar = new ToolBar({ text: `panel:office-ribbon-tools:flush:fixed:gap=4:accessory-height=${height}` }, commands);
                        for (const command of commands) destinations.set(command, toolbar);
                        return toolbar;
                    };
                    for (const command of primary) command.setProperty('state', (command.getProperty('state') & ~State.START_ALIGNED) | State.LARGE_ICON | State.FLAT);
                    tools = new Row({ text: 'panel:ribbon-tools:flush:fixed:gap=8:accessory-height=76' }, [
                        ...(primary.length ? [bar(primary, 76)] : []),
                        ...(small.length ? [new Column({ text: 'panel:flush:fixed:gap=8' }, rows.map(row => bar(row, 34)))] : []),
                    ]);
                } else {
                    tools = new ToolBar({ text: `panel:ribbon-tools:flush:fixed${controlHeight === null ? '' : `:accessory-height=${controlHeight}`}` }, group.controls);
                    for (const command of group.controls) destinations.set(command, tools);
                }
                const label = new Label({ text: group.title, state: State.MUTED | State.CENTER_ALIGNED });
                const caption = presentation === 'office' ? new Row({ text: 'panel:office-ribbon-caption:flush:fixed:accessory-height=24' }, [label]) : label;
                const overflow = new Menu({ name: `ribbon-overflow-${tab.id}-${group.id}`, text: group.title, icon: 74, state: State.START_ALIGNED, accessibleName: `${group.title} commands`, visible: false });
                const column = new Column({ name: `ribbon-group-${group.id}`, text: `panel:ribbon-group:fixed:tight-stack${presentation === 'office' ? ':office-ribbon-group:gap=0' : ':flush'}` }, [tools, caption, overflow]);
                const record = { control: column, tools, caption, overflow, commands: [...group.controls], submenuEntries: new Map(), collapsed: false, title: group.title, toolHeight: controlHeight ?? 32, compactEntry: new MenuItem({ text: group.title }),
                    minWidth: group.minWidth, destinations, primary, rows: presentation === 'office' ? rows : null, visible: group.visible !== false, enabled: group.enabled !== false };
                if (presentation === 'office') record.toolHeight = 34;
                for (const command of group.controls) {
                    walk(command, item => item.on(Message.BLUR, () => this.layoutOverflow()));
                    command.on(Message.CHANGE, event => {
                        if (['text', 'state', 'icon', 'visible'].includes(event.detail?.property)) this.layoutOverflow();
                    });
                    // Native submenus use MenuItem -> Menu, not Menu -> Menu.
                    // Keep the original menu, its children and action ownership.
                    if (command.kind === Menu.kind) {
                        const entry = new MenuItem();
                        const sync = () => {
                            for (const property of ['enabled', 'visible', 'icon', 'checked']) entry.setProperty(property, command.getProperty(property));
                            entry.text = command.text || command.getProperty('accessibleName') || command.getProperty('tooltip') || group.title;
                        };
                        sync(); command.on(Message.CHANGE, sync);
                        record.submenuEntries.set(command, entry);
                    }
                }
                tools.on(Message.SIZE_CHANGED, event => {
                    const height = event.detail.height;
                    if (presentation !== 'office' && !record.collapsed && height >= 24 && height <= 128 && height !== record.toolHeight) {
                        record.toolHeight = height; this.layoutOverflow();
                    }
                });
                groups.set(group.id, record); page.append(column);
            }
            const tabEntry = this.tabMenu.append(new MenuItem({ text: tab.title, icon: tab.icon ?? null }));
            tabEntry.on(Message.ACTIVATE, () => this.selectTab(tab.id));
            page.append(compactMenu);
            this.tabRecords.set(tab.id, { button, tabEntry, page, groups, compactMenu, compact: false, visible: tab.visible !== false, enabled: tab.enabled !== false });
            button.on(Message.ACTIVATE, () => { if (this.getProperty('enabled')) { if (this.collapsed && this.revealed && this.activeTab === tab.id) this.dismiss(); else this.selectTab(tab.id); } });
            button.on(Message.KEY, event => this.dismissKey(event) || this.navigateTabs(tab.id, event));
            this.tabs.append(button); this.append(page);
        }
        this.availableWidth = null;
        this.on(Message.SIZE_CHANGED, event => this.setAvailableWidth(event.detail.width));
        this.on(Message.CHANGE, event => { if (event.detail?.property === 'enabled') this.refresh(); });
        this.activeTab = activeTab;
        this.refresh();
    }
    /** Join file/quick-access controls to the native tab strip. */
    setHeader({ leading = [], trailing = [] } = {}) {
        if (!Array.isArray(leading) || !Array.isArray(trailing) || new Set([...leading, ...trailing]).size !== leading.length + trailing.length) throw new TypeError('Ribbon header controls must be unique arrays');
        if ([...leading, ...trailing].some(control => !(control instanceof Control) || control.parent)) throw new TypeError('Ribbon header controls must be unparented');
        for (const control of [...leading, ...trailing]) tooltips(control);
        if (this.headerControls) throw new Error('Ribbon header is already configured');
        this.header.remove(this.tabHost);
        if (this.collapseButton) this.header.remove(this.collapseButton);
        leading.forEach(control => this.header.append(control));
        this.header.append(this.tabHost);
        trailing.forEach(control => this.header.append(control));
        this.headerControls = [...leading, ...trailing, ...(this.collapseButton ? [this.collapseButton] : [])];
        // Standalone header badges stay beside the overflow button; only toolbars collapse.
        this.trailingControls = trailing.filter(control => control.kind === ToolBar.kind);
        this.quickCommands = this.trailingControls.flatMap(toolbar => toolbar.children.map(command => ({ command, toolbar })));
        for (const {command} of this.quickCommands) walk(command, item => item.on(Message.BLUR, () => this.layoutOverflow()));
        this.quickMenu = this.header.append(new Menu({ name: 'ribbon-quick-menu', text: 'Quick', accessibleName: 'Quick access', tooltip: 'Quick access commands', icon: 74, state: State.START_ALIGNED, visible: false }));
        tooltips(this.quickMenu);
        if (this.collapseButton) this.header.append(this.collapseButton);
        this.quickCollapsed = false;
        for (const control of trailing) control.on(Message.CHANGE, event => {
            if (['text', 'icon', 'visible'].includes(event.detail?.property)) this.layoutOverflow();
        });
        this.layoutOverflow();
    }
    refresh() {
        const previous = this.activeTab;
        const available = record => record?.visible && record.enabled;
        if (!available(this.tabRecords.get(this.activeTab))) this.activeTab = [...this.tabRecords].find(([, record]) => available(record))?.[0] ?? null;
        for (const [key, tab] of this.tabRecords) {
            const active = key === this.activeTab;
            tab.button.setProperty('visible', tab.visible);
            tab.button.setProperty('enabled', tab.enabled && this.getProperty('enabled'));
            tab.button.setProperty('checked', active);
            tab.button.setProperty('selected', active);
            tab.tabEntry.setProperty('checked', active);
            tab.tabEntry.setProperty('visible', tab.visible);
            tab.tabEntry.setProperty('enabled', tab.enabled && this.getProperty('enabled'));
            tab.page.setProperty('visible', active && (!this.collapsed || this.revealed));
            for (const group of tab.groups.values()) {
                group.control.setProperty('visible', group.visible && !tab.compact);
                group.compactEntry.setProperty('visible', group.visible);
                group.compactEntry.setProperty('enabled', group.enabled);
                group.control.setProperty('enabled', group.enabled);
            }
        }
        this.layoutOverflow();
        if (previous !== this.activeTab) this.emit(Message.SELECT, { value: this.activeTab });
    }
    /** Resolved native width, or explicit host sizing on older hosts. */
    setAvailableWidth(width) {
        if (!Number.isFinite(width) || width < 0 || width > 1000000) throw new RangeError('Invalid ribbon width');
        this.availableWidth = width; this.layoutOverflow();
    }
    layoutOverflow() {
        if (this.availableWidth == null || this.overflowLayoutActive) return;
        this.overflowLayoutActive = true;
        try {
            const previousCompactTabs = this.compactTabs;
            const widthOf = control => {
                if (control.kind === Button.kind) return commandWidth(control, 38) + 4;
                const commands = this.quickCommands?.filter(item => item.toolbar === control).map(item => item.command) ?? [];
                return (commands.length ? commands : control.children).reduce((width, item) => width + (control.kind === MenuBar.kind ? labelWidth(item.text || '') : commandWidth(item, 28)) + 2, 0);
            };
            const totalHeaderWidth = (this.headerControls ?? []).filter(control => control.getProperty('visible') || this.trailingControls?.includes(control)).reduce((sum, control) => sum + widthOf(control), 0);
            const showQuickAccess = this.availableWidth >= totalHeaderWidth + 100;
            if (this.quickMenu && !this.quickCommands.some(({command}) => focused(command)) && this.quickCollapsed === showQuickAccess) {
                for (const {command, toolbar} of this.quickCommands) moveCommand(command, showQuickAccess ? toolbar : this.quickMenu);
                this.quickCollapsed = !showQuickAccess;
                for (const control of this.trailingControls) control.setProperty('visible', showQuickAccess);
                this.quickMenu.setProperty('visible', !showQuickAccess);
            }
            const headerWidth = (this.headerControls ?? []).filter(control => control.getProperty('visible')).reduce((sum, control) => sum + widthOf(control), 0)
                + (this.quickCollapsed ? commandWidth(this.quickMenu, 28) : 0);
            this.compactTabs = [...this.tabRecords.values()].filter(tab => tab.visible)
                .reduce((sum, tab) => sum + labelWidth(tab.button.text) + (this.presentation === 'office' ? 16 : 0) + (tab.button.getProperty('icon') == null ? 0 : 24), 0) > Math.max(0, this.availableWidth - headerWidth);
            if (this.compactTabs !== previousCompactTabs) {
                if (this.compactTabs && [...this.tabRecords.values()].some(tab => tab.button.getProperty('focused'))) {
                    for (const tab of this.tabRecords.values()) tab.button.setProperty('focused', false);
                    this.tabMenu.setProperty('focused', true);
                } else if (!this.compactTabs && this.tabMenu.getProperty('focused')) {
                    this.tabMenu.setProperty('focused', false);
                    this.tabRecords.get(this.activeTab)?.button.setProperty('focused', true);
                }
            }
            this.tabs.setProperty('visible', !this.compactTabs);
            this.tabMenu.setProperty('visible', this.compactTabs);
            this.tabMenu.text = this.tabRecords.get(this.activeTab)?.button.text || 'Ribbon tabs';
            for (const tab of this.tabRecords.values()) {
                const groups = [...tab.groups.values()].filter(group => group.visible);
                const collapsed = new Set();
                let required = groups.reduce((sum, group) => sum + groupWidth(group), 0) + Math.max(0, groups.length - 1) * (this.presentation === 'office' ? 0 : 8);
                // Keep leading editing controls visible; collapse trailing groups only
                // when their named menu actually saves space.
                for (const group of [...groups].reverse()) {
                    if (required <= this.availableWidth) break;
                    const collapsedWidth = this.presentation === 'office' ? commandWidth(group.overflow, 28) + 12 : labelWidth(group.title);
                    const savings = groupWidth(group) - collapsedWidth;
                    if (savings > 0) { collapsed.add(group); required -= savings; }
                }
                const compact = required > this.availableWidth;
                if (compact) groups.forEach(group => collapsed.add(group));
                if (compact !== tab.compact && !groups.some(group => group.commands.some(focused))) {
                    for (const group of tab.groups.values()) {
                        if (compact) {
                            tab.compactMenu.append(group.compactEntry);
                            moveRetained(group.overflow, group.compactEntry);
                        } else {
                            moveRetained(group.overflow, group.control);
                            tab.compactMenu.remove(group.compactEntry);
                        }
                        group.control.setProperty('visible', group.visible && !compact);
                    }
                    tab.compact = compact;
                    tab.compactMenu.setProperty('visible', compact);
                }
                for (const group of tab.groups.values()) {
                    const collapse = tab.compact || collapsed.has(group);
                    if (group.collapsed === collapse || group.commands.some(focused)) continue;
                    // Preserve caller instances, values and listeners. Native IDs may change.
                    for (const command of group.commands) {
                        const entry = group.submenuEntries.get(command);
                        if (collapse && entry) {
                            group.overflow.append(entry);
                            moveRetained(command, entry);
                        } else {
                            moveCommand(command, collapse ? group.overflow : group.destinations.get(command));
                            if (entry?.parent) entry.parent.remove(entry);
                        }
                    }
                    group.collapsed = collapse;
                    group.tools.setProperty('visible', !collapse); group.caption.setProperty('visible', !collapse);
                    group.overflow.setProperty('visible', collapse);
                }
            }
        } finally { this.overflowLayoutActive = false; }
    }
    selectTab(value) {
        const tab = this.tabRecords.get(value);
        if (!tab) throw new RangeError('Unknown ribbon tab');
        if (!this.getProperty('enabled') || !tab.visible || !tab.enabled || (value === this.activeTab && (!this.collapsed || this.revealed))) return false;
        if (this.collapsed) this.revealed = true;
        this.activeTab = value; this.refresh(); this.emit(Message.SELECT, { value }); return true;
    }
    dismissKey(event) {
        if (event.detail?.pressed !== false && (event.detail?.key === 'Escape' || event.detail?.keyCode === 9) && this.dismiss()) {
            event.preventDefault(); return true;
        }
        return false;
    }
    /** Pin the full command surface open, or keep only the selected tab row. */
    setCollapsed(value) {
        if (typeof value !== 'boolean') throw new TypeError('Ribbon collapsed state must be boolean');
        this.collapsed = value; this.revealed = false;
        if (this.collapseButton) {
            const text = value ? 'Expand ribbon' : 'Collapse ribbon';
            this.collapseButton.setProperty('icon', value ? 74 : 73);
            this.collapseButton.setProperty('accessibleName', text);
            this.collapseButton.setProperty('tooltip', text);
            this.collapseButton.children.find(child => child.kind === Tooltip.kind).text = text;
        }
        this.refresh();
    }
    /** Dismiss temporary commands after an action or returning to the document. */
    dismiss() {
        if (!this.collapsed || !this.revealed) return false;
        this.revealed = false; this.refresh(); return true;
    }
    /** Move between available tabs without recreating a page or its commands. */
    navigateTabs(value, event) {
        if (!this.getProperty('enabled') || event.detail?.pressed === false) return false;
        const keys = ['ArrowLeft', 'ArrowRight', 'Home', 'End'];
        const key = event.detail?.key || ({113:'ArrowLeft',114:'ArrowRight',110:'Home',115:'End'})[event.detail?.keyCode];
        if (!keys.includes(key)) return false;
        const available = [...this.tabRecords].filter(([, tab]) => tab.visible && tab.enabled);
        const current = available.findIndex(([id]) => id === value);
        if (current < 0) return false;
        const next = key === 'Home' ? 0 : key === 'End' ? available.length - 1
            : (current + (key === 'ArrowLeft' ? -1 : 1) + available.length) % available.length;
        const [id, tab] = available[next];
        for (const record of this.tabRecords.values()) record.button.setProperty('focused', !this.compactTabs && record === tab);
        if (this.compactTabs) this.tabMenu.setProperty('focused', true);
        this.selectTab(id);
        event.preventDefault();
        return true;
    }
    setTabState(value, state) {
        const tab = this.tabRecords.get(value);
        if (!tab) throw new RangeError('Unknown ribbon tab');
        this.updateState(tab, state);
    }
    setGroupState(tabId, groupId, state) {
        const group = this.tabRecords.get(tabId)?.groups.get(groupId);
        if (!group) throw new RangeError('Unknown ribbon group');
        this.updateState(group, state);
    }
    updateState(record, state) {
        if (!state || Object.keys(state).some(key => !['visible', 'enabled'].includes(key) || typeof state[key] !== 'boolean')) throw new TypeError('Ribbon state accepts boolean visible/enabled');
        Object.assign(record, state); this.refresh();
    }
}
