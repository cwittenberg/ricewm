import { Button, CheckBox, ColorMenu, Column, Container, Dialog, Label, Menu, MenuItem, Message, Row, SceneBuilder, SceneSurface, ScrollView, State, TextField } from '@ricewm/app';

const PRESETS = Object.freeze([
    ['plain', 'Plain', '#FFFFFF', '#F2F2F2'], ['accent-1', 'Blue', '#4472C4', '#D9E2F3'],
    ['accent-2', 'Orange', '#ED7D31', '#FBE5D6'], ['accent-3', 'Green', '#70AD47', '#E2F0D9'],
    ['accent-4', 'Purple', '#8064A2', '#E4DFEC'], ['accent-5', 'Light blue', '#5B9BD5', '#DDEBF7'], ['accent-6', 'Red', '#C0504D', '#F2DCDB'],
]);
const SIDES = ['top', 'right', 'bottom', 'left'];
const rgba = hex => (0xff000000 | Number.parseInt(hex.slice(5, 7), 16) << 16 | Number.parseInt(hex.slice(3, 5), 16) << 8 | Number.parseInt(hex.slice(1, 3), 16)) >>> 0;
const presetIds = new Set(PRESETS.map(item => item[0]));

export function tableStyleThumbnail(preset, { headerRow = true, bandedRows = true } = {}) {
    const entry = PRESETS.find(item => item[0] === preset);
    if (!entry) throw new RangeError('Unknown table style');
    const builder = new SceneBuilder(96, 64).rect([0, 0, 96, 64], 0xffffffff);
    for (let row = 0; row < 4; row++) {
        const fill = row === 0 && headerRow ? entry[2] : row % 2 && bandedRows ? entry[3] : '#FFFFFF';
        for (let column = 0; column < 3; column++) {
            builder.rect([column * 32, row * 16, 32, 16], rgba(fill));
            builder.line([column * 32 + 6, row * 16 + 8], [column * 32 + 24, row * 16 + 8], 2,
                row === 0 && headerRow && preset !== 'plain' ? 0xffffffff : 0xff555555);
        }
    }
    for (let row = 0; row <= 4; row++) builder.line([0, row * 16], [96, row * 16], 1, 0xffaaaaaa);
    for (let column = 0; column <= 3; column++) builder.line([column * 32, 0], [column * 32, 64], 1, 0xffaaaaaa);
    return builder.finish();
}

