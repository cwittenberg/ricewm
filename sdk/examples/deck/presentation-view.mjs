import { hyperlinkAt } from './hyperlink-hit.mjs';
import { linkIdentity } from './hyperlink-contract.mjs';
import { Button, Column, Label, Message, Row, SceneBuilder, SceneSurface, State } from '@ricewm/app';
import { createPresentationController } from './presentation-controller.mjs';
import { geometryFrame } from './scene-preview.mjs';
import { prepareMorph } from './morph-render.mjs';
import { prepareTransition } from './transition-render.mjs';
import { createSlidePlayback } from './presentation-playback.mjs';
import { compileAnimationTimeline } from './animation-timeline.mjs';
import { prepareAnimationFrame } from './animation-render.mjs';
import { hitAudienceTrigger } from './audience-trigger-hit.mjs';
import { createAudiencePointer } from './audience-pointer.mjs';

/** Native read-only audience view. Fullscreen is a RiceWM window request; the
 * document, renderer and worker ownership remain the editor's shared resources. */
export function createPresentationView({ port, getWindow, changed, exited, openHyperlink, reducedMotion = () => false }) {
    let active = false, closed = false, restoring = null, fullscreen = false, previousFullscreen = false;
    let entering = null, lastFrame = null, lastBlank = null, jump = '', stopState = null, playbackError = null;
    let displayedFrame = null, pressedTarget = null, pressedLink = null;
    const pointerGesture = createAudiencePointer();
    const surface = new SceneSurface({ name: 'show-surface', state: State.EXPANSIVE, visible: false, backgroundColor: '#000000' });
    const caption = new Label({ name: 'show-caption', text: '' });
    const error = new Label({ name: 'show-error', visible: false });
    const buttons = new Map();
    function button(name, text, action) {
        const control = new Button({ name, text });
        control.on(Message.ACTIVATE, () => { if (active && !restoring && control.getProperty('enabled')) { void action(); surface.setProperty('focused', true); } });
        buttons.set(name, control); return control;
    }
    const toolbar = new Row({ name: 'show-toolbar', text: 'panel:fixed' }, [
        button('show-previous', 'Previous', () => controller.previous()),
        button('show-next', 'Next', () => advance()),
        button('show-fullscreen', 'Full screen', () => toggleFullscreen()),
        button('show-black', 'Black screen', () => controller.toggleBlank('black')),
        button('show-white', 'White screen', () => controller.toggleBlank('white')),
        button('show-edit', 'Edit presentation', () => stop()), caption,
    ]);
    const control = new Column({ name: 'presentation-view', text: 'panel:flush', visible: false, state: State.EXPANSIVE }, [toolbar, error, surface]);
    const playback = createSlidePlayback({
        paint: frame => { displayedFrame = frame; surface.submit(frame); },
        render: frame => geometryFrame(frame.scene, { textFrames: frame.textFrames, imageFrames: frame.imageFrames }).frame,
        prepareTransition, prepareMorph, reducedMotion,
        prepareAnimations(rendered) {
            if (!rendered.animationTimeline) {
                if (rendered.scene.hasAnimationTiming) throw Error('Source animation timing is unavailable; this slide cannot be played faithfully.');
                return null;
            }
            const timeline = compileAnimationTimeline(rendered.animationTimeline);
            const sample = prepareAnimationFrame(rendered, rendered.animationTimeline);
            return { timeline, sample, appearance: sample.appearance, visibility: sample.visibility };
        },
        advance: () => controller.next(),
        failed(cause) { playbackError = `Slide playback failed: ${String(cause?.message || cause).slice(0, 200)}`; error.text = playbackError; error.setProperty('visible', true); surface.setProperty('visible', false); },
    });
    const controller = createPresentationController({ port, changed: state => {
        if (closed || !active) return;
        caption.text = state.ended ? 'End of slide show · Press Escape to edit' : `Slide ${state.index + 1} of ${state.count}${state.loading ? ' · Loading…' : ''}`;
        buttons.get('show-previous').setProperty('enabled', state.index > 0 || state.ended);
        if (state.frame && state.frame !== lastFrame) playbackError = null;
        const viewError = state.error ? `Slide unavailable: ${state.error}` : playbackError;
        error.text = viewError || '';
        error.setProperty('visible', Boolean(viewError && !state.blank && !state.ended));
        surface.setProperty('visible', Boolean(state.frame) && !viewError);
        const blank = state.ended ? 'black' : state.blank;
        if (state.error || state.ended) playback.stop();
        else if (blank || state.loading) playback.pause();
        surface.setProperty('backgroundColor', blank === 'white' ? '#ffffff' : '#000000');
        if (blank) {
            if (lastBlank !== blank) surface.submit(new SceneBuilder(16, 9).rect([0, 0, 16, 9], blank === 'white' ? 0xffffffff : 0xff000000).finish());
            surface.setProperty('visible', true);
        } else if (state.frame && !viewError) {
            if (lastFrame !== state.frame) { playback.show(state.frame); lastFrame = state.frame; }
            else if (!state.loading) { playback.resume(); if (lastBlank) playback.repaint(); }
        }
        lastBlank = blank;
        pointerGesture.update(pointerState(state));
        showChrome();
    } });
    function showChrome() {
        toolbar.setProperty('visible', !fullscreen);
    }
    async function enterFullscreen(value) {
        pointerGesture.cancel(); pressedTarget = null;
        const window = getWindow();
        if (typeof window?.setFullscreen !== 'function') throw Error('This RiceWM host cannot switch presentation window mode');
        window.setTitlebarVisible(!value);
        try {
            const state = await window.setFullscreen(value);
            fullscreen = state.fullscreen; showChrome();
        } catch (cause) { window.setTitlebarVisible(true); throw cause; }
    }
    async function toggleFullscreen() {
        if (entering || restoring) return;
        entering = enterFullscreen(!fullscreen).catch(cause => {
            error.text = `Full screen unavailable: ${String(cause.message).slice(0, 160)}`; error.setProperty('visible', true);
        }).finally(() => { entering = null; });
        await entering; surface.setProperty('focused', true);
    }
    async function stop() {
        if (restoring) return restoring;
        if (!active) return;
        active = false;
        pointerGesture.cancel(); pressedTarget = null;
        playback.stop();
        restoring = (async () => {
            await controller.stop(); await entering;
            try {
                const window = getWindow();
                if (typeof window?.setFullscreen === 'function' && fullscreen !== previousFullscreen) await window.setFullscreen(previousFullscreen);
            } finally {
                getWindow()?.setTitlebarVisible?.(true);
                fullscreen = false; control.setProperty('visible', false); surface.setProperty('focused', false);
                // Release the retained audience texture before authoring resumes.
                surface.submit(new SceneBuilder(1, 1).finish()); lastFrame = null; lastBlank = null; playbackError = null;
                stopState?.(); stopState = null;
                if (!closed) { changed(false); await exited(controller.state.index); }
            }
        })().catch(cause => {
            if (!closed) { error.text = String(cause.message); }
        }).finally(() => { restoring = null; if (!closed) changed(false); });
        return restoring;
    }
    function advance({ pointer = false } = {}) {
        if (controller.state.ended) return stop();
        if (playback.finishTransition()) return Promise.resolve();
        if (playback.advanceAnimation()) return Promise.resolve();
        if (pointer && controller.state.frame?.scene.transition?.advanceOnClick === false) return Promise.resolve();
        return controller.next();
    }
    function key(event) {
        const detail = event.detail;
        if (!active || !detail?.pressed || detail.modifiers & 12) return;
        const key = String(detail.key || detail.text || '').toLowerCase();
        const codes = { 9: 'escape', 113: 'arrowleft', 114: 'arrowright', 111: 'arrowup', 116: 'arrowdown', 112: 'pageup', 117: 'pagedown', 110: 'home', 115: 'end', 65: ' ', 36: 'enter',
            56: 'b', 25: 'w', 41: 'f', 57: 'n', 33: 'p', 60: '.', 59: ',' };
        const value = key || codes[detail.keyCode] || (detail.keyCode >= 10 && detail.keyCode <= 19 ? String((detail.keyCode - 9) % 10) : '');
        if (/^[0-9]$/u.test(value)) { jump = (jump + value).slice(-4); event.preventDefault(); return; }
        let action;
        if (value === 'escape') action = stop;
        else if (value === 'enter' && jump) { const index = Number(jump) - 1; jump = ''; action = () => controller.go(index); }
        else if (['arrowright', 'arrowdown', 'pagedown', ' ', 'enter', 'n'].includes(value)) action = advance;
        else if (['arrowleft', 'arrowup', 'pageup', 'p'].includes(value)) action = () => controller.previous();
        else if (value === 'home') action = () => controller.first();
        else if (value === 'end') action = () => controller.last();
        else if (value === 'b' || value === '.') action = () => controller.toggleBlank('black');
        else if (value === 'w' || value === ',') action = () => controller.toggleBlank('white');
        else if (value === 'f') action = toggleFullscreen;
        if (action) { jump = ''; event.preventDefault(); void action(); }
    }
    surface.on(Message.KEY, key);
    surface.on(Message.SIZE_CHANGED, () => { pointerGesture.cancel(); pressedTarget = null; });
    for (const button of buttons.values()) button.on(Message.KEY, key);
    function pointerState(state = controller.state) {
        return { active: active && !restoring, generation: state.frame, frame: displayedFrame,
            blank: state.blank, loading: state.loading, failed: state.error || playbackError, closed };
    }
    function triggerAt(point) {
        const interaction = playback.interactionState;
        if (!interaction || !controller.state.frame) return null;
        return hitAudienceTrigger({ scene: controller.state.frame.scene, ...interaction, point });
    }
    function linkAt(point) {
        const source = controller.state.frame;
        if (!source || playback.transitioning || controller.state.loading || controller.state.blank || controller.state.ended || playbackError || controller.state.error) return null;
        return hyperlinkAt({ scene: source.scene, frame: displayedFrame, point, textFrames: source.textFrames,
            visibility: playback.interactionState?.visibility ?? new Map() });
    }
    surface.on(Message.POINTER, event => {
        const detail = event.detail;
        if (detail?.kind === 'move') surface.setProperty('cursor', linkAt(detail) ? 'pointer' : 'default');
        const point = pointerGesture.pointer(detail, pointerState());
        if (detail?.kind === 'down') { pressedTarget = triggerAt(detail); pressedLink = linkAt(detail); }
        if (!point) return;
        const link = linkAt(point), originalLink = pressedLink; pressedLink = null;
        if (linkIdentity(link) !== linkIdentity(originalLink)) return;
        if (link) {
            pressedTarget = null;
            const action = link.target.kind === 'slide' ? controller.go(link.slideIndex) : openHyperlink?.(link);
            Promise.resolve(action).catch(cause => { error.text = `Could not open link: ${String(cause.message).slice(0,200)}`; error.setProperty('visible',true); });
            return;
        }
        const target = triggerAt(point), original = pressedTarget; pressedTarget = null;
        if (target !== original) return;
        if (target && playback.triggerAnimation(target)) return;
        void advance({ pointer: true });
    });
    return Object.freeze({ control, get active() { return active || Boolean(restoring); },
        async start(options) {
            if (active || restoring || closed) return;
            active = true; previousFullscreen = getWindow()?.state?.fullscreen === true; fullscreen = previousFullscreen;
            stopState = getWindow()?.onState?.(state => {
                const wasFullscreen = fullscreen;
                if (wasFullscreen !== state.fullscreen) { pointerGesture.cancel(); pressedTarget = null; }
                fullscreen = state.fullscreen; showChrome();
                if (active && wasFullscreen && !fullscreen && !entering) getWindow()?.setTitlebarVisible?.(true);
            });
            changed(true); control.setProperty('visible', true); surface.setProperty('focused', true); jump = ''; showChrome();
            try {
                await controller.start(options);
                if (active && options.fullscreen !== false && !fullscreen && !controller.state.error) await toggleFullscreen();
            } catch (cause) { await stop(); throw cause; }
        },
        stop,
        async close() { closed = true; await stop(); },
    });
}
