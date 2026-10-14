import { Button, CheckBox, Column, Container, Dialog, Label, Menu, MenuItem, Message, Row, SceneBuilder, SceneSurface, ScrollView, State, TextField, Tooltip } from '@ricewm/app';

const effects = [['none', 'None'], ['fade', 'Fade'], ['fade-black', 'Fade Through Black'], ['push', 'Push'], ['wipe', 'Wipe'], ['morph', 'Morph']];
const directions = [['left', 'Left'], ['right', 'Right'], ['up', 'Up'], ['down', 'Down']];
const validTime = value => Number.isInteger(value) && value >= 0 && value <= 600000;
const validTransition = value => value && effects.some(([id]) => id === value.effect) && directions.some(([id]) => id === value.direction) &&
    validTime(value.durationMs) && typeof value.advanceOnClick === 'boolean' && (value.advanceAfterMs === null || validTime(value.advanceAfterMs));
const sourceValues = value => ({ effect: value.effect, direction: value.direction, durationMs: value.durationMs,
    advanceOnClick: value.advanceOnClick, advanceAfterMs: value.advanceAfterMs });

/** Explanatory document thumbnails, not substitutes for live playback. */
function thumbnail(effect) {
    const builder = new SceneBuilder(120, 68).rect([0, 0, 120, 68], 0xffffffff);
    const blue = 0xffc27a32, light = 0xffdedede;
    if (effect === 'fade-black') builder.rect([8, 10, 104, 48], 0xff202020).rect([26, 18, 72, 32], 0x99c27a32);
    else if (effect === 'morph') builder.rect([8, 24, 30, 20], light).rect([65, 10, 47, 48], blue).line([40, 34], [62, 34], 2, 0xff555555);
    else if (effect === 'fade') builder.rect([8, 10, 82, 42], light).rect([28, 20, 82, 42], 0x99c27a32);
    else if (effect === 'wipe') builder.rect([8, 10, 104, 48], light).rect([8, 10, 52, 48], blue).line([60, 10], [60, 58], 2, 0xff555555);
    else if (effect === 'push') builder.rect([4, 10, 50, 48], light).rect([64, 10, 50, 48], blue)
        .line([84, 34], [34, 34], 3, 0xff333333).line([34, 34], [44, 25], 3, 0xff333333).line([34, 34], [44, 43], 3, 0xff333333);
    else builder.rect([8, 10, 104, 48], light);
    return builder.finish();
}

/** Native controls for the entering effect and the current slide's outgoing
 * advance timing. The document adapter owns source edits, history and playback. */