/** Native contextual table tools. Cell edits never silently become whole-table edits. */
export function createTableFormatTools({ root, canvas, notice = () => {} }) {
    let table = null, selectedCell = null, usable = false, cellEnabled = false, structureEnabled = false;
    let borderColor = '#000000', borderWidth = 1, dialog = null, cellIdentity = '', previewFlags = '';
    const knownPresets = new Map(), controls = [];
    const close = () => { if (dialog) root.remove(dialog, { destroy: true }); dialog = null; };
    const run = callback => {
        try { Promise.resolve(callback()).catch(error => notice(error.message || 'Could not edit table')); }
        catch (error) { notice(error.message || 'Could not edit table'); }
    };
    const track = (control, requires = 'cell') => { controls.push({ control, requires }); return control; };
    const command = (name, label, icon, callback, requires = 'cell') => {
        const control = track(new Button({ name, text: '', icon, state: State.DISPLAY, tooltip: label, accessibleName: label, enabled: false }), requires);
        control.on(Message.ACTIVATE, () => { if (control.getProperty('enabled')) run(callback); }); return control;
    };
    const menu = (name, label, icon, requires = 'cell') => track(new Menu({ name, text: '', icon, state: State.DISPLAY,
        tooltip: label, accessibleName: label, enabled: false }), requires);
    const item = (owner, name, label, icon, callback) => {
        const child = owner.append(new MenuItem({ name, text: label, icon }));
        child.on(Message.ACTIVATE, () => { if (owner.getProperty('enabled') && child.getProperty('enabled')) run(callback); }); return child;
    };
    const cellStyle = style => { if (cellEnabled) return canvas.tableCellStyle(style); };
    const activePreset = () => table && (presetIds.has(table.table.preset) ? table.table.preset : knownPresets.get(table.id));
    const applyPreset = (preset, flags = {}) => {
        if (!usable || !table || !presetIds.has(preset)) return;
        const id = table.id;
        return Promise.resolve(canvas.tableStyle({ preset, headerRow: table.table.headerRow === true, bandedRows: table.table.bandedRows === true, ...flags })).then(() => {
            if (knownPresets.size >= 128 && !knownPresets.has(id)) knownPresets.delete(knownPresets.keys().next().value);
            knownPresets.set(id, preset); updateControls();
        });
    };
    const styles = menu('table-styles', 'Table styles', 41, 'table');
    styles.text = 'Table Styles';
    const gallery = new Column({ name: 'table-style-gallery', text: 'panel:menu-grid:columns=4:fixed:flush:gap=4' });
    const previews = [];
    for (let start = 0; start < PRESETS.length; start += 4) {
        const row = gallery.append(new Row({ text: 'panel:fixed:flush:gap=4' }));
        for (const [id, label] of PRESETS.slice(start, start + 4)) {
            const preview = new SceneSurface({ name: `table-style-preview-${id}`, enabled: false });
            preview.submit(tableStyleThumbnail(id));
            const choice = row.append(new MenuItem({ name: `table-style-${id}`, text: 'panel:fixed:accessory-height=64:width=96:flush', tooltip: label, accessibleName: `${label} table style` }, [
                new Container({ text: 'panel:fixed:accessory-height=56:width=88:flush', state: State.FLAT }, [preview]),
            ]));
            choice.on(Message.ACTIVATE, () => { if (styles.getProperty('enabled')) run(() => applyPreset(id)); });
            previews.push({ choice, id, preview });
        }
    }
    styles.append(new ScrollView({ name: 'table-styles-scroll', text: 'viewport:180|0/0' }, [gallery]));
    const header = track(new CheckBox({ name: 'table-header-row', text: 'Header Row', enabled: false }), 'preset');
    const banded = track(new CheckBox({ name: 'table-banded-rows', text: 'Banded Rows', enabled: false }), 'preset');
    header.on(Message.ACTIVATE, () => { if (header.getProperty('enabled')) run(() => applyPreset(activePreset(), { headerRow: header.getProperty('checked') })); });
    banded.on(Message.ACTIVATE, () => { if (banded.getProperty('enabled')) run(() => applyPreset(activePreset(), { bandedRows: banded.getProperty('checked') })); });
    const palettes = [];
    function palette(name, label, icon, callback, allowNone = false) {
        const control = track(new ColorMenu({ name, label, icon, allowNone, noneLabel: 'No shading', enabled: false }));
        control.on(Message.VALUE_CHANGED, event => { if (control.getProperty('enabled')) run(() => callback(event.detail.value)); });
        palettes.push({ name, control }); return control;
    }
    const shading = palette('table-cell-shading', 'Cell shading', 186, fill => cellStyle({ fill }), true);
    const border = sides => Object.fromEntries(sides.map(side => [side, { color: borderColor, width: borderWidth }]));
    const existingBorders = () => SIDES.filter(side => selectedCell?.cell?.borders?.[side]);
    const borderPalette = palette('table-border-color', 'Border color', 187, color => {
        borderColor = color;
        const sides = existingBorders();
        if (sides.length) return cellStyle({ borders: Object.fromEntries(sides.map(side => [side, { color, width: selectedCell.cell.borders[side].width }])) });
    });
    const weight = menu('table-border-weight', 'Border weight', 183);
    for (const points of [0.5, 1, 1.5, 2.25, 3, 4.5, 6]) item(weight, `table-border-weight-${String(points).replace('.', '-')}`, `${points} pt`, 183, () => {
        borderWidth = points * 96 / 72;
        const sides = existingBorders();
        if (sides.length) return cellStyle({ borders: Object.fromEntries(sides.map(side => [side, { color: selectedCell.cell.borders[side].color, width: borderWidth }])) });
    });
    const borders = menu('table-cell-borders', 'Cell borders', 41);
    item(borders, 'table-borders-all', 'All borders', 41, () => cellStyle({ borders: border(SIDES) }));
    item(borders, 'table-borders-none', 'No borders', 25, () => cellStyle({ borders: Object.fromEntries(SIDES.map(side => [side, null])) }));
    for (const [side, icon] of [['top', 233], ['bottom', 235], ['left', 230], ['right', 232]]) item(borders, `table-border-${side}`, `${side[0].toUpperCase()}${side.slice(1)} border`, icon, () => cellStyle({ borders: border([side]) }));
    const structure = (type, axis, offset = 0) => { if (structureEnabled) return canvas.tableStructure(type, selectedCell[axis] + offset, 1); };
    const structureControls = [
        command('table-insert-row-above', 'Insert row above', 178, () => structure('insert-table-rows', 'row'), 'structure'),
        command('table-insert-row-below', 'Insert row below', 169, () => structure('insert-table-rows', 'row', 1), 'structure'),
        command('table-insert-column-left', 'Insert column left', 179, () => structure('insert-table-columns', 'column'), 'structure'),
        command('table-insert-column-right', 'Insert column right', 170, () => structure('insert-table-columns', 'column', 1), 'structure'),
        command('table-delete-row', 'Delete row', 180, () => structure('delete-table-rows', 'row'), 'delete-row'),
        command('table-delete-column', 'Delete column', 181, () => structure('delete-table-columns', 'column'), 'delete-column'),
    ];
    const alignment = menu('table-cell-alignment', 'Cell alignment', 190);
    const positions = alignment.append(new Column({ text: 'panel:menu-grid:columns=3:fixed:flush:gap=2' }));
    for (const vertical of ['top', 'middle', 'bottom']) {
        const row = positions.append(new Row({ text: 'panel:fixed:flush:gap=2' }));
        for (const [align, icon] of [['left', 151], ['center', 152], ['right', 153]]) {
            const label = `${vertical[0].toUpperCase()}${vertical.slice(1)} ${align}`;
            const choice = row.append(new MenuItem({ name: `table-align-${vertical}-${align}`, text: 'panel:fixed:accessory-height=36:width=36:flush', icon, tooltip: label, accessibleName: label }));
            choice.on(Message.ACTIVATE, () => { if (cellEnabled) run(() => cellStyle({ text: { align }, textLayout: { vertical } })); });
        }
    }
    const margins = command('table-cell-margins', 'Cell margins', 189, () => {
        if (!cellEnabled || dialog) return;
        const fields = new Map(), error = new Label({ name: 'table-margins-error', text: '' });
        const rows = SIDES.map(side => {
            const resolved = selectedCell.cell.text?.layout?.[side];
            const value = Number.isFinite(resolved) ? String(Math.round(resolved * 72 / 96 * 100) / 100) : '';
            const field = new TextField({ name: `table-margin-${side}`, value, placeholder: 'Inherited', accessibleName: `${side} margin in points` });
            fields.set(side, { field, initial: value });
            return new Row({ text: 'panel:form-control' }, [new Label({ text: `${side[0].toUpperCase()}${side.slice(1)} (pt)` }), field]);
        });
        const apply = new Button({ name: 'table-margins-apply', text: 'Apply', accent: true });
        const cancel = new Button({ name: 'table-margins-cancel', text: 'Cancel' }); cancel.on(Message.ACTIVATE, close);
        apply.on(Message.ACTIVATE, () => {
            if (!cellEnabled) { close(); return; }
            const textLayout = {};
            for (const [side, { field, initial }] of fields) {
                const value = field.getProperty('value').trim(); if (value === initial) continue;
                if (!/^(?:\d+(?:\.\d*)?|\.\d+)$/u.test(value) || Number(value) > 750000) { error.text = 'Enter a non-negative margin in points.'; return; }
                textLayout[side] = Number(value) * 96 / 72;
            }
            close(); if (Object.keys(textLayout).length) run(() => cellStyle({ textLayout }));
        });
        dialog = new Dialog({ name: 'table-margins-dialog', text: 'Cell margins', modal: true }, [...rows, error, new Row({ text: 'panel:actions' }, [cancel, apply])]);
        dialog.on(Message.KEY, event => { if (event.detail?.pressed && event.detail.keyCode === 9) close(); }); root.append(dialog);
    });
    function updateControls() {
        const preset = activePreset();
        const tableEditable = Boolean(table?.table.canEditCells && typeof canvas.tableStyle === 'function');
        for (const { control, requires } of controls) control.setProperty('enabled', usable && (requires === 'table' ? tableEditable : requires === 'preset' ? Boolean(tableEditable && preset)
            : requires === 'structure' ? structureEnabled : requires === 'delete-row' ? structureEnabled && table.table.rows > 1
                : requires === 'delete-column' ? structureEnabled && table.table.columns > 1 : cellEnabled));
        header.setProperty('checked', table?.table.headerRow === true); banded.setProperty('checked', table?.table.bandedRows === true);
        for (const flag of [header, banded]) flag.setProperty('tooltip', preset ? flag.text : 'Choose a table style first');
        const flags = { headerRow: table?.table.headerRow === true, bandedRows: table?.table.bandedRows === true };
        const changed = previewFlags !== JSON.stringify(flags); previewFlags = JSON.stringify(flags);
        previews.forEach(({ choice, id, preview }) => {
            choice.setProperty('selected', id === preset);
            if (changed) preview.submit(tableStyleThumbnail(id, flags));
        });
        const fill = selectedCell?.cell?.fill?.color ?? null;
        palettes[0].control.setValue(fill, { selected: Boolean(selectedCell) });
        const currentBorders = SIDES.map(side => selectedCell?.cell?.borders?.[side]?.color ?? null);
        const outline = currentBorders.find(Boolean) ?? borderColor;
        palettes[1].control.setValue(outline, { mixed: currentBorders.some(value => value && value !== outline), selected: Boolean(selectedCell) });
    }
    return Object.freeze({ tabs: [
        { id: 'table', title: 'Table', icon: 41, visible: false, groups: [
            { id: 'table-style', title: 'Table styles', controls: [styles], primary: 1 },
            { id: 'table-options', title: 'Style options', controls: [header, banded] },
            { id: 'table-borders', title: 'Borders and shading', controls: [shading, borders, borderPalette, weight] },
            { id: 'table-structure', title: 'Rows and columns', controls: structureControls },
            { id: 'table-cell-layout', title: 'Alignment', controls: [alignment, margins] },
        ] },
    ], close,
        setState({ selected = [], usable: ready }) {
            usable = Boolean(ready); table = selected.length === 1 && selected[0].table ? selected[0] : null;
            const candidate = canvas.selectedTableCell;
            selectedCell = candidate && table && candidate.object?.id === table.id ? candidate : null;
            const identity = selectedCell ? `${table.id}:${selectedCell.row}:${selectedCell.column}` : '';
            if (identity !== cellIdentity) {
                close(); cellIdentity = identity;
                const current = SIDES.map(side => selectedCell?.cell?.borders?.[side]).find(Boolean);
                borderColor = current?.color || '#000000'; borderWidth = current?.width || 1;
            }
            cellEnabled = Boolean(usable && selectedCell?.cell?.editable && !selectedCell.cell.covered && typeof canvas.tableCellStyle === 'function');
            structureEnabled = Boolean(usable && selectedCell && table.table.canEditStructure && typeof canvas.tableStructure === 'function');
            updateControls(); return Boolean(table);
        },
    });
}
