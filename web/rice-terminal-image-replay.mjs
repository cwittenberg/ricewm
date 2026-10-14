import {globalCommandRect} from './rice-native-canvas-window-damage.mjs';

const contains = (a, b) => a && b && a.x <= b.x && a.y <= b.y &&
    a.x + a.width >= b.x + b.width && a.y + a.height >= b.y + b.height;
const overlaps = (a, b) => a && b && a.x < b.x + b.width && a.y < b.y + b.height &&
    a.x + a.width > b.x && a.y + a.height > b.y;

// Direct grid composition moves terminal rows after ordinary window commands.
// Keep the original order when later controls paint inside that grid, even
// when the producer does not attach separate dialog or popup metadata.
export function terminalGridHasForeground(native, grid) {
    return native.commands.slice(native.commands.indexOf(grid) + 1).some(command =>
        command.type !== 'terminal-row' && command.type !== 'push-clip' && command.type !== 'pop-clip' &&
        (contains(grid.rect, command.rect) ||
            command.type === 'balloon-frame' && overlaps(grid.rect, command.rect)));
}

// Retain only the small compositor underlay beneath an unobstructed terminal
// image. Ordinary damage drops it. Image ticks rebuild the same tile using
// the regular retained composition, presentation, and frame notifications.
export function captureTerminalImageReplay(renderer, context, image) {
    if (image.replaySafe === false) return null;
    const {damage, bounds} = image;
    const nativeRenderer = renderer.nativeCanvasWindow;
    if (!renderer.doubleBuffer || renderer.externalOverlay || renderer.calendarActive ||
        renderer.overviewProgress || renderer.overviewTarget || renderer.workspaceTransition ||
        renderer.windowTransitions.size || renderer.closingSceneWindows.size ||
        renderer.launchAnimations?.size || renderer.gnomeEffectRenderer.effects?.length ||
        renderer.widgetHost?.state?.instances?.some(item => item.visible) ||
        nativeRenderer.transitions.size || nativeRenderer.closingWindows.size ||
        nativeRenderer.closingPopups.size || nativeRenderer.dragVisual || nativeRenderer.layoutPreview ||
        nativeRenderer.snapPreview || nativeRenderer.wallpaper.flight ||
        nativeRenderer.desktopLaunchAnimations.size ||
        nativeRenderer.snapshot?.shell?.shellStyle === 'Embedded' ||
        nativeRenderer.snapshots.some(n => n.popup || n.dialog || n.shell?.frostedIce?.pointerLight)) return null;
    const windows = nativeRenderer.snapshots.filter(n => n.role !== 'desktop' && n.layerVisible !== false);
    const index = windows.findIndex(n => n.commands?.some(command =>
        command.type === 'terminal-grid' && contains(globalCommandRect(n, command), bounds)));
    if (index < 0 || windows.slice(index + 1).some(n => overlaps(
        {...n.frame,x:n.frame.x-48,y:n.frame.y-48,width:n.frame.width+96,height:n.frame.height+96}, damage))) return null;
    const native = windows[index];
    if (native.tileClip && !contains(native.tileClip, bounds)) return null;
    // Glass and live lower planes need fresh composition. The common solid
    // RiceWM, Afterburn, and Construct terminal textures qualify.
    if (windows.some(n => n.chrome?.material === 'frosted-ice' || n.chrome?.glassTitlebar ||
        n.shell?.frostedIce?.enabled || n.commands?.some(c =>
            ['glass','spinner','activity','media','image','inline-image','web-scene','host-portal','terminal-portal'].includes(c.type) || c.animated))) return null;
    if ([...renderer.windows.values()].some(w => w.viewable && !w.inputOnly && w.parent !== w.id &&
        !w.desktopLayer && (!w.shellLayer || overlaps(w, damage)))) return null;
    return captureImageUnderlay(renderer.canvasFactory, context, image, nativeRenderer.paintOrigin);
}

export function captureTerminalPreviewReplay(nativeRenderer, native, context, image) {
    if (image.replaySafe === false) return null;
    if (native.dialog || native.popup || native.chrome?.material === 'frosted-ice' ||
        native.chrome?.glassTitlebar || native.shell?.frostedIce?.enabled ||
        native.commands?.some(c => ['glass','spinner','activity','media','image','inline-image','web-scene','host-portal','terminal-portal'].includes(c.type) || c.animated)) return null;
    const grid = native.commands.find(c => c.type === 'terminal-grid'), t = context.getTransform();
    if (!grid || !contains({x:grid.rect.x*t.a+t.e,y:grid.rect.y*t.d+t.f,
        width:grid.rect.width*t.a,height:grid.rect.height*t.d},image.bounds)) return null;
    return captureImageUnderlay((w,h) => nativeRenderer.frostedIceMaterial.canvasFactory(w,h), context, image, null);
}

function captureImageUnderlay(canvasFactory, context, image, origin) {
    const {damage} = image;
    const x = damage.x - (origin?.x || 0), y = damage.y - (origin?.y || 0);
    if (damage.width * damage.height > 1024 * 1024 ||
        !contains({x:0,y:0,width:context.canvas.width,height:context.canvas.height},
            {x,y,width:damage.width,height:damage.height})) return null;
    const background = canvasFactory(damage.width, damage.height);
    const target = background.getContext('2d');
    target.drawImage(context.canvas, x, y, damage.width, damage.height, 0, 0, damage.width, damage.height);
    return {...image, background};
}

// The one-pixel damage padding can touch later rows or window decorations.
// Retain their final pixels after the normal frame, without copying the logo
// itself into its underlay and leaving trails on the next rotation frame.
export function finishTerminalImageReplay(replay, context, origin) {
    if (!replay) return;
    const {damage:d, bounds, background} = replay;
    const b = {x:Math.floor(bounds.x),y:Math.floor(bounds.y),
        width:Math.ceil(bounds.x+bounds.width)-Math.floor(bounds.x),
        height:Math.ceil(bounds.y+bounds.height)-Math.floor(bounds.y)};
    const target = background.getContext('2d');
    target.globalCompositeOperation = 'copy';
    const strips = [
        {x:d.x,y:d.y,width:b.x-d.x,height:d.height},
        {x:b.x+b.width,y:d.y,width:d.x+d.width-b.x-b.width,height:d.height},
        {x:b.x,y:d.y,width:b.width,height:b.y-d.y},
        {x:b.x,y:b.y+b.height,width:b.width,height:d.y+d.height-b.y-b.height},
    ];
    // A clipped copy preserves the other strips in the retained background.
    for (const r of strips) {
        if (r.width <= 0 || r.height <= 0) continue;
        target.save();
        target.beginPath();target.rect(r.x-d.x,r.y-d.y,r.width,r.height);target.clip();
        target.drawImage(context.canvas,r.x-(origin?.x||0),r.y-(origin?.y||0),r.width,r.height,
            r.x-d.x,r.y-d.y,r.width,r.height);
        target.restore();
    }
    target.globalCompositeOperation = 'source-over';
}

export function terminalImageReplayMatches(replay, damage) {
    return replay && damage?.length > 0 && damage.every(rect =>
        rect.x === replay.damage.x && rect.y === replay.damage.y &&
        rect.width === replay.damage.width && rect.height === replay.damage.height);
}
