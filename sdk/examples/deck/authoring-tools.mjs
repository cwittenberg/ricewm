import { canReorderObjects, objectOrderingActions } from './object-ordering.mjs';
import { Button, ColorMenu, Menu, MenuItem, Message, Ribbon, State } from '@ricewm/app';
import { selectedTextStyle } from './text-selection-style.mjs';
import { createAuthoringDialogs } from './authoring-dialogs.mjs';
import { createShapeGallery } from './shape-gallery.mjs';
import { createTextLayoutTools } from './text-layout-tools.mjs';
import { createParagraphSpacingTools } from './paragraph-spacing-tools.mjs';
import { createRotationTools } from './rotation-tools.mjs';
import { createTableTools } from './table-tools.mjs';
import { createTableFormatTools } from './table-format-tools.mjs';
import { createShadowTools } from './shadow-tools.mjs';

/** Native RiceUI supplies the chrome, interaction and theme. Document commands
 * remain independent of the reusable ribbon and palette components. */
export function createAuthoringTools({ root, canvas, notice, picture, shadowsSupported = false, clipboardActions = {}, insertSlideControl = null, slideActions = [], designControls = [], transitionControls = [], animationControls = [], layoutControls = [], showControls = [], hyperlinkControls = [] }) {
    let active = false, locked = false, selection = [], clipboardReadyOverride;
    let currentStyle = selectedTextStyle([]);
    let alignmentReference = 'auto';
    const controls = [], palettes = [];
    const dialogs = createAuthoringDialogs({ root, canvas, notice, selected: () => selection });
    const textLayout = createTextLayoutTools({ root, canvas, notice });
    const paragraphSpacing = createParagraphSpacingTools({ root, canvas, notice });
    const rotation = createRotationTools({ root, canvas, notice });
    const tables = createTableTools({ root, insert: size => canvas.insertTable(size), notice });
    const tableFormat = createTableFormatTools({ root, canvas, notice });
    const shadows = shadowsSupported ? ['home-shadow', 'shape-shadow', 'picture-shadow'].map(name =>
        createShadowTools({ root, canvas, name, notice })) : [];
    function textSelection() {
        const cell = canvas.selectedTableCell;
        return selection.length === 1 && selection[0].table && cell?.object?.id === selection[0].id
            ? cell.cell?.editable && cell.cell.text ? [{ ...selection[0], text: { ...cell.cell.text, editable: true } }] : []
            : selection;
    }
    function tracked(control, requires = 'document') { controls.push({ control, requires }); return control; }
    function button(name, label, icon, callback, requires = 'document') {
        const control = tracked(new Button({ name, text: '', icon, state: State.DISPLAY,
            accessibleName: label, tooltip: label, enabled: false }), requires);
        control.on(Message.ACTIVATE, () => { if (control.getProperty('enabled')) void callback(); });
        return control;
    }
    function menu(name, text, entries, requires = 'document', icon = null) {
        const control = tracked(new Menu({ name, text, icon, tooltip: text, accessibleName: text, enabled: false },
            entries.map(([label, callback, itemIcon]) => {
                const item = new MenuItem({ text: label, icon: itemIcon ?? null });
                item.on(Message.ACTIVATE, () => { if (control.getProperty('enabled')) void callback(); });
                return item;
            })), requires);
        return control;
    }
    function palette(name, label, icon, callback, requires, value, allowNone = true) {
        const control = tracked(new ColorMenu({ name, label, icon, allowNone,
            noneLabel: name.endsWith('shape-outline') ? 'No outline' : 'No fill', enabled: false }), requires);
        const role = name.replace(/^home-/u, '');
        control.on(Message.VALUE_CHANGED, event => {
            if (!control.getProperty('enabled')) return;
            for (const peer of palettes.filter(peer => peer.role === role)) {
                peer.control.lastColor = event.detail.value;
                peer.control.setValue(event.detail.value, { selected: true });
            }
            void callback(event.detail.value);
        });
        palettes.push({ control, value, name, role }); return control;
    }
    const gallery = createShapeGallery({ root, onSelect: kind => canvas.insert(kind), notice });
    tracked(gallery.control);
    const homeGallery = createShapeGallery({ root, onSelect: kind => canvas.insert(kind), notice });
    function homeNames(control) {
        if (control.name && !control.name.startsWith('home-')) control.setProperty('name', `home-${control.name}`);
        control.children.forEach(homeNames);
    }
    homeNames(homeGallery.control); tracked(homeGallery.control);
    const bold = button('format-bold', 'Bold', 126, () => canvas.style({ bold: currentStyle.bold !== true }), 'text');
    const italic = button('format-italic', 'Italic', 127, () => canvas.style({ italic: currentStyle.italic !== true }), 'text');
    const underline = button('format-underline', 'Underline', 128, () => canvas.style({ underline: currentStyle.underline !== true }), 'text');
    const painter = button('format-painter', 'Format Painter — double-click to keep painting', 514, () => canvas.paintFormat(), 'paint-format');
    const family = menu('font-family', 'Font', ['Arial', 'Liberation Sans', 'Manrope', 'Noto Sans Arabic', 'Noto Sans JP']
        .map(fontFamily => [fontFamily, () => canvas.style({ fontFamily })]), 'text');
    const size = menu('font-size', 'Size', [10, 12, 14, 16, 18, 20, 24, 28, 32, 40, 48, 60, 72]
        .map(points => [`${points} pt`, () => canvas.style({ fontSize: points * 96 / 72 })]), 'text');
    // A native toolbar treats unaligned menus as icon-only commands. Font
    // selectors display document values and therefore use textual dropdowns.
    for (const [control, label] of [[family, 'Font family'], [size, 'Font size']]) {
        control.setProperty('state', State.START_ALIGNED | State.TRAILING_ICON);
        control.setProperty('icon', 74);
        control.setProperty('tooltip', label);
        control.setProperty('accessibleName', label);
    }
    const changeFontSize = grow => {
        const points = currentStyle.fontSize * 72 / 96;
        const steps = [1, 8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 32, 36, 40, 44, 48, 54, 60, 66, 72, 80, 88, 96, 120, 144, 192, 288, 384];
        const next = grow ? steps.find(value => value > points) ?? 384 : [...steps].reverse().find(value => value < points) ?? 0.75;
        return canvas.style({ fontSize: next * 96 / 72 });
    };
    const growFont = button('grow-font', 'Increase font size', 158, () => changeFontSize(true), 'font-size');
    const shrinkFont = button('shrink-font', 'Decrease font size', 157, () => changeFontSize(false), 'font-size');
    const color = palette('font-color', 'Text color', 188, color => canvas.style({ color }), 'text', () => currentStyle.color || '#202124', false);
    const fill = palette('shape-fill', 'Shape fill', 186, value => canvas.fill(value), 'fill', () => selection[0]?.fill?.color ?? null);
    const outline = palette('shape-outline', 'Shape outline', 187, value => canvas.strokeColor(value), 'style', () => selection[0]?.stroke?.color ?? null);
    const width = menu('outline-width', 'Width', [0.5, 1, 1.5, 2, 3, 4.5, 6].map(points => [`${points} pt`, () => canvas.strokeWidth(points * 96 / 72)]), 'style', 183);
    const align = menu('align-objects', 'Align', [['Left', 'left', 230], ['Center', 'center', 231], ['Right', 'right', 232], ['Top', 'top', 233], ['Middle', 'middle', 234], ['Bottom', 'bottom', 235]]
        .map(([label, direction, icon]) => [`Align ${label.toLowerCase()}`, () => canvas.align(direction, alignmentReference), icon]), 'transform', 190);
    const alignmentScopes = ['slide', 'selection'].map(scope => {
        const choice = new MenuItem({ name: `align-to-${scope}`, text: scope === 'slide' ? 'Align to slide' : 'Align selected objects' });
        choice.on(Message.ACTIVATE, () => { if (choice.getProperty('enabled')) { alignmentReference = scope; update(); } });
        align.append(choice); return choice;
    });
    const homeFill = palette('home-shape-fill', 'Shape fill', 186, value => canvas.fill(value), 'fill', () => selection[0]?.fill?.color ?? null);
    const homeOutline = palette('home-shape-outline', 'Shape outline', 187, value => canvas.strokeColor(value), 'style', () => selection[0]?.stroke?.color ?? null);
    const arrange = tracked(new Menu({ name: 'home-arrange', text: '', icon: 189, state: State.DISPLAY,
        accessibleName: 'Arrange', tooltip: 'Arrange', enabled: false }), 'selection');
    function arrangeItem(parent, name, label, icon, callback, requires) {
        const item = tracked(new MenuItem({ name, text: label, icon, enabled: false }), requires);
        item.on(Message.ACTIVATE, () => { if (item.getProperty('enabled')) void callback(); });
        parent.append(item); return item;
    }
    function arrangeSubmenu(name, label, icon, requires) {
        const child = new Menu({ name: `${name}-menu` });
        arrange.append(tracked(new MenuItem({ name, text: label, icon, enabled: false }, [child]), requires));
        return child;
    }
    function orderingItems(parent, prefix) {
        for (const { direction, label, icon } of objectOrderingActions) {
            arrangeItem(parent, `${prefix}-order-${direction}`, label, icon, () => canvas.reorder(direction), 'reorder');
        }
    }
    orderingItems(arrange, 'home');
    const formatArrange = tracked(new Menu({ name: 'format-arrange', text: '', icon: 189, state: State.DISPLAY,
        accessibleName: 'Arrange', tooltip: 'Arrange', enabled: false }), 'reorder');
    orderingItems(formatArrange, 'format');
    const homeAlign = arrangeSubmenu('home-align', 'Align', 190, 'transform');
    for (const [label, direction, icon] of [['Left', 'left', 230], ['Center', 'center', 231], ['Right', 'right', 232], ['Top', 'top', 233], ['Middle', 'middle', 234], ['Bottom', 'bottom', 235]]) {
        arrangeItem(homeAlign, `home-align-${direction}`, `Align ${label.toLowerCase()}`, icon, () => canvas.align(direction, alignmentReference), 'transform');
    }
    for (const scope of ['slide', 'selection']) {
        const item = new MenuItem({ name: `home-align-to-${scope}`, text: scope === 'slide' ? 'Align to slide' : 'Align selected objects' });
        item.on(Message.ACTIVATE, () => { if (item.getProperty('enabled')) { alignmentReference = scope; update(); } });
        homeAlign.append(item); alignmentScopes.push(item);
    }
    for (const [axis, icon] of [['horizontal', 191], ['vertical', 192]]) arrangeItem(homeAlign, `home-distribute-${axis}`, `Distribute ${axis === 'horizontal' ? 'horizontally' : 'vertically'}`, icon, () => canvas.align(`distribute-${axis}`, alignmentReference), 'distribute');
    arrangeItem(arrange, 'home-object-position', 'Position and size…', 189, dialogs.position, 'single-transform');
    const homeRotate = arrangeSubmenu('home-rotate', 'Rotate', 195, 'rotate');
    for (const [name, label, degrees, relative] of [['right', 'Rotate right 90°', 90, true], ['left', 'Rotate left 90°', -90, true], ['reset', 'Reset rotation', 0, false]]) arrangeItem(homeRotate, `home-rotate-${name}`, label, 195, () => canvas.rotate(degrees, relative), 'rotate');
    arrangeItem(arrange, 'home-group', 'Group', 239, () => canvas.group(), 'group');
    arrangeItem(arrange, 'home-ungroup', 'Ungroup', 240, () => canvas.ungroup(), 'ungroup');
    const clipboard = [['paste', 'Paste', 24], ['cut', 'Cut', 22], ['copy', 'Copy', 23]]
        .filter(([kind]) => typeof clipboardActions[kind] === 'function')
        .map(([kind, label, icon]) => button(`clipboard-${kind}`, label, icon, () => clipboardActions[kind](), `clipboard-${kind}`));
    const group = (id, title, controls, primary = 0, rowBreak = undefined) => ({ id, title, controls, primary, rowBreak });
    slideActions.forEach((control, index) => {
        const label = control.text; control.text = ''; control.setProperty('icon', [10, 23, 25, 31, 32][index]);
        control.setProperty('state', State.DISPLAY); control.setProperty('tooltip', label); control.setProperty('accessibleName', label);
    });
    if (layoutControls[0]) layoutControls[0].text = 'New Slide';
    const ribbon = new Ribbon({ name: 'authoring-ribbon', presentation: 'office', controlHeight: 32, activeTab: 'home', tabs: [
        { id: 'home', title: 'Home', icon: 5, groups: [
            group('clipboard', 'Clipboard', [...clipboard, painter], clipboard[0]?.name === 'clipboard-paste' ? 1 : 0),
            group('layouts', 'Slides', layoutControls, 0, Math.min(1, layoutControls.length)),
            group('font', 'Font', [family, size, growFont, shrinkFont, bold, italic, underline, color], 0, 2),
            group('paragraph', 'Paragraph', [...[['left', 151], ['center', 152], ['right', 153]].map(([align, icon]) =>
                button(`paragraph-${align}`, `Align text ${align}`, icon, () => canvas.style({ align }), 'text')), paragraphSpacing.control, ...textLayout.controls]),
            group('drawing', 'Drawing', [homeGallery.control, arrange, homeFill, homeOutline,
                ...(shadows[0] ? [shadows[0].control] : []),
                button('delete-object', 'Delete selected objects', 25, () => canvas.remove(), 'delete')]),
        ] },
        { id: 'insert', title: 'Insert', icon: 76, groups: [
            ...(insertSlideControl ? [group('slides', 'Slides', [insertSlideControl], 1)] : []),
            group('tables', 'Tables', [tables.control]),
            group('images', 'Images', [button('insert-picture', 'Insert picture', 71, () => picture?.(), 'picture-document')]),
            group('illustrations', 'Illustrations', [gallery.control,
                button('insert-line', 'Draw line', 183, () => canvas.insert('line')),
                button('insert-polygon', 'Draw polygon', 518, () => canvas.insert('polygon')),
                button('insert-polyline', 'Draw polyline', 519, () => canvas.insert('polyline'))]),
            ...(hyperlinkControls.length ? [group('links', 'Links', hyperlinkControls)] : []),
            group('text', 'Text', [button('insert-text', 'Draw text box', 194, () => canvas.insert('text'))]),
        ] },
        { id: 'presentation', title: 'Presentation', icon: 104, groups: [
            group('slides', 'Slides', slideActions),
            group('page', 'Page', designControls),
            ...(transitionControls.length ? [group('transitions', 'Transitions', transitionControls)] : []),
            ...(animationControls.length ? [group('animations', 'Animations', animationControls)] : []),
            ...(showControls.length ? [group('present', 'Present', showControls, 0, 1)] : []),
        ] },
        { id: 'format', title: 'Format', icon: 182, visible: false, groups: [
            group('appearance', 'Shape style', [fill, outline, width, ...(shadows[1] ? [shadows[1].control] : [])]),
            group('arrange', 'Arrange', [
                formatArrange, align,
                button('distribute-horizontal', 'Distribute horizontally', 191, () => canvas.align('distribute-horizontal', alignmentReference), 'distribute'),
                button('distribute-vertical', 'Distribute vertically', 192, () => canvas.align('distribute-vertical', alignmentReference), 'distribute'),
                button('object-position', 'Position and size', 189, dialogs.position, 'single-transform'),
                ...rotation.controls,
                button('group-objects', 'Group selected objects', 239, () => canvas.group(), 'group'),
                button('ungroup-object', 'Ungroup', 240, () => canvas.ungroup(), 'ungroup'),
            ]),
        ] },
        { id: 'picture', title: 'Picture', icon: 71, visible: false, groups: [
            group('picture', 'Picture', [
                button('insert-placeholder-picture', 'Insert picture into placeholder', 71, () => picture?.(), 'picture-placeholder'),
                button('replace-picture', 'Replace picture', 71, () => picture?.(selection[0].id), 'replace-picture'),
                button('crop-picture', 'Crop picture', 193, dialogs.crop, 'crop-picture'),
                button('reset-picture-crop', 'Reset crop', 21, () => canvas.crop({ left: 0, top: 0, right: 0, bottom: 0 }), 'crop-picture'),
                ...(shadows[2] ? [shadows[2].control] : []),
            ]),
        ] },
        ...tableFormat.tabs,
    ] });
    function update() {
        const transform = selection.length > 0 && selection.every(object => object.edit?.canTransform);
        const style = selection.length > 0 && selection.every(object => object.edit?.canStyle);
        const usable = active && !locked && !canvas.busy;
        const textTargets = textSelection();
        currentStyle = selectedTextStyle(textTargets);
        tables.setEnabled(usable && typeof canvas.insertTable === 'function');
        const tableContext = tableFormat.setState({ selected: selection, usable });
        // Keep context stable while a document operation temporarily clears selection.
        if (!locked && !canvas.busy) {
            const pictureContext = selection.length > 0 && selection.every(object => object.image != null || object.kind === 'image' || object.placeholder?.role === 'picture');
            ribbon.setTabState('table', { visible: tableContext || ribbon.activeTab === 'table' });
            ribbon.setTabState('picture', { visible: !tableContext && pictureContext || ribbon.activeTab === 'picture' });
            ribbon.setTabState('format', { visible: !tableContext && !pictureContext && selection.length > 0 || ribbon.activeTab === 'format' });
        }
        textLayout.setState({ selected: selection, usable });
        paragraphSpacing.setState({ selected: selection, usable });
        rotation.setState({ selected: selection, usable });
        shadows.forEach(tools => tools.setState({ selected: selection, usable }));
        const toSlide = alignmentReference === 'slide' || alignmentReference === 'auto' && selection.length === 1;
        alignmentScopes.forEach((item, index) => {
            item.setProperty('checked', index % 2 === 0 ? toSlide : !toSlide);
            item.setProperty('enabled', usable && transform && (index % 2 === 0 || selection.length > 1));
        });
        for (const { control, requires } of controls) {
            const enabled = (requires.startsWith('clipboard-') && (typeof clipboardActions.can === 'function'
                ? clipboardActions.can(requires.slice(10), selection)
                : requires === 'clipboard-paste' || selection.length > 0 && (requires !== 'clipboard-cut' || selection.every(object => object.edit?.canDelete)))) ||
                (requires === 'selection' && selection.length > 0) ||
                (requires === 'reorder' && typeof canvas.reorder === 'function' && canReorderObjects(selection)) ||
                (requires === 'rotate' && selection.length > 0 && selection.every(object => object.edit?.canRotate)) ||
                (requires === 'font-size' && Number.isFinite(currentStyle.fontSize) && !currentStyle.mixed.includes('fontSize')) ||
                requires === 'document' || (requires === 'picture-document' && typeof picture === 'function') ||
                (requires === 'paint-format' && (canvas.formatPainter?.active || selection.length === 1 && selection[0].edit?.canStyle && !selection[0].group && !selection[0].image)) ||
                (requires === 'replace-picture' && typeof picture === 'function' && selection.length === 1 && selection[0].image?.canReplace) ||
                (requires === 'picture-placeholder' && typeof picture === 'function' && selection.length === 1 && selection[0].placeholder?.role === 'picture' && !selection[0].text?.text && selection[0].edit?.canDelete) ||
                (requires === 'crop-picture' && selection.length === 1 && selection[0].image?.canCrop) ||
                (requires === 'text' && textTargets.some(object => object.text != null)) || (requires === 'style' && style) ||
                (requires === 'fill' && style && selection.every(object => object.geometry !== 'line')) ||
                (requires === 'delete' && selection.length > 0 && selection.every(object => object.edit?.canDelete)) ||
                (requires === 'group' && selection.length > 1 && selection.every(object => object.edit?.canGroup && !object.parentId)) ||
                (requires === 'ungroup' && selection.length === 1 && selection[0].group?.canUngroup) ||
                (requires === 'transform' && transform) || (requires === 'single-transform' && transform && selection.length === 1 && selection[0].geometry !== 'line') ||
                (requires === 'distribute' && transform && selection.length >= (toSlide ? 2 : 3));
            const ready = requires.startsWith('clipboard-') ? clipboardReadyOverride ?? usable : usable;
            control.setProperty('enabled', ready && enabled);
        }
        growFont.setProperty('enabled', growFont.getProperty('enabled') && currentStyle.fontSize < 512);
        shrinkFont.setProperty('enabled', shrinkFont.getProperty('enabled') && currentStyle.fontSize > 1);
        painter.setProperty('checked', Boolean(canvas.formatPainter?.active));
        painter.setProperty('tooltip', canvas.formatPainter?.persistent ? 'Format Painter: multiple targets. Escape cancels.' : 'Format Painter — double-click to keep painting');
        bold.setProperty('checked', currentStyle.bold === true); italic.setProperty('checked', currentStyle.italic === true); underline.setProperty('checked', currentStyle.underline === true);
        family.text = currentStyle.mixed.includes('fontFamily') ? 'Mixed fonts' : currentStyle.fontFamily || 'Font';
        size.text = currentStyle.mixed.includes('fontSize') ? 'Mixed sizes' : currentStyle.fontSize ? `${Math.round(currentStyle.fontSize * 72 / 96)} pt` : 'Size';
        for (const { control, value, role: name } of palettes) {
            const key = name === 'shape-fill' ? 'fill' : 'stroke';
            const mixed = name === 'font-color' ? currentStyle.mixed.includes('color') || currentStyle.color === null
                : selection.some(object => (object[key]?.color ?? null) !== (selection[0]?.[key]?.color ?? null));
            control.setValue(value(), { mixed, selected: selection.length > 0 });
        }
        gallery.refresh?.(); homeGallery.refresh?.(); homeNames(homeGallery.control);
    }
    return Object.freeze({ control: ribbon, rotationProperties: rotation.properties,
        position: dialogs.position, crop: dialogs.crop, insertPicture: () => picture?.(),
        selectTab: tab => ribbon.selectTab(({ slides: 'presentation', design: 'presentation', show: 'presentation', 'table-design': 'table', 'table-layout': 'table' })[tab] || tab),
        toggleTextStyle(property) {
            if (active && !locked && !canvas.busy && ['bold', 'italic', 'underline'].includes(property) && textSelection().some(object => object.text != null)) {
                return canvas.style({ [property]: currentStyle[property] !== true });
            }
        },
        setState({ ready = active, busy = locked, selected = selection, clipboardReady } = {}) {
            active = ready; locked = busy; selection = selected; clipboardReadyOverride = clipboardReady; update();
        } });
}
