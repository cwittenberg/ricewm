import { ContextMenu, MenuItem, Message } from '@ricewm/app';

/** Native owner-scoped commands keep the clicked slide identity across activation. */
export function slideContext(owner, { target, invoke, available, selection = () => target ? [target.slide] : [], hidden = () => false }) {
    const menu = new ContextMenu({ name: target ? `slide-context-${target.index}` : 'slides-empty-context' });
    const commands = [['insert-slide', 'New slide', 10], ...(target
        ? [['duplicate-slide', 'Duplicate slide', 23], ['delete-slide', 'Delete slide', 25], ['set-slides-hidden', 'Hide Slide', 104]] : [])];
    for (const [type, text, icon] of commands) {
        const item = new MenuItem({ name: `${menu.name}-${type}`, text, icon, enabled: available() });
        item.on(Message.ACTIVATE, () => { if (!available()) return;
            const slides = [...selection()];
            const scoped = target && (type === 'delete-slide' || type === 'set-slides-hidden')
                ? { ...target, slides, ...(type === 'set-slides-hidden' ? { hidden: !slides.every(hidden) } : {}) } : target;
            void invoke(type, scoped); });
        menu.append(item);
    }
    menu.attachTo(owner);
    return { update() {
        const slides = selection(), allHidden = slides.length > 0 && slides.every(hidden);
        for (const item of menu.children) {
            item.setProperty('enabled', available());
            if (item.name.endsWith('-delete-slide')) item.text = slides.length > 1 ? 'Delete selected slides' : 'Delete slide';
            if (item.name.endsWith('-set-slides-hidden')) {
                item.text = `${allHidden ? 'Unhide' : 'Hide'} ${slides.length > 1 ? 'Selected Slides' : 'Slide'}`;
                item.setProperty('tooltip', item.text);
            }
        }
    } };
}

/** Count only completed primary pointer clicks on the actual blank native target. */
export function emptySlideDoubleClick(target, invoke, available) {
    let down = null, previous = null;
    const reset = () => { down = null; previous = null; };
    target.on(Message.POINTER, ({ detail }) => {
        if (detail.kind === 'down' && detail.button === 1 && available()) {
            down = { x: detail.x, y: detail.y, time: performance.now() };
        } else if (detail.kind === 'move') {
            if (down && Math.hypot(detail.x - down.x, detail.y - down.y) > 6) reset();
        } else if (detail.kind !== 'up') reset();
    });
    target.on(Message.KEY, reset);
    target.on(Message.BLUR, reset);
    target.on(Message.ACTIVATE, () => {
        const click = down; down = null;
        if (!click || !available()) { previous = null; return; }
        if (previous && click.time - previous.time <= 450 && Math.hypot(click.x - previous.x, click.y - previous.y) <= 6) {
            previous = null; void invoke('insert-slide');
        } else previous = click;
    });
    return reset;
}
