import {deckSelectionColors} from './selection-chrome.mjs';
import {ribbonCommandArtwork} from '@ricewm/app';
import { Button, CheckBox, ColorMenu, Column, Container, Dialog, Gallery, Label, Menu, MenuItem, Message, Row, SceneBuilder, SceneSurface, ScrollView, State, TextField, distributeTrackSizes,presentationTableStyles,presentationTableStylePreview } from '@ricewm/app';

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
export function createTableFormatTools({ root, canvas, notice = () => {},restoreTextFocus=()=>{} }) {
    let table = null, selectedCell = null, usable = false, cellEnabled = false, structureEnabled = false;
    let borderColor = '#000000', borderWidth = 1, dialog = null, cellIdentity = '', previewFlags = '';
    let sourceDesign=false,theme=null,themeKey='',pendingOptions=null;
    const knownPresets = new Map(), controls = [];
    const close = () => { if (dialog) root.remove(dialog, { destroy: true }); dialog = null; };
    const run = callback => {
        try { return Promise.resolve(callback()).catch(error => {notice(error.message || 'Could not edit table');return false;}); }
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
    const resolveScheme=slot=>{const entry=theme?.colors?.find(color=>color.slot===slot);return entry?{color:entry.color,opacity:entry.opacity}:null;};
    const hasPalette=()=>['dk1','accent1','accent2','accent3','accent4','accent5','accent6'].every(slot=>resolveScheme(slot));
    const sourceEditable=()=>Boolean(usable&&table?.table.canSetDesign);
    const applyDesign=preset=>{
        if(!sourceEditable()||!hasPalette())return false;
        return Promise.resolve(canvas.tableDesign({preset,...table.table.designOptions})).then(result=>{if(result!==false)restoreTextFocus();return result;});
    };
    const applyOptions=options=>{
        if(!sourceEditable())return false;
        const pending={id:table.id,options};pendingOptions=pending;updateControls();
        return Promise.resolve(canvas.tableOptions(options)).then(result=>{if(result!==false)restoreTextFocus();return result;}).finally(()=>{if(pendingOptions===pending)pendingOptions=null;updateControls();});
    };
    function designGallery(name,inlineOnly){
        const control=track(new Gallery({name,text:`panel:fixed-height:${inlineOnly?40:252}:fixed:width=${inlineOnly?391:360}:gap=0:padding=0:flush`,presentation:'tiles',itemRole:inlineOnly?'button':'menuitem',pageSize:inlineOnly?8:37,
            tileWidth:48,tileHeight:inlineOnly?40:36,gap:inlineOnly?1:2,chrome:false,previewPadding:2,showSearch:false,showTitles:false,enabled:false,
            provider:async()=>({items:hasPalette()?presentationTableStyles({inlineOnly}).map(entry=>({...entry,title:entry.label})):[],next:null}),
            renderPreview:entry=>{const preview=new SceneSurface({name:name+'-preview-'+entry.id,enabled:false});preview.submit(presentationTableStylePreview(entry.id,{width:44,height:inlineOnly?36:32,resolveScheme}));return preview;},
            onSelect:entry=>run(()=>applyDesign(entry.id))}),'design');
        control.status.visible=false;return control;
    }
    const inlineStyles=designGallery('table-inline-styles',true),moreStyles=designGallery('table-more-styles-gallery',false);
    const more=track(moreStyles.menu({name:'table-more-styles',text:'panel:fixed:width=14:fixed-height:40:flush',icon:74,state:State.DISPLAY,tooltip:'More table styles',accessibleName:'More table styles',enabled:false}),'design');
    inlineStyles.visible=more.visible=false;
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
    const flags=[['headerRow','header-row','Header Row'],['firstColumn','first-column','First Column'],['totalRow','total-row','Total Row'],['lastColumn','last-column','Last Column'],['bandedRows','banded-rows','Banded Rows'],['bandedColumns','banded-columns','Banded Columns']].map(([key,id,label])=>{
        const control=track(new CheckBox({name:'table-'+id,text:label,enabled:false}),'option');
        control.on(Message.ACTIVATE,()=>{if(control.enabled)run(()=>sourceDesign?applyOptions({[key]:control.getProperty('checked')}):applyPreset(activePreset(),{[key]:control.getProperty('checked')}));});return {key,control};
    });
    const header=flags[0].control,banded=flags[4].control;
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
    for (const points of [0.25, 0.5, 0.75, 1, 1.5, 2.25, 3, 4.5, 6]) item(weight, `table-border-weight-${String(points).replace('.', '-')}`, `${points} pt`, 183, () => {
        borderWidth = points * 96 / 72;
        const sides = existingBorders();
        if (sides.length) return cellStyle({ borders: Object.fromEntries(sides.map(side => [side, { color: selectedCell.cell.borders[side].color, width: borderWidth }])) });
    });
    const borders = menu('table-cell-borders', 'Cell borders', 41);
    item(borders, 'table-borders-all', 'All borders', 41, () => cellStyle({ borders: border(SIDES) }));
    item(borders, 'table-borders-none', 'No borders', 25, () => cellStyle({ borders: Object.fromEntries(SIDES.map(side => [side, null])) }));
    for (const [side, icon] of [['top', 233], ['bottom', 235], ['left', 230], ['right', 232]]) item(borders, `table-border-${side}`, `${side[0].toUpperCase()}${side.slice(1)} border`, icon, () => cellStyle({ borders: border([side]) }));
    const structure = (type, axis, offset = 0) => { if (structureEnabled) return canvas.tableStructure(type, selectedCell[axis] + offset, 1); };
    const deletion = menu('table-delete', 'Delete', 25, 'structure');
    deletion.text = 'Delete';
    const deleteColumn = item(deletion, 'table-delete-column', 'Delete Columns', 181, () => structure('delete-table-columns', 'column'));
    const deleteRow = item(deletion, 'table-delete-row', 'Delete Rows', 180, () => structure('delete-table-rows', 'row'));
    const deleteTable = item(deletion, 'table-delete-table', 'Delete Table', 25, () => canvas.remove());
    const structureControls = [deletion,
        command('table-insert-row-above', 'Insert Above', 178, () => structure('insert-table-rows', 'row'), 'structure'),
        command('table-insert-row-below', 'Insert Below', 169, () => structure('insert-table-rows', 'row', 1), 'structure'),
        command('table-insert-column-left', 'Insert Left', 179, () => structure('insert-table-columns', 'column'), 'structure'),
        command('table-insert-column-right', 'Insert Right', 170, () => structure('insert-table-columns', 'column', 1), 'structure'),
    ];
    structureControls.slice(1).forEach((control, index) => { control.text = `Insert\n${['Above', 'Below', 'Left', 'Right'][index]}`; });
    const mergeArtwork=(split,theme)=>{
        const b=new SceneBuilder(32,32),stroke=deckSelectionColors(theme).selection;
        b.line([3,6],[29,6],1.5,stroke).line([29,6],[29,26],1.5,stroke)
            .line([29,26],[3,26],1.5,stroke).line([3,26],[3,6],1.5,stroke);
        if(split){b.line([16,6],[16,26],1.5,stroke).line([3,16],[29,16],1.5,stroke);}
        else{b.line([3,12],[8,12],1.5,stroke).line([24,12],[29,12],1.5,stroke).line([16,6],[16,10],1.5,stroke).line([16,22],[16,26],1.5,stroke);
            for(const sign of [-1,1]){const start=16+sign*9,end=16+sign*2;b.line([start,16],[end,16],1.5,stroke).line([end+sign*3,13],[end,16],1.5,stroke).line([end+sign*3,19],[end,16],1.5,stroke);}}
        return b.finish();
    };
    const merge=command('table-merge-cells','Merge Cells',41,()=>canvas.mergeTableCells(),'merge');
    const split=command('table-split-cells','Split Cells',41,()=>canvas.splitTableCell(),'split');
    merge.text='Merge\nCells';split.text='Split\nCells';
    ribbonCommandArtwork(merge,theme=>mergeArtwork(false,theme));ribbonCommandArtwork(split,theme=>mergeArtwork(true,theme));
    const tracksReady = () => Boolean(table?.table.canEditStructure && typeof canvas.tableTracks === 'function' &&
        Array.isArray(table.table.columnWidths) && Array.isArray(table.table.rowHeights));
    const distribute = axis => {
        if (!usable || !tracksReady()) return;
        return canvas.tableTracks(axis === 'columns' ? distributeTrackSizes(table.table.columnWidths) : table.table.columnWidths,
            axis === 'rows' ? distributeTrackSizes(table.table.rowHeights) : table.table.rowHeights);
    };
    const distribution = [['rows', 'Rows', 192], ['columns', 'Columns', 191]].map(([axis, label, icon]) => {
        const control = command(`table-distribute-${axis}`, `Distribute ${label}`, icon, () => distribute(axis), 'tracks');
        control.text = `Distribute ${label}`; control.setProperty('state', State.START_ALIGNED); return control;
    });
    const alignmentControls = [];
    for (const [align, icon] of [['left', 151], ['center', 152], ['right', 153]]) {
        const control = command(`table-align-${align}`, align === 'center' ? 'Center' : `Align ${align}`, icon, () => cellStyle({ text: { align } }));
        alignmentControls.push({ control, axis: 'align', value: align });
    }
    for (const [vertical, label, icon] of [['top', 'Top', 233], ['middle', 'Center', 234], ['bottom', 'Bottom', 235]]) {
        const control = command(`table-align-${vertical}`, `Align text to the ${label.toLowerCase()} of the cell`, icon,
            () => cellStyle({ textLayout: { vertical } }));
        control.text = label; control.setProperty('state', State.START_ALIGNED);
        alignmentControls.push({ control, axis: 'vertical', value: vertical });
    }
    const customMargins = command('table-cell-margins-custom', 'Custom cell margins…', 189, () => {
        if (!cellEnabled || dialog) return;
        const fields = new Map(), error = new Label({ name: 'table-margins-error', text: '' });
        const rows = SIDES.map(side => {
            const values=(selectedCell.cells||[selectedCell.cell]).map(c=>c.text?.layout?.[side]),resolved=values.every(v=>v===values[0])?values[0]:null;
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
    const margins = menu('table-cell-margins', 'Cell Margins', 189);
    margins.text = 'Cell Margins'; margins.setProperty('state', State.START_ALIGNED);
    // The reference defines these values in points. Document commands use 96 dpi units.
    for (const [preset, label, left, right, top, bottom] of [
        ['normal', 'Normal', 7.2, 7.2, 3.6, 3.6], ['none', 'None', 0, 0, 0, 0],
        ['narrow', 'Narrow', 3.6, 3.6, 3.6, 3.6], ['wide', 'Wide', 14.4, 14.4, 14.4, 14.4],
    ]) item(margins, `table-cell-margins-${preset}`, label, 189, () => cellStyle({ textLayout:
        Object.fromEntries(Object.entries({ left, right, top, bottom }).map(([side, points]) => [side, points * 96 / 72])) }));
    margins.append(customMargins); customMargins.text = 'Custom margins…'; customMargins.setProperty('state', State.START_ALIGNED);
    function updateControls() {
        const preset = activePreset();
        const tableEditable = Boolean(table?.table.canEditCells && typeof canvas.tableStyle === 'function');
        for (const { control, requires } of controls) control.setProperty('enabled', usable && (requires==='design'?sourceEditable()&&hasPalette():requires==='option'?(sourceDesign?sourceEditable():Boolean(tableEditable&&preset)):requires === 'table' ? tableEditable : requires === 'preset' ? Boolean(tableEditable && preset)
            : requires === 'merge' ? Boolean(selectedCell?.multiple&&table?.table.canMergeCells&&typeof canvas.mergeTableCells==='function') : requires === 'split' ? Boolean(selectedCell&&!selectedCell.multiple&&table?.table.canMergeCells&&(selectedCell.cell.rowSpan>1||selectedCell.cell.columnSpan>1)&&typeof canvas.splitTableCell==='function') : requires === 'structure' ? structureEnabled : requires === 'tracks' ? tracksReady() : cellEnabled));
        deleteRow.enabled = usable && structureEnabled && table.table.rows > 1;
        deleteColumn.enabled = usable && structureEnabled && table.table.columns > 1;
        deleteTable.enabled = Boolean(usable && table?.edit?.canDelete && typeof canvas.remove === 'function');
        // Deleting a whole table remains available without selecting an individual cell.
        deletion.enabled = Boolean(usable && (structureEnabled || deleteTable.enabled));
        for (const {control, axis, value} of alignmentControls) control.setProperty('checked', cellEnabled &&
            (selectedCell.cells||[selectedCell.cell]).every(c=>(axis === 'align' ? c.text?.style?.align : c.text?.layout?.vertical) === value));
        styles.visible=!sourceDesign;inlineStyles.visible=more.visible=sourceDesign;
        const options=sourceDesign?{...table?.table.designOptions,...(pendingOptions&&table&&pendingOptions.id===table.id?pendingOptions.options:{})}:{headerRow:table?.table.headerRow===true,bandedRows:table?.table.bandedRows===true};
        for(const{key,control}of flags){control.visible=sourceDesign||['headerRow','bandedRows'].includes(key);control.setProperty('checked',options[key]===true);control.setProperty('tooltip',sourceDesign?control.text:preset?control.text:'Choose a table style first');}
        inlineStyles.setSelection(table?.table.design?.preset??null);moreStyles.setSelection(table?.table.design?.preset??null);
        const legacyFlags = { headerRow: table?.table.headerRow === true, bandedRows: table?.table.bandedRows === true };
        const changed = previewFlags !== JSON.stringify(legacyFlags); previewFlags = JSON.stringify(legacyFlags);
        previews.forEach(({ choice, id, preview }) => {
            choice.setProperty('selected', id === preset);
            if (changed) preview.submit(tableStyleThumbnail(id, legacyFlags));
        });
        const fill = selectedCell?.cell?.fill?.color ?? null;
        palettes[0].control.setValue(fill, {mixed:Boolean(selectedCell&&(selectedCell.cells||[selectedCell.cell]).some(c=>(c.fill?.color??null)!==fill)),selected:Boolean(selectedCell)});
        const currentBorders = SIDES.map(side => selectedCell?.cell?.borders?.[side]?.color ?? null);
        const outline = currentBorders.find(Boolean) ?? borderColor;
        palettes[1].control.setValue(outline, { mixed: currentBorders.some(value => value && value !== outline), selected: Boolean(selectedCell) });
    }
    return Object.freeze({ tabs: [
        { id: 'table-design', title: 'Table Design', icon: 41, visible: false, groups: [
            { id: 'table-options', title: 'Table Style Options', controls: flags.map(row=>row.control),rowCount:3,rowBreak:2 },
            { id: 'table-style', title: 'Table Styles', controls: [inlineStyles,more,styles], primary: 0,rowCount:1,minWidth:432 },
            { id: 'table-borders', title: 'Draw Borders', controls: [shading, borders, borderPalette, weight] },
        ] },
        { id: 'table-layout', title: 'Layout', icon: 41, visible: false, groups: [
            { id: 'table-structure', title: 'Rows & Columns', controls: structureControls, primary: 5, minWidth: 360 },
            {id:'table-merge',title:'Merge',controls:[merge,split],primary:2},
            { id: 'table-distribute', title: 'Distribute', controls: distribution, rowBreak: 1 },
            { id: 'table-cell-layout', title: 'Alignment', controls: [...alignmentControls.map(item => item.control), margins], rowBreak: 3 },
        ] },
    ], close,
        setState({ selected = [], usable: ready,theme:currentTheme=theme }) {
            usable = Boolean(ready); table = selected.length === 1 && selected[0].table ? selected[0] : null;
            if(table)sourceDesign=Object.hasOwn(table.table,'designOptions');else if(usable)sourceDesign=false;
            theme=currentTheme;const paletteKey=JSON.stringify(theme?.colors??null);
            if(paletteKey!==themeKey){themeKey=paletteKey;for(const control of [inlineStyles,moreStyles])void control.search('').catch(error=>notice(error.message));}
            const candidate = canvas.selectedTableCell;
            selectedCell = candidate && table && candidate.object?.id === table.id ? candidate : null;
            const identity = selectedCell ? `${table.id}:${selectedCell.row}:${selectedCell.column}:${JSON.stringify(selectedCell.range)}` : '';
            if (identity !== cellIdentity) {
                close(); cellIdentity = identity;
                const current = SIDES.map(side => selectedCell?.cell?.borders?.[side]).find(Boolean);
                borderColor = current?.color || '#000000'; borderWidth = current?.width || 1;
            }
            cellEnabled = Boolean(usable && !selectedCell?.cells?.some(c=>!c.editable) && selectedCell?.cell?.editable && !selectedCell.cell.covered && typeof canvas.tableCellStyle === 'function');
            structureEnabled = Boolean(usable && selectedCell && table.table.canEditStructure && typeof canvas.tableStructure === 'function');
            updateControls(); return Boolean(table);
        },
    });
}