export function createTransitionTools({ root, apply, preview, notice = () => {} }) {
    let scene = null, ready = false, pending = false, selectedSlides = null;
    let dialog = null, dialogSlide = null, dialogClickCapability = null, dialogSelection = null, dialogValues = null;
    const choices = [];
    const content = new Column({ text: 'panel:fixed:gap=4:flush' });
    const scroll = new ScrollView({ name: 'transition-gallery-scroll', text: 'viewport:224|0/0', state: State.FLAT }, [content]);
    const gallery = new Menu({ name: 'slide-transition', text: 'None', icon: 74, state: State.START_ALIGNED,
        tooltip: 'Slide transition', accessibleName: 'Slide transition', enabled: false }, [scroll]);
    const direction = new Menu({ name: 'transition-direction', text: 'Options', icon: 50, state: State.START_ALIGNED,
        tooltip: 'Effect Options', accessibleName: 'Effect Options', enabled: false });
    const timing = new Button({ name: 'transition-timing', text: 'Timing', icon: 50, state: State.START_ALIGNED,
        tooltip: 'Transition timing', accessibleName: 'Transition timing', enabled: false });
    const all = new Button({ name: 'transition-apply-all', text: 'Apply to All', icon: 78, state: State.START_ALIGNED,
        tooltip: 'Apply transition to all slides', accessibleName: 'Apply to All', enabled: false });
    const play = typeof preview === 'function' ? new Button({ name: 'transition-preview', text: 'Preview', icon: 82, state: State.START_ALIGNED,
        tooltip: 'Preview transition', accessibleName: 'Preview transition', enabled: false }) : null;
    const controls = [gallery, direction, timing, ...(play ? [play] : []), all];
    const targets = () => selectedSlides ?? (scene?.slideId ? [scene.slideId] : []);
    const validTargets = ids => Array.isArray(ids) && ids.length >= 1 && ids.length <= 4096 &&
        ids.every(id => typeof id === 'string' && id.length > 0 && id.length <= 1024) && new Set(ids).size === ids.length;
    const sameSelection = (a, b) => a?.length === b?.length && a.every((id, i) => id === b[i]);
    const baseUsable = () => ready && !pending && scene?.canSetTransition === true && validTransition(scene.transition) && typeof apply === 'function';
    const usable = () => baseUsable() && validTargets(targets());
    const close = () => { if (dialog) root.remove(dialog, { destroy: true }); dialog = null; dialogSlide = null; dialogClickCapability = null; dialogSelection = null; dialogValues = null; };
    function update() {
        const enabled = usable(), value = scene?.transition, ids = targets();
        const scope = ids.length > 1 ? ` ${ids.length} slides selected.` : '';
        for (const control of controls) control.setProperty('enabled', enabled);
        all.setProperty('enabled', baseUsable());
        play?.setProperty('enabled', baseUsable());
        direction.setProperty('enabled', enabled && ['push', 'wipe'].includes(value?.effect));
        gallery.text = effects.find(([id]) => id === value?.effect)?.[1] || 'Unsupported';
        gallery.setProperty('tooltip', scene?.canSetTransition === false ? 'This slide transition is not supported for editing.' : !validTargets(ids) ? 'Select up to 4096 slides to apply a transition.' : `Slide transition.${scope}`);
        timing.setProperty('tooltip', validTransition(value) ? `Duration: ${value.durationMs / 1000} s. Advance ${value.advanceOnClick ? 'on click' : 'without click'}${value.advanceAfterMs === null ? '' : `; after ${value.advanceAfterMs / 1000} s`}.${scope}` : 'Transition timing');
        for (const { item, effect } of choices) { item.setProperty('enabled', enabled && (effect !== 'morph' || scene?.canMorphTransition === true)); item.setProperty('selected', value?.effect === effect); item.setProperty('checked', value?.effect === effect); }
        for (const item of direction.children) { item.setProperty('checked', item.name === `transition-direction-${value?.direction}`); item.setProperty('enabled', direction.getProperty('enabled')); }
    }
    async function run(values, selection = targets()) {
        if (selection === null ? !baseUsable() : !usable()) return;
        const ids = selection === null ? null : [...selection];
        const settings = { ...values, advanceOnClick: scene.canDisableTransitionClick === false ? true : values.advanceOnClick };
        const command = ids && ids.length > 1 ? { type: 'set-selected-slide-transitions', slides: ids, ...settings } :
            { type: 'set-slide-transition', slide: ids?.[0] ?? null, ...settings };
        pending = true; update();
        try { await apply([command]); }
        catch (error) { notice(error.message); }
        finally { pending = false; update(); }
    }
    for (let start = 0; start < effects.length; start += 3) {
        const row = new Row({ text: 'panel:fixed:gap=4:flush' });
        for (const [effect, label] of effects.slice(start, start + 3)) {
            const sample = new SceneSurface({ name: `transition-thumbnail-${effect}`, enabled: false }); sample.submit(thumbnail(effect));
            const item = new MenuItem({ name: `transition-effect-${effect}`, text: 'panel:fixed-height:100:width=168:gap=2:flush',
                accessibleName: label, tooltip: label }, [new Container({ text: 'panel:fixed-height:64:flush', state: State.FLAT }, [sample]), new Label({ text: label }), new Tooltip({ text: label })]);
            item.on(Message.ACTIVATE, () => {
                if (!usable() || effect === 'morph' && scene?.canMorphTransition !== true) return;
                const previous = scene.transition;
                void run({ ...sourceValues(previous), effect, durationMs: previous.effect === 'none' && effect !== 'none' && previous.durationMs === 0 ? 500 : previous.durationMs });
            });
            choices.push({ item, effect }); row.append(item);
        }
        content.append(row);
    }
    for (const [value, label] of directions) {
        const item = direction.append(new MenuItem({ name: `transition-direction-${value}`, text: label }));
        item.on(Message.ACTIVATE, () => { if (direction.getProperty('enabled')) void run({ ...sourceValues(scene.transition), direction: value }); });
    }
    all.on(Message.ACTIVATE, () => { if (baseUsable()) void run(sourceValues(scene.transition), null); });
    play?.on(Message.ACTIVATE, () => {
        if (!baseUsable()) return;
        const value = { slide: scene.slideId, transition: sourceValues(scene.transition) };
        Promise.resolve().then(() => preview(value)).catch(error => notice(error.message));
    });
    timing.on(Message.ACTIVATE, () => {
        if (!usable() || dialog) return;
        dialogSlide = scene.slideId;
        dialogSelection = [...targets()];
        dialogClickCapability = scene.canDisableTransitionClick !== false;
        const original = sourceValues(scene.transition);
        dialogValues = JSON.stringify(original);
        const duration = new TextField({ name: 'transition-duration', value: String(original.durationMs / 1000), enabled: original.effect !== 'none' });
        const click = new CheckBox({ name: 'transition-on-click', text: 'On mouse click',
            checked: dialogClickCapability ? original.advanceOnClick : true, enabled: dialogClickCapability,
            tooltip: dialogClickCapability ? 'Advance this slide on mouse click' : 'This document supports advancing on mouse click; disabling it is not supported.' });
        const after = new CheckBox({ name: 'transition-after-enabled', text: 'After', checked: original.advanceAfterMs !== null });
        const seconds = new TextField({ name: 'transition-after-seconds', value: String((original.advanceAfterMs ?? 0) / 1000), enabled: original.advanceAfterMs !== null });
        after.on(Message.ACTIVATE, () => seconds.setProperty('enabled', after.getProperty('checked')));
        const error = new Label({ name: 'transition-timing-error', text: '' });
        const save = new Button({ name: 'apply-transition-timing', text: 'Apply', accent: true });
        const cancel = new Button({ name: 'cancel-transition-timing', text: 'Cancel' }); cancel.on(Message.ACTIVATE, close);
        const milliseconds = field => {
            const value = field.getProperty('value').trim();
            return /^(?:\d+(?:\.\d{1,3})?|\.\d{1,3})$/u.test(value) && Number(value) <= 600 ? Math.round(Number(value) * 1000) : null;
        };
        save.on(Message.ACTIVATE, () => {
            if (!usable() || scene.slideId !== dialogSlide || !sameSelection(targets(), dialogSelection)) return;
            const durationMs = original.effect === 'none' ? original.durationMs : milliseconds(duration);
            const advanceAfterMs = after.getProperty('checked') ? milliseconds(seconds) : null;
            if (durationMs === null || after.getProperty('checked') && advanceAfterMs === null) {
                error.text = 'Enter seconds from 0 to 600, with at most three decimal places.'; return;
            }
            const selection = [...dialogSelection], values = { ...original, durationMs, advanceOnClick: click.getProperty('checked'), advanceAfterMs };
            close(); void run(values, selection);
        });
        dialog = new Dialog({ name: 'transition-timing-dialog', text: dialogSelection.length > 1 ? `Transition timing — ${dialogSelection.length} slides` : 'Transition timing', modal: true }, [
            new Row({ text: 'panel:form-control' }, [new Label({ text: 'Duration (seconds)' }), duration]),
            new Label({ text: 'Advance slide' }), click,
            new Row({ text: 'panel:form-control' }, [after, seconds, new Label({ text: 'seconds' })]),
            new Label({ text: `The effect plays when a slide appears. Advance settings control when it leaves.${dialogSelection.length > 1 ? ` Applies to ${dialogSelection.length} selected slides.` : ''}` }), error,
            new Row({ text: 'panel:actions' }, [cancel, save]),
        ]);
        dialog.on(Message.KEY, event => { if (event.detail?.pressed && event.detail.keyCode === 9) close(); });
        root.append(dialog);
    });
    update();
    return Object.freeze({ controls, close, setState({ current = scene, ready: enabled = ready, selectedSlides: selection } = {}) {
        if (selection !== undefined) selectedSlides = Array.isArray(selection) ? [...selection] : [];
        else if (current?.slideId !== scene?.slideId) selectedSlides = null;
        scene = current; ready = enabled;
        if (dialog && (scene?.slideId !== dialogSlide || !sameSelection(targets(), dialogSelection) ||
            scene?.canSetTransition !== true || !validTransition(scene?.transition) ||
            JSON.stringify(sourceValues(scene.transition)) !== dialogValues ||
            (scene?.canDisableTransitionClick !== false) !== dialogClickCapability)) close();
        update();
    } });
}
