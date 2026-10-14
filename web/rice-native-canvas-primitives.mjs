// Shrink before growing when an orientation change swaps the dimensions.
// Growing the width of a portrait canvas first temporarily allocates a
// landscape-width, portrait-height backing store.
export function resizeCanvasBackingStore(canvas, width, height) {
    const changed = canvas.width !== width || canvas.height !== height;
    if (height < canvas.height) canvas.height = height;
    if (canvas.width !== width) canvas.width = width;
    if (canvas.height !== height) canvas.height = height;
    return changed;
}

export function rgbaColor(channels) {
    if (
        !Array.isArray(channels) ||
        channels.length !== 4 ||
        channels.some(value =>
            !Number.isInteger(value) || value < 0 || value > 255
        )
    ) {
        throw new TypeError("invalid native UI color");
    }
    return `rgba(${channels[0]}, ${channels[1]}, ${channels[2]}, ` +
        `${channels[3] / 255})`;
}

export function boundedRadius(rect, radius) {
    return Math.max(0, Math.min(
        Number(radius) || 0,
        32,
        Math.floor(Math.min(rect.width, rect.height) / 2),
    ));
}

export function packedRgba(value) {
    return `rgba(${value & 0xff}, ${(value >>> 8) & 0xff}, ` +
        `${(value >>> 16) & 0xff}, ${((value >>> 24) & 0xff) / 255})`;
}

export function roundedPath(context, rect, radius) {
    const value = boundedRadius(rect, radius);
    context.beginPath();
    if (value === 0) {
        context.rect(rect.x, rect.y, rect.width, rect.height);
    } else if (typeof context.roundRect === "function") {
        context.roundRect(rect.x, rect.y, rect.width, rect.height, value);
    } else {
        const right = rect.x + rect.width;
        const bottom = rect.y + rect.height;
        context.moveTo(rect.x + value, rect.y);
        context.lineTo(right - value, rect.y);
        context.quadraticCurveTo(right, rect.y, right, rect.y + value);
        context.lineTo(right, bottom - value);
        context.quadraticCurveTo(right, bottom, right - value, bottom);
        context.lineTo(rect.x + value, bottom);
        context.quadraticCurveTo(rect.x, bottom, rect.x, bottom - value);
        context.lineTo(rect.x, rect.y + value);
        context.quadraticCurveTo(rect.x, rect.y, rect.x + value, rect.y);
    }
    return value;
}

/** Preserve theme ink when readable, otherwise choose text contrast for its actual surface. */
export function readableInk(ink, paper, minimum = 4.5) {
    const luminance = color => color.slice(0, 3).reduce((sum, channel, index) => {
        const value = channel / 255;
        return sum + (value <= .04045 ? value / 12.92 : ((value + .055) / 1.055) ** 2.4) * [.2126, .7152, .0722][index];
    }, 0);
    const alpha=(ink[3]??255)/255;
    const a=luminance(ink.slice(0,3).map((channel,index)=>channel*alpha+paper[index]*(1-alpha))), b=luminance(paper);
    return (Math.max(a,b)+.05)/(Math.min(a,b)+.05)>=minimum ? ink : b>.179 ? [0,0,0,255] : [255,255,255,255];
}
