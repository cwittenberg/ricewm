import { exact, failure } from './contract-values.mjs';

const identity = value => typeof value === 'string' && value.length > 0 && value.length <= 4096 && !/[\ud800-\udfff]/u.test(value);
const integer = value => Number.isSafeInteger(value) && value >= 0 && value <= 4096;
export const SLIDE_COMMANDS = new Set(['insert-slide', 'duplicate-slide', 'delete-slide', 'delete-selected-slides', 'set-slides-hidden', 'move-slide', 'set-slide-size', 'set-slide-background', 'set-slide-layout']);
const layoutId = value => typeof value === 'string' && /^[a-z][a-z0-9-]{0,63}$/u.test(value);

export function checkedSlideCommand(command) {
    const fail = () => { throw failure('INVALID_REQUEST', 'Invalid slide authoring command'); };
    if (!SLIDE_COMMANDS.has(command?.type)) fail();
    if (['delete-selected-slides', 'set-slides-hidden'].includes(command.type)) {
        const keys = command.type === 'set-slides-hidden' ? ['type', 'slides', 'hidden'] : ['type', 'slides'];
        if (!exact(command, keys) || !Array.isArray(command.slides) || !command.slides.length || command.slides.length > 4096 ||
            !command.slides.every(identity) || new Set(command.slides).size !== command.slides.length ||
            command.type === 'set-slides-hidden' && typeof command.hidden !== 'boolean') fail();
        return { ...command, slides: [...command.slides] };
    } else if (command.type === 'set-slide-size') {
        if (!exact(command, ['type', 'width', 'height']) || ![command.width, command.height].every(value =>
            typeof value === 'number' && Number.isFinite(value) && value > 0 && value <= 5376)) fail();
    } else if (command.type === 'set-slide-background') {
        if (!exact(command, ['type', 'slide', 'color']) || (command.slide !== null && !identity(command.slide)) ||
            typeof command.color !== 'string' || !/^#[0-9a-f]{6}$/iu.test(command.color)) fail();
    } else if (command.type === 'insert-slide') {
        if (!exact(command, ['type', 'after', 'layout'], ['type', 'after']) || (command.after !== null && !identity(command.after)) ||
            command.layout !== undefined && !layoutId(command.layout)) fail();
    } else if (command.type === 'set-slide-layout') {
        if (!exact(command, ['type', 'slide', 'layout']) || !identity(command.slide) || !layoutId(command.layout)) fail();
    } else {
        if (!identity(command.slide)) fail();
        if (command.type === 'move-slide') {
            if (!exact(command, ['type', 'slide', 'before']) || (command.before !== null && !identity(command.before))) fail();
        } else if (!exact(command, ['type', 'slide'])) fail();
    }
    return { ...command };
}

export function outlineArguments(args) {
    if (!exact(args, ['startSlide', 'limit', 'anchor'], [])) throw failure('INVALID_REQUEST', 'Invalid slide outline');
    const result = { startSlide: args.startSlide ?? 0, limit: args.limit ?? 32 };
    if (!integer(result.startSlide) || !integer(result.limit) || result.limit < 1 || result.limit > 64 ||
        (args.anchor !== undefined && !identity(args.anchor))) throw failure('INVALID_REQUEST', 'Invalid bounded slide outline');
    if (args.anchor !== undefined) result.anchor = args.anchor;
    return result;
}

/** Validate disposable navigation data; authoritative ordering remains in Rust. */
export function outlineResult(data, args) {
    const fail = () => { throw failure('INVALID_RESPONSE', 'Invalid slide outline response'); };
    if (!integer(data.slideCount) || !integer(data.startSlide) || !Array.isArray(data.slides) || data.slides.length > 64 ||
        (data.anchorIndex !== null && (!integer(data.anchorIndex) || data.anchorIndex >= data.slideCount))) fail();
    const seen = new Set();
    for (const [offset, slide] of data.slides.entries()) {
        if (!exact(slide, ['index', 'id', 'name', 'hidden', 'nameSpecified'], ['index', 'id', 'name']) ||
            slide.hidden !== undefined && typeof slide.hidden !== 'boolean' || slide.index !== data.startSlide + offset ||
            slide.index >= data.slideCount || !identity(slide.id) || seen.has(slide.id) ||
            (slide.nameSpecified !== undefined && typeof slide.nameSpecified !== 'boolean') ||
            typeof slide.name !== 'string' || [...slide.name].length > 256 || /[\ud800-\udfff]/u.test(slide.name)) fail();
        seen.add(slide.id);
    }
    if (data.slideCount === 0 ? data.startSlide !== 0 || data.slides.length !== 0 : data.startSlide >= data.slideCount || !data.slides.length) fail();
    const end = data.startSlide + data.slides.length;
    if (data.nextSlide !== (end < data.slideCount ? end : null)) fail();
    if (args) {
        const expectedStart = data.anchorIndex !== null ? Math.floor(data.anchorIndex / args.limit) * args.limit
            : Math.min(args.startSlide, Math.max(0, Math.floor((data.slideCount - 1) / args.limit) * args.limit));
        if (data.startSlide !== expectedStart || data.slides.length !== Math.min(args.limit, data.slideCount - data.startSlide)) fail();
        if (data.anchorIndex !== null && (!args.anchor || data.slides[data.anchorIndex - data.startSlide]?.id !== args.anchor)) fail();
        if (args.anchor && data.anchorIndex === null && seen.has(args.anchor)) fail();
    }
    return data;
}
