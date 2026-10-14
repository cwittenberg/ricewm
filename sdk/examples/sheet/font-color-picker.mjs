import { ColorMenu, Container, Message, State } from '@ricewm/app';

/** Native anchored palette. An explicit choice is an edit even when the
 * effective color is unchanged: it can override an inherited named style. */
export function createFontColorPicker({ onChoose }) {
    const control = new ColorMenu({ name: 'toolbar-font-color', label: 'Text Color',
        icon: 188, value: null, allowNone: true, noneLabel: 'Automatic', enabled: false });
    control.lastAction.setProperty('visible', false);
    control.remove(control.scroll);
    control.append(new Container({text:'panel:fixed:flush:width=280',state:State.FLAT},[control.scroll]));
    control.palette.setProperty('text','panel:flush:fixed:tight-stack:intrinsic-flow');
    const compact = () => control.scroll.setProperty('text', '0/0');
    compact();
    control.palette.moreButton.on(Message.ACTIVATE, () => control.scroll.setProperty('text',
        'viewport:360|' + control.scroll.text.replace(/^viewport:\d+\|/u, '')));
    control.palette.cancelButton.on(Message.ACTIVATE, compact);
    const remember = control.palette.onColorCommit;
    control.palette.onColorCommit = value => {
        remember(value);
        compact();
        onChoose(value);
    };
    control.refreshSelection = projection => control.setValue(
        projection?.selection.format?.fontColor ?? null,
        { selected: Boolean(projection) });
    return control;
}
