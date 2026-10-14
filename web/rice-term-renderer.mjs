// Ordinary compositor paint; invisible image resources only supply animated pixels.
const color = value => `#${value.toString(16).padStart(6, "0")}`;
export function terminalContrast(a, b) {
    const luminance = value => [16,8,0].reduce((sum, shift, index) => {
        const channel = ((value >> shift) & 255) / 255;
        return sum + (channel <= .04045 ? channel / 12.92 : ((channel + .055) / 1.055) ** 2.4) * [.2126,.7152,.0722][index];
    }, 0);
    const x = luminance(a), y = luminance(b);
    return (Math.max(x,y)+.05)/(Math.min(x,y)+.05);
}
const blend = (a,b,t) => [16,8,0].reduce((value,shift) => value |
    (Math.round(((a>>shift)&255)*(1-t)+((b>>shift)&255)*t)<<shift),0);
const contrastCache = new Map();
export function readableTerminalInk(foreground, background, transparency = 0, alpha = 1) {
    const key = `${foreground}:${background}:${transparency}:${alpha}`;
    if (contrastCache.has(key)) return contrastCache.get(key);
    let bounds = [blend(background, 0, transparency/100), blend(background, 0xffffff, transparency/100)];
    const score = ink => Math.min(...bounds.map(bg => terminalContrast(blend(bg, ink, alpha),bg)));
    let opaque = false;
    if (Math.max(score(0),score(0xffffff)) < 4.5) {
        // A middle-tone/translucent surface or dim ink may have no safe endpoint.
        // Make that row solid; preserve dim only when the target remains feasible.
        bounds = [background,background]; opaque = true;
        if (Math.max(score(0),score(0xffffff)) < 4.5) alpha = 1;
    }
    let ink = foreground;
    if (score(ink) < 4.5) {
        const candidates = [0,0xffffff].filter(endpoint => score(endpoint) >= 4.5).map(endpoint => {
            let low=0, high=1;
            for(let i=0;i<16;i++) { const mid=(low+high)/2; if(score(blend(foreground,endpoint,mid))>=4.5) high=mid; else low=mid; }
            return {ink:blend(foreground,endpoint,high), distance:high};
        });
        ink = candidates.sort((a,b)=>a.distance-b.distance)[0]?.ink ?? foreground;
    }
    const result = {ink, alpha, opaque};
    if (contrastCache.size >= 4096) contrastCache.clear();
    contrastCache.set(key,result); return result;
}
const containsPrivateUseGlyph = text => Array.from(text).some(character => {
    const codepoint = character.codePointAt(0);
    return (codepoint >= 0xe000 && codepoint <= 0xf8ff) ||
        (codepoint >= 0xf0000 && codepoint <= 0xffffd) ||
        (codepoint >= 0x100000 && codepoint <= 0x10fffd);
});
// Cache only leaf rasterization, never a window/background or an independently
// presented surface. Animated sibling windows otherwise repaint thousands of
// unchanged terminal glyphs on every compositor frame.
const rowCache = new Map();
const maximumCacheBytes = 8 * 1024 * 1024;
let cacheBytes = 0;
let fontSet = null;
export function clearTerminalRowCache() {
    for (const entry of rowCache.values()) entry.canvas.width = 1;
    rowCache.clear();
    cacheBytes = 0;
}
function cachedRow(context, rect, row, motionRasterScale = null) {
    if (typeof OffscreenCanvas !== "function" || !context.getTransform ||
        context.globalAlpha !== 1 || context.globalCompositeOperation !== "source-over" ||
        (context.filter && context.filter !== "none") || context.shadowBlur ||
        context.shadowOffsetX || context.shadowOffsetY ||
        (context.shadowColor && context.shadowColor !== "rgba(0, 0, 0, 0)")) return null;
    const fonts = globalThis.document?.fonts;
    if (fonts !== fontSet) {
        fontSet?.removeEventListener?.("loadingdone", clearTerminalRowCache);
        fontSet = fonts;
        fontSet?.addEventListener?.("loadingdone", clearTerminalRowCache);
        clearTerminalRowCache();
    }
    if (fonts && fonts.status !== "loaded") return null;
    let { a, b, c, d, e, f } = context.getTransform();
    // During window motion reuse a leaf at the settled device scale. The
    // compositor transforms that bitmap; glyphs need not be rasterized for
    // every fractional position. Settled rows retain exact pixel rendering.
    if (motionRasterScale && b === 0 && c === 0) {
        a = motionRasterScale.x; d = motionRasterScale.y;
        e = -rect.x * a; f = -rect.y * d;
    }
    if (b !== 0 || c !== 0 || !(a > 0 && a <= 4 && d > 0 && d <= 4) ||
        !Number.isInteger(rect.width * a) || !Number.isInteger(rect.height * d) ||
        !Number.isInteger(rect.x * a + e) || !Number.isInteger(rect.y * d + f)) return null;
    const width = Math.ceil(rect.width * a), height = Math.ceil(rect.height * d);
    const bytes = width * height * 4;
    if (!(width > 0 && height > 0 && width <= 8192 && height <= 512 &&
        bytes <= maximumCacheBytes / 4)) return null;
    const textStyle = ["direction", "fontKerning", "fontStretch", "fontVariantCaps",
        "letterSpacing", "wordSpacing", "textRendering"].map(name => [name, context[name]]);
    const key = JSON.stringify([rect.width, rect.height, a, d, textStyle,
        row.image ? { ...row, image: undefined } : row]);
    let entry = rowCache.get(key);
    if (entry) {
        rowCache.delete(key);
        rowCache.set(key, entry);
        return entry;
    }
    const canvas = new OffscreenCanvas(width, height);
    const paint = canvas.getContext("2d");
    if (!paint) return null;
    for (const [name, value] of textStyle) if (value !== undefined) paint[name] = value;
    paint.setTransform(a, 0, 0, d, 0, 0);
    paintTerminalRow(paint, { ...rect, x: 0, y: 0 }, row);
    entry = { canvas, width: width / a, height: height / d, bytes };
    while (rowCache.size && (rowCache.size >= 128 || cacheBytes + bytes > maximumCacheBytes)) {
        const oldest = rowCache.keys().next().value;
        const removed = rowCache.get(oldest);
        cacheBytes -= removed.bytes;
        removed.canvas.width = 1;
        rowCache.delete(oldest);
    }
    rowCache.set(key, entry);
    cacheBytes += bytes;
    return entry;
}
// Cache source preparation independently of row painting: a ten-row SVG is
// decoded and themed once, and rasterized only once per animation frame.
const imageSources = new Map();
const imageCache = new Map();
const animatedImages = new Set();
const imageFrameInterval = 1000 / 30;
const maximumImageBytes = 16 * 1024 * 1024;
let imageBytes = 0;
let loopBytes = 0;
let loopReserved = 0;
const maximumLoopBytes = 64 * 1024 * 1024;
let imageFrame = null;
let lastImageFrame = -Infinity;
let reducedImageMotion;
function imageMotionReduced() {
    reducedImageMotion ||= globalThis.matchMedia?.('(prefers-reduced-motion: reduce)');
    return reducedImageMotion?.matches === true;
}
function mergeDamage(map, notify, rect) {
    const previous = map.get(notify);
    if (!rect || (map.has(notify) && !previous)) { map.set(notify, null); return; }
    // Keep separate windows separate. A bounding box spanning several tiled
    // images includes unrelated windows and defeats regional composition.
    const regions = previous || [];
    let merged = {...rect};
    for (let i=regions.length-1;i>=0;i--) {
        const old=regions[i];
        if (old.x > merged.x+merged.width || merged.x > old.x+old.width ||
            old.y > merged.y+merged.height || merged.y > old.y+old.height) continue;
        const x=Math.min(old.x,merged.x),y=Math.min(old.y,merged.y);
        merged={x,y,width:Math.max(old.x+old.width,merged.x+merged.width)-x,
            height:Math.max(old.y+old.height,merged.y+merged.height)-y};
        regions.splice(i,1);
    }
    regions.push(merged);
    map.set(notify,regions.length<=64 ? regions : null);
}
function notifyDamage(map, imageOnly = false) {
    const pending = [...map];
    map.clear();
    for (const [notify, regions] of pending) notify(regions || undefined, imageOnly);
}
function animateImages(now) {
    imageFrame = null;
    const due = now - lastImageFrame >= imageFrameInterval - .5;
    if (due) lastImageFrame = now;
    for (const entry of animatedImages) {
        // Renewed only by compositor paints, so cleared/scrolled-away images
        // cannot keep a desktop repainting. RAF also pauses in background tabs.
        if (now - entry.lastPaint > 250 || document.hidden ||
            imageMotionReduced()) {
            entry.image.remove();
            for (const raster of entry.rasters.values()) if (!raster.loop?.ready) clearImageLoop(raster);
            entry.damage.clear();
            animatedImages.delete(entry);
        } else if (due) {
            entry.version++;
            entry.frameTime = now;
            notifyDamage(entry.damage, true);
        }
    }
    if (animatedImages.size && imageFrame === null) imageFrame = requestAnimationFrame(animateImages);
}
function animateImage(entry, onDamage, damage, raster) {
    if (!entry.animated || !globalThis.document?.body || document.hidden ||
        typeof requestAnimationFrame !== 'function' ||
        imageMotionReduced()) return;
    if (!raster.loop?.ready && !entry.image.isConnected) {
        // Chromium freezes detached SVG image resources. This invisible image
        // only advances its clock; visible pixels stay in the normal compositor.
        entry.image.style.cssText = 'position:fixed;left:0;top:0;width:1px;height:1px;opacity:0;pointer-events:none';
        entry.image.setAttribute('aria-hidden', 'true');
        entry.image.alt = '';
        document.body.append(entry.image);
        entry.version++;
        entry.frameTime = performance.now();
    }
    entry.lastPaint = performance.now();
    mergeDamage(entry.damage, onDamage, damage);
    animatedImages.add(entry);
    if (imageFrame === null) imageFrame = requestAnimationFrame(animateImages);
}
function clearImageLoop(entry) {
    if (!entry.loop) return;
    for (const frame of new Set(entry.loop.frames.filter(Boolean))) frame.close();
    loopBytes -= entry.loop.bytes;
    loopReserved -= entry.loop.reserved;
    if (entry.bitmapRetained) { entry.bitmap = null; entry.bitmapRetained = false; entry.rasterVersion = -1; }
    entry.loop = null;
}
function clearImageRaster(entry) {
    if (entry.rasters) {
        for (const raster of entry.rasters.values()) clearImageRaster(raster);
        entry.rasters.clear();
        return;
    }
    clearImageLoop(entry);
    entry.bitmap?.close();
    entry.bitmap = null;
    if (!entry.raster) return;
    imageBytes -= entry.raster.width * entry.raster.height * 4;
    entry.raster.width = 1;
    entry.raster = null;
}
function discardImage(entry) {
    entry.image.remove();
    entry.listeners.clear();
    entry.damage.clear();
    clearImageRaster(entry);
    animatedImages.delete(entry);
    const variants = imageSources.get(entry.source);
    variants?.delete(entry.accent);
    if (!variants?.size) imageSources.delete(entry.source);
    imageCache.delete(entry);
}
function svgLoopDuration(svg) {
    // Only autonomous, uniform SMIL loops are replayable. Event-triggered,
    // one-shot, CSS and mixed-duration animations keep the live SVG path.
    if (/@(?:-webkit-)?keyframes|\banimation(?:-[\w-]+)?\s*:|\baccumulate\s*=\s*["']sum["']/i.test(svg)) return 0;
    const animations = svg.match(/<(?:animate(?:Transform|Motion|Color)?|set)\b[^>]*>/gi);
    if (!animations?.length) return 0;
    let duration = 0;
    for (const tag of animations) {
        if (/\b(?:begin|end|repeatDur)\s*=/i.test(tag) || !/\brepeatCount\s*=\s*["']indefinite["']/i.test(tag)) return 0;
        const value = /\bdur\s*=\s*["']([\d.]+)(ms|s)["']/i.exec(tag);
        if (!value) return 0;
        const ms = Number(value[1]) * (value[2].toLowerCase() === 'ms' ? 1 : 1000);
        if (!(ms >= 200 && ms <= 8000) || (duration && duration !== ms)) return 0;
        duration = ms;
    }
    return duration;
}
function preparedImage(source, accent) {
    let entry = imageSources.get(source)?.get(accent);
    if (entry) return entry;
    if (source.length > 32800 || !/^data:image\/svg\+xml;base64,[A-Za-z0-9+/]+=*$/.test(source)) return null;
    let svg;
    try { svg = atob(source.slice(source.indexOf(',') + 1)); } catch { return null; }
    let resource = source;
    if (accent !== null && svg.includes('<title id="title">riceWM logo</title>') &&
        svg.includes('stroke="#536B91"') && svg.includes('fill="#536B91"')) {
        svg = svg.replaceAll('#536B91', color(accent));
        resource = 'data:image/svg+xml;base64,' + btoa(svg);
    }
    const image = new Image();
    entry = { image, source, accent, listeners: new Map(), damage: new Map(), version: 0,
        rasters: new Map(),
        loopDuration: svgLoopDuration(svg),
        animated: /<(?:[\w.-]+:)?(?:animate(?:Transform|Motion|Color)?|set)\b|@(?:-webkit-)?keyframes\b|\banimation(?:-[\w-]+)?\s*:/i.test(svg) };
    let variants = imageSources.get(source);
    if (!variants) imageSources.set(source, variants = new Map());
    variants.set(accent, entry);
    imageCache.set(entry, true);
    image.onload = () => notifyDamage(entry.listeners);
    image.onerror = () => entry.listeners.clear();
    image.src = resource;
    if (imageCache.size > 32) discardImage(imageCache.keys().next().value);
    return entry;
}
function imageRasterState(entry, width, height, transform) {
    const sx = transform ? Math.hypot(transform.a, transform.b) : 1;
    const sy = transform ? Math.hypot(transform.c, transform.d) : 1;
    const w = Math.ceil(width * sx), h = Math.ceil(height * sy);
    const key = `${w}:${h}`;
    let raster = entry.rasters.get(key);
    if (!raster) {
        // Desktop and Construct app faces can paint the same source at different
        // scales. Retain each size instead of throwing away its warmed cycle on
        // every alternating paint. All sizes share one SVG clock and source.
        raster = Object.assign(Object.create(entry), {rasters:null,owner:entry,w,h,
            raster:null,rasterVersion:-1,bitmap:null,bitmapRetained:false,loop:null});
        if (entry.rasters.size >= 4) {
            const oldest = entry.rasters.keys().next().value;
            clearImageRaster(entry.rasters.get(oldest));entry.rasters.delete(oldest);
        }
    } else entry.rasters.delete(key);
    entry.rasters.set(key,raster);
    raster.lastPaint = performance.now();
    return raster;
}
function imageRaster(entry) {
    const {w,h} = entry;
    if (!(w > 0 && h > 0 && w * h <= 1024 * 1024)) return entry.image;
    if (!entry.raster) {
        const bytes = w * h * 4;
        for (const candidate of imageCache.keys()) {
            if (imageBytes + bytes <= maximumImageBytes) break;
            if (candidate !== entry.owner) clearImageRaster(candidate);
        }
        const raster = typeof OffscreenCanvas === 'function' ? new OffscreenCanvas(w, h)
            : globalThis.document?.createElement('canvas');
        if (!raster) return entry.image;
        raster.width = w; raster.height = h;
        entry.raster = raster;
        entry.rasterVersion = -1;
        imageBytes += bytes;
    }
    // A terminal image spans multiple row commands, sometimes across several
    // windows. Select replay frames at the animation tick, not per row: a slow
    // compositor pass must not splice different rotation phases into one logo.
    const now = entry.frameTime ?? performance.now();
    const count = Math.ceil(entry.loopDuration / imageFrameInterval);
    if (!entry.loop && entry.loopDuration && animatedImages.has(entry.owner)) {
        const needed = w * h * 4 * count;
        for (const candidate of imageCache.keys()) {
            if (needed <= maximumLoopBytes - loopReserved) break;
            for (const raster of candidate.rasters.values()) {
                if (raster !== entry && raster.loop &&
                    (!animatedImages.has(candidate) || now - raster.lastPaint > 250)) clearImageLoop(raster);
            }
        }
    }
    if (!entry.loop && entry.loopDuration && animatedImages.has(entry.owner) &&
        typeof entry.raster.transferToImageBitmap === 'function' &&
        w * h * 4 * count <= maximumLoopBytes - loopReserved) {
        // Warm one cycle at the ordinary playback rate. Completed cycles detach
        // the SVG resource entirely; subsequent frames are bitmap copies only.
        const reserved = w * h * 4 * count;
        loopReserved += reserved;
        entry.loop = { start: now, frames: [], count, captured: 0, bytes: 0, reserved, ready: false };
    }
    const loop = entry.loop;
    const index = loop ? Math.floor((now - loop.start) / entry.loopDuration * loop.count) % loop.count : 0;
    if (loop && !loop.ready && now - loop.start >= entry.loopDuration && loop.captured >= loop.count * .85) {
        let previous = loop.frames.findLast(Boolean);
        for (let i = 0; i < loop.count; i++) {
            if (loop.frames[i]) previous = loop.frames[i];
            else loop.frames[i] = previous;
        }
        loop.ready = true;
        if ([...entry.owner.rasters.values()].every(raster => raster.loop?.ready || now - raster.lastPaint > 250)) entry.image.remove();
    }
    if (loop?.ready && imageMotionReduced()) return loop.frames[0];
    if (loop?.ready || loop?.frames[index]) return loop.frames[index];
    if (entry.rasterVersion !== entry.version || (loop && !loop.frames[index])) {
        const paint = entry.raster.getContext('2d');
        paint.clearRect(0, 0, w, h);
        paint.drawImage(entry.image, 0, 0, w, h);
        if (entry.bitmap && !entry.bitmapRetained) entry.bitmap.close();
        entry.bitmapRetained = false;
        entry.bitmap = entry.raster.transferToImageBitmap?.() || null;
        if (loop && entry.bitmap && loopBytes + w * h * 4 <= maximumLoopBytes) {
            loop.frames[index] = entry.bitmap;
            loop.captured++;
            loop.bytes += w * h * 4;
            loopBytes += w * h * 4;
            entry.bitmapRetained = true;
        }
        entry.rasterVersion = entry.version;
    }
    return entry.bitmap || entry.raster;
}
function paintImage(context, rect, row, onDamage, damageOrigin = null, imageVisible = null, capture = null, captureOnly = false) {
    const spec = row.image;
    if (!spec || typeof Image !== 'function') return;
    const size = row.font_size || 13, cw = Math.ceil(size * 8 / 13), ch = size + 5;
    const x = rect.x + spec.column * cw, y = rect.y - spec.slice * ch;
    const width = spec.columns * cw, height = spec.rows * ch;
    const left = Math.max(rect.x, x), top = Math.max(rect.y, y);
    const right = Math.min(rect.x + rect.width, x + width), bottom = Math.min(rect.y + rect.height, y + height);
    if (right <= left || bottom <= top) return;
    const transform = context.getTransform?.();
    let damage = null;
    if (transform) {
        const { a, b, c, d, e, f } = transform;
        const points = [[left,top],[right,top],[left,bottom],[right,bottom]];
        const xs = points.map(([px,py]) => a*px+c*py+e), ys = points.map(([px,py]) => b*px+d*py+f);
        const dx = Math.floor(Math.min(...xs))-1, dy = Math.floor(Math.min(...ys))-1;
        damage = { x:dx + (damageOrigin?.x || 0), y:dy + (damageOrigin?.y || 0),
            width:Math.ceil(Math.max(...xs))+1-dx, height:Math.ceil(Math.max(...ys))+1-dy };
    }
    // A covered image must not renew its animation lease. Uncover damage
    // paints it again and resumes the same animation without changing content.
    if (damage && imageVisible && !imageVisible(damage)) return;
    const ink = row.theme?.imageAccent ?? row.theme?.selection;
    const accent = Number.isInteger(ink) && ink >= 0 && ink <= 0xffffff ? ink : null;
    const entry = preparedImage(spec.source, accent);
    if (!entry) return;
    if (!entry.image.complete) { mergeDamage(entry.listeners, onDamage, damage); return; }
    if (!entry.image.naturalWidth) return;
    // A partial text/cursor repaint may touch just one slice. Renew the whole
    // image's damage, otherwise that slice alone keeps animating while the
    // remaining rows retain an older rotation phase.
    let animationDamage = damage;
    if (transform) {
        const {a,b,c,d,e,f} = transform;
        const points = [[left,y],[right,y],[left,y+height],[right,y+height]];
        const xs = points.map(([px,py])=>a*px+c*py+e), ys = points.map(([px,py])=>b*px+d*py+f);
        const dx=Math.floor(Math.min(...xs))-1,dy=Math.floor(Math.min(...ys))-1;
        animationDamage={x:dx+(damageOrigin?.x||0),y:dy+(damageOrigin?.y||0),
            width:Math.ceil(Math.max(...xs))+1-dx,height:Math.ceil(Math.max(...ys))+1-dy};
    }
    const rasterState = imageRasterState(entry,width,height,transform);
    animateImage(entry, onDamage, animationDamage, rasterState);
    const raster = imageRaster(rasterState);
    if (capture && damage && transform.a > 0 && transform.b === 0 && transform.c === 0 && transform.d > 0 && context.globalAlpha === 1 &&
        context.globalCompositeOperation === 'source-over' && (!context.filter || context.filter === 'none')) {
        const e = transform.e + (damageOrigin?.x || 0), f = transform.f + (damageOrigin?.y || 0);
        capture(context, {damage, bounds:{x:left*transform.a+e,y:top*transform.d+f,width:(right-left)*transform.a,height:(bottom-top)*transform.d},
            paint(target, origin) {
                target.save();
                try {
                    target.setTransform(transform.a,0,0,transform.d,e-(origin?.x || 0),f-(origin?.y || 0));
                    paintImage(target, rect, row, onDamage, origin, imageVisible);
                } finally { target.restore(); }
            }});
    }
    if (captureOnly) return;
    context.save(); context.beginPath(); context.rect(rect.x, rect.y, rect.width, rect.height); context.clip();
    const rw = raster === entry.image ? raster.naturalWidth : raster.width;
    const rh = raster === entry.image ? raster.naturalHeight : raster.height;
    context.drawImage(raster, (left - x) / width * rw, (top - y) / height * rh,
        (right - left) / width * rw, (bottom - top) / height * rh,
        left, top, right - left, bottom - top);
    context.restore();
}
export function renderTerminalRow(context, rect, row, theme = null, onDamage = () => {}, motionRasterScale = null, damageOrigin = null, imageVisible = null) {
    if (theme) row = {...row, theme};
    renderTerminalRowText(context, rect, row, motionRasterScale);
    paintImage(context, rect, row, onDamage, damageOrigin, imageVisible);
}
function renderTerminalRowText(context, rect, row, motionRasterScale) {
    const cached = cachedRow(context, rect, row, motionRasterScale);
    if (cached) {
        context.drawImage(cached.canvas, rect.x, rect.y, cached.width, cached.height);
        return;
    }
    paintTerminalRow(context, rect, row);
}
// Contiguous terminal rows are disjoint clipped leaves. Paint their text first,
// then copy each image once across its adjacent slices. This preserves normal
// compositor transforms/occlusion without ten bitmap draws and subscriptions
// for a single spinning logo. Gaps and differing images keep separate draws.
export function renderTerminalRows(context, commands, theme = null, onDamage = () => {}, motionRasterScale = null, damageOrigin = null, imageVisible = null, capture = null) {
    for (let index = 0; index < commands.length;) {
        const {rect, row: original} = commands[index];
        const row = theme ? {...original, theme} : original;
        const image = row.image;
        const height = (row.font_size || 13) + 5;
        const transform = context.getTransform?.();
        let end = index + 1;
        // Fractional row boundaries have distinct Canvas sampling/clip edges.
        // Keep individual slices there so batching cannot change those pixels.
        const integralSlices = transform && transform.b === 0 && transform.c === 0 &&
            Number.isInteger(height * transform.d) && Number.isInteger(rect.y * transform.d + transform.f);
        if (image && rect.height === height && (integralSlices || imageUnderlayIsEmpty(row))) {
            while (end < commands.length) {
                const next = commands[end], spec = next.row.image;
                if (!spec || next.rect.x !== rect.x || next.rect.width !== rect.width ||
                    next.rect.height !== height || next.rect.y !== rect.y + (end - index) * height ||
                    (next.row.font_size || 13) !== (row.font_size || 13) ||
                    (!theme && next.row.theme !== row.theme) ||
                    spec.source !== image.source || spec.column !== image.column ||
                    spec.columns !== image.columns || spec.rows !== image.rows ||
                    spec.slice !== image.slice + end - index ||
                    (!integralSlices && !imageUnderlayIsEmpty(theme ? {...next.row,theme} : next.row))) break;
                end++;
            }
        }
        for (let i = index; i < end; i++) {
            const command = commands[i];
            renderTerminalRowText(context, command.rect, theme ? {...command.row, theme} : command.row, motionRasterScale);
        }
        if (integralSlices || end === index + 1) {
            paintImage(context, {...rect, height: rect.height * (end - index)}, row, onDamage, damageOrigin, imageVisible, capture);
        } else {
            // Empty image cells have no row-local overlays. Their text can be
            // retained together, while fractional SVG slices keep their exact
            // original Canvas sampling and clipping at playback.
            const sliceCommands = commands.slice(index,end);
            const paintSlices = (target, origin) => {
                target.save();
                try {
                    target.setTransform(transform.a,transform.b,transform.c,transform.d,
                        transform.e+(damageOrigin?.x||0)-(origin?.x||0),
                        transform.f+(damageOrigin?.y||0)-(origin?.y||0));
                    for(const command of sliceCommands)paintImage(target,command.rect,
                        theme ? {...command.row,theme} : command.row,onDamage,origin,imageVisible);
                } finally {target.restore();}
            };
            const following = commands[end];
            const followingY = rect.y+(end-index)*height;
            if(capture)paintImage(context,{...rect,height:rect.height*(end-index)},row,onDamage,damageOrigin,imageVisible,
                (target,image)=>capture(target,{...image,paint(canvas,origin) {
                    canvas.save();
                    try {
                        canvas.setTransform(1,0,0,1,-(origin?.x||0),-(origin?.y||0));
                        canvas.beginPath();canvas.rect(image.damage.x,image.damage.y,image.damage.width,image.damage.height);canvas.clip();
                        paintSlices(canvas,origin);
                        // A later row can cover part of the final fractional
                        // edge pixel. Replay that static overlay after the SVG.
                        if (following && !following.row.image && following.rect.y === followingY) {
                            const bounds = image.bounds;
                            canvas.beginPath();canvas.rect(Math.floor(bounds.x),Math.floor(bounds.y),
                                Math.ceil(bounds.x+bounds.width)-Math.floor(bounds.x),
                                Math.ceil(bounds.y+bounds.height)-Math.floor(bounds.y));canvas.clip();
                            canvas.setTransform(transform.a,transform.b,transform.c,transform.d,
                                transform.e+(damageOrigin?.x||0)-(origin?.x||0),transform.f+(damageOrigin?.y||0)-(origin?.y||0));
                            renderTerminalRowText(canvas,following.rect,theme ? {...following.row,theme} : following.row,motionRasterScale);
                        }
                    } finally {canvas.restore();}
                }}),true);
            paintSlices(context,damageOrigin);
        }
        index = end;
    }
}
function imageUnderlayIsEmpty(row) {
    const image = row.image, background = row.theme?.background ?? 0x10141c;
    const first = image.column, last = first + image.columns;
    if ((row.cursor != null && row.cursor >= first && row.cursor < last) ||
        (row.selection && row.selection[0] < last && row.selection[1] > first) ||
        (row.readable_colors && readableTerminalInk(0,background,row.transparency || 0).opaque)) return false;
    return row.cells.every(([column,width,text,,bg,flags]) => column >= last || column + width <= first ||
        (/^ *$/.test(text) && bg === background && !(flags & 24)));
}
// Unicode block elements have exact cell geometry. Rasterizing them directly
// avoids font gaps and supports the Symbols for Legacy Computing fractions.
export function paintTerminalBlock(context, text, x, y, width, height) {
    const code = text.codePointAt(0);
    if (text.length !== (code > 0xffff ? 2 : 1)) return false;
    let left = 0, top = 0, right = 1, bottom = 1;
    if (code === 0x2580) bottom = .5;
    else if (code >= 0x2581 && code <= 0x2588) top = 1 - (code - 0x2580) / 8;
    else if (code >= 0x2589 && code <= 0x258f) right = (0x2590 - code) / 8;
    else if (code === 0x2590) left = .5;
    else if (code === 0x2594) bottom = 1 / 8;
    else if (code === 0x2595) left = 7 / 8;
    else if (code >= 0x1fb82 && code <= 0x1fb86) bottom = [2, 3, 5, 6, 7][code - 0x1fb82] / 8;
    else if (code >= 0x1fb87 && code <= 0x1fb8b) left = 1 - [2, 3, 5, 6, 7][code - 0x1fb87] / 8;
    else return false;
    context.fillRect(x + left * width, y + top * height,
        (right - left) * width, (bottom - top) * height);
    return true;
}
function paintTerminalRow(context, rect, row) {
    const size = row.font_size || 13;
    const cellWidth = Math.ceil(size * 8 / 13), cellHeight = size + 5;
    context.save();
    context.textAlign = "left";
    context.textBaseline = "middle";
    // A scene preview can scale the native grid to fractional device pixels.
    // Clipping each background to its row antialiases both sides of every
    // shared edge, exposing the paper beneath ncurses' otherwise solid panel.
    // Partition backgrounds at the same device boundary; clip glyphs below.
    const transform = context.getTransform?.();
    const fillBackground = (x, width) => {
        const left = Math.max(rect.x, x), right = Math.min(rect.x + rect.width, x + width);
        if (right <= left) return;
        const top = rect.y, bottom = rect.y + Math.min(cellHeight, rect.height);
        if (transform && transform.b === 0 && transform.c === 0 && transform.a > 0 && transform.d > 0) {
            const x0 = Math.round(left * transform.a + transform.e), x1 = Math.round(right * transform.a + transform.e);
            const y0 = Math.round(top * transform.d + transform.f), y1 = Math.round(bottom * transform.d + transform.f);
            context.fillRect((x0-transform.e)/transform.a, (y0-transform.f)/transform.d,
                (x1-x0)/transform.a, (y1-y0)/transform.d);
        } else context.fillRect(left, top, right-left, bottom-top);
    };
    // Paint contiguous backgrounds in one operation. Besides reducing ncurses
    // draw calls, this avoids seams between neighboring cells under scaling.
    context.globalAlpha = 1;
    let runColor = null, runStart = 0, runEnd = 0;
    const fillRun = () => {
        if (runColor === null) return;
        context.fillStyle = color(runColor);
        fillBackground(rect.x + runStart * cellWidth, (runEnd - runStart) * cellWidth);
    };
    for (const [column, width, , fg, bg] of row.cells) {
        const selected = row.selection && column < row.selection[1] && column + width > row.selection[0];
        const background = row.cursor === column ? (row.cursor_color ?? row.theme?.foreground ?? fg)
            : selected ? (row.theme?.selection ?? 0x365880) : bg === (row.theme?.background ?? 0x10141c) ? null : bg;
        if (background !== runColor || column !== runEnd) {
            fillRun(); runStart = column; runColor = background;
        }
        runEnd = column + width;
    }
    fillRun();
    const baseBackground = row.theme?.background ?? 0x10141c;
    const solid = row.readable_colors && readableTerminalInk(0, baseBackground, row.transparency || 0).opaque;
    if (solid) {
        context.fillStyle = color(baseBackground);
        fillBackground(rect.x, rect.width);
        // Repaint explicit cell backgrounds/cursor after strengthening the surface.
        for (const [column,width,,fg,bg] of row.cells) {
            const selected = row.selection && column < row.selection[1] && column + width > row.selection[0];
            context.fillStyle = color(row.cursor === column ? (row.cursor_color ?? row.theme?.foreground ?? fg) : selected ? (row.theme?.selection ?? 0x365880) : bg);
            fillBackground(rect.x + column*cellWidth, width*cellWidth);
        }
    }
    context.beginPath();
    context.rect(rect.x, rect.y, rect.width, rect.height);
    context.clip();
    let font = "";
    for (const [column, width, text, fg, bg, flags] of row.cells) {
        const visibleText = text !== " " && text !== "" &&
            !(row.icon_glyphs === false && containsPrivateUseGlyph(text));
        if (!visibleText && !(flags & 24)) continue;
        const selected = row.selection && column < row.selection[1] && column + width > row.selection[0];
        const cursor = row.cursor === column;
        const x = rect.x + column * cellWidth;
        let ink = cursor ? baseBackground : selected ? (row.theme?.selectionForeground ?? 0xffffff) : fg;
        let alpha = flags & 2 ? .65 : 1;
        if (row.readable_colors) {
            const background = cursor ? (row.cursor_color ?? row.theme?.foreground ?? fg) : selected ? (row.theme?.selection ?? 0x365880) : bg;
            const corrected = readableTerminalInk(ink, background, !solid && !cursor && !selected && bg === baseBackground ? row.transparency || 0 : 0, alpha);
            // For dim text, retain the row's actual surface and use full opacity
            // if dimming cannot meet contrast on it.
            const final = corrected.opaque && !solid && bg === baseBackground
                ? readableTerminalInk(ink, background, row.transparency || 0, 1) : corrected;
            ink = final.ink; alpha = final.alpha;
        }
        context.fillStyle = color(ink);
        context.globalAlpha = alpha;
        if (visibleText && !paintTerminalBlock(context, text, x, rect.y, width * cellWidth, cellHeight)) {
            const iconFonts = row.icon_glyphs === false
                ? ""
                : ', "Symbols Nerd Font Mono"';
            const nextFont = `${flags & 4 ? "italic " : ""}${flags & 1 ? "bold " : ""}${size}px "RiceWM Mono"${iconFonts}, monospace`;
            if (font !== nextFont) { context.font = nextFont; font = nextFont; }
            context.fillText(text, x, rect.y + cellHeight / 2, width * cellWidth);
        }
        if (flags & 8) context.fillRect(x, rect.y + cellHeight - 2, width * cellWidth, 1);
        if (flags & 16) context.fillRect(x, rect.y + cellHeight / 2, width * cellWidth, 1);
    }
    context.restore();
}
