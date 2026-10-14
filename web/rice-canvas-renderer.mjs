import {desktopThemeFont,themeColor} from './rice-theme-customization.mjs';
import {RiceThemeSkinRenderer} from './rice-theme-skin-renderer.mjs';
import {classicUiFont, ensureClassicUiFonts} from "./rice-classic-ui.mjs";
import {t, riceLocale} from './rice-i18n.mjs';
import {overviewTheme} from './rice-overview-theme.mjs';
import {RiceWidgetSurfaceHost} from './rice-widget-surface-host.mjs';
import {topbarRows} from "./rice-topbar-layout.mjs";
import * as CanvasRendererShared from "./canvas-renderer/shared.mjs";
const {MAX_NATIVE_MEDIA_REQUEST_TRACKING,MAX_NATIVE_MEDIA_SURFACES,RICEWM_FIRST_MANAGED_FRAME_ID,RICEWM_GNOME_BACKGROUND_ID,RICEWM_GNOME_PANEL_ID,RICEWM_MANAGED_FRAME_ID_LIMIT,RICEWM_MENU_ID,RICEWM_NATIVE_PREVIEW_PREFIX,RICEWM_NOTIFICATION_ID,RICEWM_TASKBAR_ID,RICEWM_TASK_PREVIEW_ID,RICE_BUILTIN_MENU_ICONS,RICE_NATIVE_MEDIA_CACHE_MAX_BYTES,RICE_NATIVE_MENU_ICONS,RICE_SCENE_COMMAND_BYTES,RICE_SCENE_DAMAGE_WINDOW,RICE_SCENE_OWNED_WINDOW_STACK_FLAG,RICE_SCENE_REMOVE_WINDOW,RICE_SCENE_RESET,RICE_SCENE_RESIZABLE_WINDOW_STACK_FLAG,RICE_SCENE_STACK_INDEX_MASK,RICE_SCENE_UPSERT_WINDOW,ROOT_PARENT_SENTINEL,appendWedgedSurfacePath,defaultCancelFrame,defaultCanvasFactory,defaultRequestFrame,overviewEase,pixelColor,riceFocusOverlayRects,riceNativeMenuIconName,riceNativeTaskIconName,riceOverviewLayout,riceWindowTransitionDuration,riceWindowTransitionOrigin,riceWmBarBorderPixels,sameSceneWindow,appendWindowSilhouette,RiceNativeCanvasWindow,FrostedIceMaterial,RiceGnomeEffectRenderer,riceIconName,NATIVE_ENTRYPOINTS,nativeEntrypointIconName,nativeEntrypointTitle,nativeTitlebarLayout} = CanvasRendererShared;
import { RiceCanvasRendererBase } from "./canvas-renderer/base.mjs";
import { resizeCanvasBackingStore, readableInk, rgbaColor } from "./rice-native-canvas-primitives.mjs";
import {captureTerminalImageReplay, finishTerminalImageReplay, terminalImageReplayMatches} from './rice-terminal-image-replay.mjs';

export {
    RICE_SCENE_COMMAND_BYTES, RICE_SCENE_RESET, RICE_SCENE_UPSERT_WINDOW,
    RICE_SCENE_REMOVE_WINDOW, RICE_SCENE_DAMAGE_WINDOW,
    RICE_NATIVE_MEDIA_CACHE_MAX_BYTES, riceWmBarBorderPixels,
    riceNativeMenuIconName, riceNativeTaskIconName,
    riceWindowTransitionDuration, riceWindowTransitionOrigin,
    riceOverviewLayout, riceFocusOverlayRects,
} from "./canvas-renderer/shared.mjs";

export function riceCanvasVerticalTrayLayout(
    taskbar,
    trayItems,
    workspaceCount = 4,
) {
    if (!taskbar || taskbar.height <= taskbar.width) return [];
    let cursor = Math.max(0, taskbar.height - 6);
    const layout = [];
    for (let index = trayItems.length - 1; index >= 0; index -= 1) {
        const item = trayItems[index];
        const extent = item?.type === "workspaces"
            ? Math.max(0, workspaceCount) * 28
            : ["clock", "performance"].includes(item?.type)
                ? 30
                : 28;
        cursor = Math.max(0, cursor - extent);
        layout.push({ index, item, top: cursor, extent });
        cursor = Math.max(0, cursor - 4);
    }
    return layout.reverse();
}

export function riceCanvasVerticalStorageIndicatorRect(
    taskbar,
    trayItems,
    workspaceCount = 4,
) {
    const layout = riceCanvasVerticalTrayLayout(
        taskbar,
        trayItems,
        workspaceCount,
    );
    const firstTrayTop = layout.reduce(
        (top, item) => Math.min(top, item.top),
        Math.max(0, taskbar.height - 6),
    );
    return {
        x: taskbar.x + Math.max(0, Math.floor((taskbar.width - 16) / 2)),
        y: taskbar.y + Math.max(32, Math.min(taskbar.height - 19, firstTrayTop - 23)),
        width: 16,
        height: 16,
    };
}

export function riceCanvasTaskbarLogoRect(taskbar, redmond98 = false) {
    const vertical = taskbar.height > taskbar.width;
    const size = redmond98 ? 16 : 22;
    return {
        x: taskbar.x + (vertical
            ? Math.max(0, Math.floor((taskbar.width - size) / 2))
            : 6),
        y: taskbar.y + (redmond98 ? 8 : 5),
        width: size,
        height: size,
    };
}

const RICE_MATRIX_NOTIFICATION_ACCENTS = Object.freeze({
    info: Object.freeze({ rgb: "16, 185, 129", text: "#a7f3d0" }),
    storage: Object.freeze({ rgb: "16, 185, 129", text: "#d1fae5" }),
    warning: Object.freeze({ rgb: "250, 204, 21", text: "#fef08a" }),
    error: Object.freeze({ rgb: "248, 113, 113", text: "#fecaca" }),
});

export function riceMatrixNotificationPalette(kind = "info") {
    return RICE_MATRIX_NOTIFICATION_ACCENTS[kind] ||
        RICE_MATRIX_NOTIFICATION_ACCENTS.info;
}

export function riceMatrixNotificationBreathe(atMs, enabled = true) {
    if (!enabled) return Object.freeze({
        borderAlpha: .5,
        glowAlpha: .16,
        glowBlur: 18,
    });
    const phase = ((Number(atMs) || 0) % 4000 + 4000) % 4000 / 4000;
    const amount = (1 - Math.cos(phase * Math.PI * 2)) / 2;
    const stable = value => Number(value.toFixed(4));
    return Object.freeze({
        borderAlpha: stable(.3 + .5 * amount),
        glowAlpha: stable(.1 + .2 * amount),
        glowBlur: stable(10 + 15 * amount),
    });
}

export function riceMatrixNotificationKindFromPixels(pixels, fallback = "info") {
    const bytes = pixels instanceof Uint8Array
        ? pixels
        : new Uint8Array(pixels || 0);
    const accents = new Map([
        ["34,197,94", "storage"],
        ["245,158,11", "warning"],
        ["239,68,68", "error"],
        ["56,189,248", "info"],
    ]);
    for (let offset = 0; offset + 2 < bytes.byteLength; offset += 4) {
        const kind = accents.get(
            `${bytes[offset]},${bytes[offset + 1]},${bytes[offset + 2]}`,
        );
        if (kind) return kind;
    }
    return Object.hasOwn(RICE_MATRIX_NOTIFICATION_ACCENTS, fallback)
        ? fallback
        : "info";
}

export class RiceCanvasRenderer extends RiceCanvasRendererBase {
    setWidgets(state) {
        this.widgetHost ||= new RiceWidgetSurfaceHost(this);
        const signature = JSON.stringify(state ?? null);
        if (signature === this.widgetStateSignature) {
            this.widgetHost.reconcileHover();
            return;
        }
        this.widgetStateSignature = signature;
        this.widgetHost.update(state);
        this.widgetHost.invalidate();
    }

    get shellOverlayActive() {
        return Boolean(this.externalOverlay && this.externalOverlayCapturesInput) || super.shellOverlayActive;
    }

    setVisualEffectWebGpuDevice(device, format) {
        this.nativeCanvasWindow.frostedIceMaterial.setWebGpuDevice(device, format);
        this.shellFrostedIceMaterial.setWebGpuDevice(device, format);
    }

    // Optional target-provided shell accessories share retained shell damage.
    // Unlike a modal overlay, an idle accessory must not force full-frame work.
    setShellAccessoryPainter(paint = null) {
        this.shellAccessoryPainter = typeof paint === "function" ? paint : null;
        this.damaged = true;
        this.schedule();
    }

    setExternalOverlay(paint = null, { capturesInput = true } = {}) {
        const next = typeof paint === "function" ? paint : null;
        if (next !== this.externalOverlay) this.externalOverlay = next;
        // A closing overlay may still paint its fade without owning new keys.
        this.externalOverlayCapturesInput = Boolean(next && capturesInput);
        // An overlay is outside the retained Rice/X11 damage protocol. Both
        // adding and removing it must repaint the complete composition so the
        // previous pixels cannot remain on a directly presented output.
        this.damaged = true;
        this.schedule();
    }

    captureDesktopCanvas({includeApplications=false,includeMenu=false}={}) {
        const root=[...this.windows.values()].find(window=>window.parent===window.id);
        if(!root||root.width<1||root.height<1||root.width>8192||root.height>8192)throw Error('The desktop is not ready for capture');
        const output=this.canvasFactory(root.width,root.height),context=output.getContext('2d',{alpha:false});
        const native=this.nativeCanvasWindow,original=this.context,originalNative=native.context;
        const saved={paintDamageRects:native.paintDamageRects,paintOrigin:native.paintOrigin,desktopBackdropKey:native.desktopBackdropKey,staticBackdropKey:native.staticBackdropKey};
        const children=new Map();
        for(const window of this.windows.values()){
            if(window.id===root.id||(!includeMenu&&window.id===RICEWM_MENU_ID)||[RICEWM_NOTIFICATION_ID,RICEWM_TASK_PREVIEW_ID].includes(window.id))continue;
            const siblings=children.get(window.parent)||[];siblings.push(window);children.set(window.parent,siblings);
        }
        for(const siblings of children.values())siblings.sort((a,b)=>a.stackIndex-b.stackIndex||a.id-b.id);
        try{
            this.context=context;native.setRenderContext(context);native.setPaintDamage(null);native.paintOrigin=null;native.desktopBackdropKey=null;native.staticBackdropKey=null;
            native.beginBackdropEpoch(++this.frostedBackdropGeneration);
            if(root.surface)context.drawImage(root.surface,0,0);else{context.fillStyle=pixelColor(root.backgroundPixel);context.fillRect(0,0,root.width,root.height);}
            native.renderDesktop(root,{extendWallpaperToTop:this.topbarController?.transparent===true});
            this._renderChildren(root,children,new Set(),window=>window.id===RICEWM_GNOME_BACKGROUND_ID);
            this.gnomeEffectRenderer.render(context,{width:root.width,height:root.height});
            this.customEffects.render(context,{width:root.width,height:root.height,reducedMotion:true});
            this.widgetHost?.paint(context,{width:root.width,height:root.height});
            const applications=()=>this._renderChildren(root,children,new Set(),window=>!window.desktopLayer&&!window.shellLayer);
            if(includeApplications){if(native.foregroundActive){applications();native.renderWindows(root);}else{native.renderWindows(root);applications();}}
            this._renderChildren(root,children,new Set(),window=>window.shellLayer&&window.id!==RICEWM_GNOME_BACKGROUND_ID);
            return output;
        }finally{
            this.context=original;Object.assign(native,saved);native.setRenderContext(originalNative);native.invalidateBackdrop();
        }
    }

    render() {
        const started = performance.now();
        try { return this._renderFrame(); }
        finally { const work=Math.max(0,performance.now()-started);this.cpuWorkMs=(this.cpuWorkMs||0)+work;this.customEffects.setInputs({session:Math.round(Math.min(1,work/33.333)*100)/100}); }
    }

    cpuSnapshot() {
        return { totalMs: this.cpuWorkMs || 0,
            apps: Object.fromEntries(this.nativeCanvasWindow.cpuWorkByApp || []) };
    }

    _renderFrame() {
        // Read recipe preferences before composing the wallpaper and panel.
        if(this.topbarController?.refreshTransparency())this.damaged=true;
        const effectMotionOff=this.nativeCanvasWindow.snapshot?.shell?.motionEnabled===false;
        if(this.customEffects.layers.some(l=>l.effect.inputs.includes('audio'))){const sample=this.widgetHost?.audio?.sample?.(),frequency=sample?.active?sample.frequency:null;this.customEffects.setInputs({audio:frequency?.length?[...frequency].reduce((sum,v)=>sum+v,0)/(255*frequency.length):0});}
        this.customEffects.pause(effectMotionOff||Boolean(this.customEffects.failed)||this.customEffects.mode==='disabled'||globalThis.document?.hidden===true);
        const imageReplay = this.terminalImageOnlyDamage &&
            terminalImageReplayMatches(this.terminalImageReplay, this.damageRects)
            ? this.terminalImageReplay : null;
        this.terminalImageOnlyDamage = false;
        if (this.nativeCanvasWindow.snapshot?.chrome?.redmond98 && !this.classicFontsRequested) {
            this.classicFontsRequested = true;
            ensureClassicUiFonts()?.then(() => this.render()).catch(() => {});
        }
        const pointerLightActive = this.nativeCanvasWindow.snapshots.some(snapshot =>
            snapshot.shell?.frostedIce?.enabled === true &&
            snapshot.shell?.frostedIce?.pointerLight === true
        );
        // Pointer lighting changes the optical response of every frosted
        // surface. Regional clips can therefore leave stale rectangles in
        // windows that were not part of the most recent protocol damage. Use
        // one complete, atomically presented composition while it is active.
        const nativeTransitionActive = this.nativeCanvasWindow.transitions.size > 0;
        const embeddedActive = this.nativeCanvasWindow.snapshot?.shell?.shellStyle === "Embedded";
        const matrixNotification = this.windows.get(RICEWM_NOTIFICATION_ID);
        const notificationBreathing = matrixNotification?.viewable === true &&
            this.nativeCanvasWindow.snapshot?.shell?.motionEnabled !== false;
        if (notificationBreathing && Array.isArray(this.damageRects)) {
            this.damageRects.push({
                x: matrixNotification.x - 32,
                y: matrixNotification.y - 32,
                width: matrixNotification.width + 64,
                height: matrixNotification.height + 64,
            });
        }
        let damageRects = pointerLightActive || nativeTransitionActive || embeddedActive || this.embeddedWasActive || this.externalOverlay
            ? null : imageReplay ? this.damageRects : this._backdropDamage(this.damageRects);
        // X11 can report identical damage from more than one event. Keep it
        // once so a tiny update still uses the bounded single-region surface.
        if (Array.isArray(damageRects) && damageRects.length > 1) {
            const seen = new Set();
            damageRects = damageRects.filter(rect => {
                const key = `${rect.x},${rect.y},${rect.width},${rect.height}`;
                if (seen.has(key)) return false;
                seen.add(key);
                return true;
            });
        }
        this.embeddedWasActive = embeddedActive;
        this.lastDamageRects = Array.isArray(damageRects)
            ? damageRects.map(rect => ({ ...rect }))
            : null;
        this._damaged = false;
        this.damageRects = [];
        this._advanceOverview();
        const roots = [...this.windows.values()]
            .filter(window => window.parent === window.id)
            .sort((left, right) => left.id - right.id);
        const root = roots[0];
        if (!root) return;
        if (resizeCanvasBackingStore(this.canvas, root.width, root.height)) {
            damageRects = null;
        }
        const glassMaterial = this.nativeCanvasWindow.frostedIceMaterial;
        const cpuGlass = glassMaterial.gpuUnavailable && !glassMaterial.webgpu?.pipeline &&
            this.nativeCanvasWindow.snapshots.some(native=>native.chrome?.frostedIce?.model === 'hyprglass');
        if (this.doubleBuffer && (
            !this.compositionSurface ||
            this.compositionCpuGlass !== cpuGlass ||
            this.compositionSurface.width !== root.width ||
            this.compositionSurface.height !== root.height
        )) {
            if (!this.compositionSurface || this.compositionCpuGlass !== cpuGlass) {
                if (this.compositionSurface) resizeCanvasBackingStore(this.compositionSurface, 0, 0);
                this.compositionSurface = this.canvasFactory(root.width, root.height);
                this.compositionContext = this.compositionSurface.getContext?.("2d", {
                    alpha: true,
                    willReadFrequently: cpuGlass,
                }) || null;
            }
            resizeCanvasBackingStore(this.compositionSurface, root.width, root.height);
            this.compositionCpuGlass = cpuGlass;
            if (!this.compositionContext) {
                throw new TypeError("RiceWM requires a Canvas2D composition buffer");
            }
            damageRects = null;
        }
        this.spatialShellLease?.sync();
        if(this.spatialShellOnly)damageRects=null;
        const backdropEpoch = ++this.frostedBackdropGeneration;
        this.shellFrostedIceMaterial.beginBackdropEpoch(backdropEpoch);
        this.nativeCanvasWindow.beginBackdropEpoch(backdropEpoch);
        this.nativeCanvasWindow.setPaintDamage(damageRects);

        const children = new Map();
        for (const window of this.windows.values()) {
            if (window.id === root.id) continue;
            const parent = this.windows.has(window.parent)
                ? window.parent
                : ROOT_PARENT_SENTINEL;
            const siblings = children.get(parent) || [];
            siblings.push(window);
            children.set(parent, siblings);
        }
        for (const [id, window] of this.closingSceneWindows) {
            if (this.windows.get(id)?.viewable) {
                this.closingSceneWindows.delete(id);
                continue;
            }
            const parent = this.windows.has(window.parent)
                ? window.parent
                : ROOT_PARENT_SENTINEL;
            const siblings = children.get(parent) || [];
            siblings.push(window);
            children.set(parent, siblings);
        }
        for (const siblings of children.values()) {
            siblings.sort((left, right) =>
                left.stackIndex - right.stackIndex || left.id - right.id
            );
        }

        // A clip on a desktop-sized recording canvas can still make Blink
        // rasterize/snapshot that entire backing store when it is copied.
        // Reconstruct small updates in a physically bounded scratch surface,
        // then commit only those pixels to both retained desktop images.
        let tile = null;
        if (this.doubleBuffer && damageRects?.length === 1 &&
            !this.overviewProgress && !this.overviewTarget && !this.workspaceTransition &&
            !this.nativeCanvasWindow.dragVisual && !this.gnomeEffectRenderer.effects?.length) {
            const rect = damageRects[0];
            const x = Math.max(0, Math.floor(rect.x));
            const y = Math.max(0, Math.floor(rect.y));
            const width = Math.min(root.width, Math.ceil(rect.x + rect.width)) - x;
            const height = Math.min(root.height, Math.ceil(rect.y + rect.height)) - y;
            if (width > 0 && height > 0 && width * height < root.width * root.height / 2) {
                tile = { x, y, width, height };
                damageRects = [tile];
                this.nativeCanvasWindow.setPaintDamage(damageRects);
                if (!this.damageSurface || this.damageSurface.width !== width || this.damageSurface.height !== height) {
                    this.damageSurface = this.canvasFactory(width, height);
                    this.damageSurface.width = width;
                    this.damageSurface.height = height;
                }
            }
        }
        const presentationContext = this.context;
        this.lastDamageRects = damageRects?.map(rect => ({...rect})) || null;
        const context = tile ? this.damageSurface.getContext('2d', {alpha:true}) : this.doubleBuffer
            ? this.compositionContext : presentationContext;
        this.context = context;
        this.nativeCanvasWindow.setRenderContext(context);
        this.nativeCanvasWindow.paintOrigin = tile;
        this.nativeCanvasWindow.commandRenderer.paintOrigin = tile;
        let capturedImage = null, imageCaptures = 0;
        this.nativeCanvasWindow.commandRenderer.terminalImageCapture = imageReplay ? null : (context, image) => {
            imageCaptures++;
            if (imageCaptures === 1) capturedImage = captureTerminalImageReplay(this, context, image);
            else capturedImage = null;
        };
        context.save();
        try {
            context.setTransform(1, 0, 0, 1, -(tile?.x || 0), -(tile?.y || 0));
            if (imageReplay && tile) {
                context.clearRect(tile.x, tile.y, tile.width, tile.height);
                context.drawImage(imageReplay.background, imageReplay.damage.x, imageReplay.damage.y);
                imageReplay.paint(context, tile);
                this.terminalImageReplayFrames = (this.terminalImageReplayFrames || 0) + 1;
            } else {
                if (tile) context.clearRect(tile.x, tile.y, tile.width, tile.height);
                if (Array.isArray(damageRects) && damageRects.length > 0) {
                    context.beginPath();
                    for (const rect of damageRects) {
                        context.rect(rect.x, rect.y, rect.width, rect.height);
                    }
                    context.clip();
                }
                if(this.spatialShellOnly){context.clearRect(0,0,root.width,root.height);}else {
                if (root.surface) {
                    if (Array.isArray(damageRects)) {
                        for (const rect of damageRects) {
                            const x = Math.max(0, rect.x);
                            const y = Math.max(0, rect.y);
                            const right = Math.min(root.width, rect.x + rect.width);
                            const bottom = Math.min(root.height, rect.y + rect.height);
                            if (right <= x || bottom <= y) continue;
                            context.drawImage(
                                root.surface,
                                x,
                                y,
                                right - x,
                                bottom - y,
                                x,
                                y,
                                right - x,
                                bottom - y,
                            );
                        }
                    } else {
                        context.drawImage(root.surface, 0, 0);
                    }
                } else {
                    context.fillStyle = pixelColor(root.backgroundPixel);
                    if (Array.isArray(damageRects)) {
                        for (const rect of damageRects) {
                            context.fillRect(rect.x, rect.y, rect.width, rect.height);
                        }
                    } else {
                        context.fillRect(0, 0, root.width, root.height);
                    }
                }
                this.nativeCanvasWindow.renderDesktop(root, {
                    extendWallpaperToTop: this.topbarController?.transparent === true,
                });
                this._renderChildren(
                    root,
                    children,
                    new Set(),
                    window => window.id === RICEWM_GNOME_BACKGROUND_ID,
                );
                this.gnomeEffectRenderer.render(context, {
                    width: root.width,
                    height: root.height,
                });
                this.customEffects.render(context,{width:root.width,height:root.height,reducedMotion:effectMotionOff});
                this.widgetHost?.paint(context, {width:root.width,height:root.height});
                // Desktop launch feedback belongs to the desktop plane. Painting
                // it after application windows lets a stale icon animation bleed
                // through newly mapped X11/USER32 clients on their next damaged
                // frame.
                this._renderLaunchAnimations();
                if (this.overviewProgress > 0) {
                    this._renderOverview(root, children);
                } else {
                    const renderX11Applications = () => this._renderChildren(
                        root,
                        children,
                        new Set(),
                        window => !window.desktopLayer && !window.shellLayer,
                    );
                    const renderNativeApplications = () => {
                        this.nativeCanvasWindow.setBackdropOcclusions(
                            this.nativeCanvasWindow.foregroundActive
                                ? [] : this._opaqueGpuWindowRects(),
                        );
                        this._renderEmbeddedWindowJoins(root);
                        this.nativeCanvasWindow.desktopBackdropKey =
                            !this.customEffects.layers.length && !this.gnomeEffectRenderer.effects?.length && !this.widgetHost?.state?.instances?.some(item=>item.visible) &&
                            !this.nativeCanvasWindow.wallpaper.flight &&
                            ![...this.windows.values()].some(w=>w.viewable && !w.inputOnly && w.parent!==w.id && !w.desktopLayer && !w.shellLayer)
                            ? `${root.width}:${root.height}:${root.backgroundPixel}:${this.sceneBackdropRevision || 0}` : null;
                        // Retain a static native lower stack while the top window moves.
                        // Dynamic/external planes require fresh sampling every frame.
                        this.nativeCanvasWindow.staticBackdropKey =
                            !this.customEffects.layers.length && !this.gnomeEffectRenderer.effects?.length && !this.widgetHost?.state?.instances?.some(item=>item.visible) &&
                            !this.nativeCanvasWindow.wallpaper.flight && !this.launchAnimations?.size &&
                            !this.windowTransitions.size && !this.workspaceTransition &&
                            !this.nativeCanvasWindow.transitions.size &&
                            !this.nativeCanvasWindow.desktopLaunchAnimations.size &&
                            ![...this.windows.values()].some(w => w.viewable && !w.inputOnly &&
                                w.parent !== w.id && !w.desktopLayer && !w.shellLayer)
                            ? `${root.width}:${root.height}:${root.backgroundPixel}:${this.staticBackdropRevision || 0}` : null;
                        this.nativeCanvasWindow.renderWindows(root);
                        this.nativeCanvasWindow.renderOverlays(root);
                        this.nativeCanvasWindow.renderPopupOverlays(root);
                    };
                    if (this.nativeCanvasWindow.foregroundActive) {
                        renderX11Applications();
                        renderNativeApplications();
                    } else {
                        renderNativeApplications();
                        renderX11Applications();
                    }
                }
                this.nativeCanvasWindow.renderLayoutPreview(root);
                this._renderEmbeddedFrame(root);
                }
                this._renderChildren(
                    root,
                    children,
                    new Set(),
                    window => window.shellLayer && (!this.spatialShellOnly || window.id!==RICEWM_TASKBAR_ID) &&
                        window.id !== RICEWM_GNOME_BACKGROUND_ID,
                );
                // Desktop-owned modal editors (for example Rename Desktop Item)
                // must be composited after every ordinary application and shell
                // plane. The main compositor uses this explicit sequence instead
                // of NativeCanvasWindow.render(), so promote them here as well.
                this.nativeCanvasWindow.renderPromotedDesktopPopups(root);
                this.nativeCanvasWindow.renderPromotedDesktopDialogs(root);
                this._renderFocusOverlay(root);
                if (this.calendarActive) this._renderCalendar(root);
                // Target-neutral transient UI can opt into the compositor's final
                // plane. This is deliberately after every native, X11 and shell
                // surface so the same shared UI is visible on direct KMS and on
                // multi-output receivers, rather than existing only in the hidden
                // Chromium owner page.
                if (this.externalOverlay) {
                    try {
                        this.externalOverlayFrame = this.externalOverlay(
                            context,
                            Object.freeze({
                                width: root.width,
                                height: root.height,
                                chrome: this.nativeCanvasWindow.snapshot?.chrome || null,
                            }),
                        );
                        this.externalOverlayError = null;
                    } catch (error) {
                        this.externalOverlayError = error instanceof Error
                            ? error.stack || error.message
                            : String(error);
                        throw error;
                    }
                } else {
                    this.externalOverlayFrame = null;
                    this.externalOverlayError = null;
                }
                finishTerminalImageReplay(capturedImage, context, tile);
            }
        } finally {
            context.restore();
            this.context = presentationContext;
            this.nativeCanvasWindow.setRenderContext(presentationContext);
            this.nativeCanvasWindow.paintOrigin = null;
            this.nativeCanvasWindow.commandRenderer.terminalImageCapture = null;
        }
        if (!imageReplay) this.terminalImageReplay = capturedImage;
        if (tile) {
            const retained = this.compositionContext;
            retained.save();
            retained.setTransform(1, 0, 0, 1, 0, 0);
            // Replace only the bounded tile, including transparent terminal
            // holes. A clipped 'copy' filters edges in Chromium's CPU path.
            retained.globalCompositeOperation = 'source-over';
            retained.clearRect(tile.x, tile.y, tile.width, tile.height);
            retained.imageSmoothingEnabled = false;
            retained.drawImage(this.damageSurface, tile.x, tile.y);
            retained.restore();
        }
        const transitionTarget = this.doubleBuffer
            ? this.compositionSurface
            : this.canvas;
        const overlayOverviewDrag=Boolean(this.workspaceTransition?.interactive&&this.overviewActive);
        const workspaceTransitionPresented =
            this._presentWorkspaceTransition(
                presentationContext,
                transitionTarget,
            );
        // A GPU presenter consumes the retained composition directly. Hosted
        // capture explicitly requests the CPU-visible copy for its duration.
        const deferCanvas = this.deferCanvasPresentation && this.gpuPresentationActive &&
            !this.captureCanvasPresentation && !overlayOverviewDrag;
        if (this.doubleBuffer && !workspaceTransitionPresented && !deferCanvas) {
            presentationContext.save();
            try {
                presentationContext.setTransform(1, 0, 0, 1, 0, 0);
                presentationContext.globalCompositeOperation = "copy";
                presentationContext.imageSmoothingEnabled = false;
                if (Array.isArray(damageRects)) {
                    presentationContext.globalCompositeOperation = 'source-over';
                    for (const rect of damageRects) {
                        const x = Math.max(0, Math.floor(rect.x));
                        const y = Math.max(0, Math.floor(rect.y));
                        const right = Math.min(root.width, Math.ceil(rect.x + rect.width));
                        const bottom = Math.min(root.height, Math.ceil(rect.y + rect.height));
                        if (right <= x || bottom <= y) continue;
                        presentationContext.clearRect(x, y, right - x, bottom - y);
                        if (tile) presentationContext.drawImage(this.damageSurface, tile.x, tile.y);
                        else presentationContext.drawImage(this.compositionSurface,
                            x, y, right - x, bottom - y, x, y, right - x, bottom - y);
                    }
                } else presentationContext.drawImage(this.compositionSurface, 0, 0);
            } finally {
                presentationContext.restore();
            }
        }
        if(overlayOverviewDrag)this.overviewInteraction?.paintDrag?.(presentationContext);
        // GPU presentation consumes the retained, complete composition. The
        // DOM canvas is a damage-clipped Canvas2D presentation target and is
        // hidden while a GPU presenter is active; it is not the authoritative
        // image to import back into the GPU. Workspace transitions are the
        // exception: their final blend is written to the DOM canvas itself.
        this.spatialShellLease?.syncHostedOverview();
        this.presentationSource = workspaceTransitionPresented || overlayOverviewDrag
            ? this.canvas : transitionTarget;
        this.presentationPatch = tile && !workspaceTransitionPresented && !overlayOverviewDrag
            ? {source:this.damageSurface,x:tile.x,y:tile.y} : null;
        // Native planes are rendered individually above for correct z-order.
        // Complete their animation bookkeeping as well: otherwise transitions
        // and activity indicators advance only when unrelated input repaints.
        this.nativeCanvasWindow.finishFrame();
        this.onFrame({
            animationFrame: this.animationFrameBoundary === true,
            width: root.width,
            height: root.height,
            windows: this.windows.size,
        });
        if (workspaceTransitionPresented && (!this.workspaceTransition?.interactive || this.workspaceTransition.goal!==null)) {
            this.damaged = true;
            this.schedule();
        } else if (this.overviewProgress !== this.overviewTarget) {
            this.damaged = true;
            this.schedule();
        } else if (this.overviewProgress === 0) {
            this.overviewItems = null;
            this.overviewLayout = [];
        }
    }

    dispose() {
        this.terminalImageReplay = null;
        if (this.frameHandle !== null) {
            this.cancelFrame(this.frameHandle);
            this.frameHandle = null;
        }
        if (this.matrixNotificationBreatheTimer !== null) {
            this.clearTimer(this.matrixNotificationBreatheTimer);
            this.matrixNotificationBreatheTimer = null;
        }
        for (const window of this.windows.values()) {
            window.scaledUploadSurface?.close?.();
        }
        this.windows.clear();
        this.shellTexts.clear();
        this.shellTextsSignature = "";
        this.menuApplicationIcons.clear();
        this.menuSourceIcons.clear();
        this.menuColorArtworkLabels.clear();
        this.menuBuiltinIcons.clear();
        this.pointerPosition = null;
        this.nativeCanvasWindow.dispose();
        this.shellFrostedIceMaterial.dispose();
        this.shellGlassTile = null;
        this.liquidPanelCache?.clear();
        this.widgetHost?.close();
        this.gnomeEffectRenderer.dispose();
        this.customEffects.dispose();
        globalThis.document?.removeEventListener('visibilitychange',this.effectVisibilityListener);
        for (const surface of this.nativeMediaSurfaces.values()) surface.close?.();
        this.nativeMediaSurfaces.clear();
        this.nativeMediaWallpaperRequests.clear();
        this.nativeMediaSurfaceBytes.clear();
        this.nativeMediaBytes = 0;
        this.nativeMediaLiveRequests.clear();
        this.nativeMediaHotRequests.clear();
        this.nativeMediaRetiredRequests.clear();
        this.nativeMediaRefreshAttempts.clear();
        this.nativeMediaRefreshQueue.clear();
        this.nativeMediaRefreshScheduled = false;
        this.nativeMediaStates.clear();
        this.shellGlassBackdrop = null;
        this.compositionSurface = null;
        this.compositionContext = null;
        this.damageSurface = null;
        this.presentationSource = null;
        this.presentationPatch = null;
        this.windowTransitions.clear();
        this.closingSceneWindows.clear();
        this.overviewProgress = 0;
        this.overviewTarget = 0;
        this.overviewItems = null;
        this.overviewLayout = [];
        this.overviewHover = null;
        this.overviewPointerActive = false;
        this.overviewCloseHover = null;
        this.overviewCloseRects.clear();
        this.overviewSearchQuery = "";
        this.overviewExternalResults = [];
        this.calendarActive = false;
        this.calendarLayout = null;
        this.workspaceTransition = null;
        if (this.nativeMediaBlinkTimer !== null) {
            this.clearTimer(this.nativeMediaBlinkTimer);
            this.nativeMediaBlinkTimer = null;
        }
        this.damaged = false;
    }

    _markNativeDamage(rects, imageOnly = false) {
        if (imageOnly !== true) this.terminalImageReplay = null;
        this.terminalImageOnlyDamage = imageOnly === true &&
            (!this._damaged || this.terminalImageOnlyDamage === true);
        if (!Array.isArray(rects)) {
            this.staticBackdropRevision = (this.staticBackdropRevision || 0) + 1;
            this._damaged = true;
            this.damageRects = null;
            return;
        }
        const valid = rects.filter(rect => rect &&
            Number.isFinite(rect.x) && Number.isFinite(rect.y) &&
            Number.isFinite(rect.width) && Number.isFinite(rect.height) &&
            rect.width > 0 && rect.height > 0
        );
        if (valid.length === 0) return;
        if (this.topbarController?.transparent && this.nativeCanvasWindow.snapshots.some(native =>
            native.role === 'desktop' && valid.some(rect => rect.x <= native.frame.x &&
                rect.y <= native.frame.y && rect.x + rect.width >= native.frame.x + native.frame.width &&
                rect.y + rect.height >= native.frame.y + native.frame.contentHeight))) {
            // Wallpaper damage names the native work area. Its transparent
            // panels expose the same backdrop above that area, too.
            for (const panel of this.windows.values()) {
                if (panel.viewable && (panel.id === RICEWM_GNOME_PANEL_ID ||
                    (panel.id >= 0xe0000100 && panel.id < 0xe0000120 && (panel.id - 0xe0000100) % 2 === 0))) {
                    valid.push({ x: panel.x, y: panel.y, width: panel.width, height: panel.height });
                }
            }
        }
        if (this._damaged && this.damageRects === null) return;
        this._damaged = true;
        this.damageRects.push(...valid);
        if (this.damageRects.length > 64) {
            const left = Math.min(...this.damageRects.map(rect => rect.x));
            const top = Math.min(...this.damageRects.map(rect => rect.y));
            const right = Math.max(...this.damageRects.map(rect => rect.x + rect.width));
            const bottom = Math.max(...this.damageRects.map(rect => rect.y + rect.height));
            this.damageRects = [{
                x: left,
                y: top,
                width: right - left,
                height: bottom - top,
            }];
        }
    }

    _sceneWindowRect(window, windows, local = null) {
        if (!window || window.inputOnly || window.viewable === false) return null;
        const border = local ? 0 : Number(window.borderWidth) || 0;
        let x = Number(local?.x) || -border;
        let y = Number(local?.y) || -border;
        const width = local ? Number(local.width) : Number(window.width) + border * 2;
        const height = local ? Number(local.height) : Number(window.height) + border * 2;
        if (!(width > 0 && height > 0)) return null;
        let current = window;
        const visited = new Set();
        while (current && current.parent !== current.id && !visited.has(current.id)) {
            visited.add(current.id);
            x += Number(current.x) || 0;
            y += Number(current.y) || 0;
            current = windows?.get(current.parent);
        }
        // Preview card shadows extend beyond the retained shell window. Damage both
        // old and new shadow footprints when the pointer moves or dismisses it.
        if(window.id===RICEWM_TASK_PREVIEW_ID)return {x:x-40,y:y-40,width:width+80,height:height+90};
        // Brackets are painted outside managed X11 geometry. Include their
        // complete footprint for both old and new scene bounds (move, resize,
        // unmap and removal), plus a pixel for antialiased strokes. Local
        // client damage and opaque-surface clipping must retain exact bounds.
        const surface = this.nativeCanvasWindow.snapshot?.shell?.surface;
        if (!local && surface?.frame === "bracket" &&
            window.id >= RICEWM_FIRST_MANAGED_FRAME_ID &&
            window.id < RICEWM_MANAGED_FRAME_ID_LIMIT) {
            const margin = Math.ceil(surface.inset ?? 3) + 1;
            return { x: x - margin, y: y - margin,
                width: width + margin * 2, height: height + margin * 2 };
        }
        return { x, y, width, height };
    }

    _opaqueGpuWindowRects() {
        const rects = [];
        const shell = this.nativeCanvasWindow.snapshot?.shell;
        for (const window of this.windows.values()) {
            if (!window.opaqueGpuFrame || !window.scaledUploadSurface) continue;
            let rect = this._sceneWindowRect(window, this.windows, {
                x: 0, y: 0, width: window.width, height: window.height,
            });
            let current = window;
            const visited = new Set();
            // Only settled, opaque client interiors can hide a backdrop.
            // Clip to every ancestor, and conservatively reject transitions
            // or inactive-window translucency anywhere along the path.
            while (rect && current && current.parent !== current.id) {
                if (visited.has(current.id) || !current.viewable || current.inputOnly ||
                    current.desktopLayer || current.shellLayer ||
                    this.windowTransitions.has(current.id)) { rect = null; break; }
                visited.add(current.id);
                const parent = this.windows.get(current.parent);
                if (!parent || (parent.parent === parent.id &&
                    !current.activeWindow && shell?.inactiveWindowsTranslucent === true)) {
                    rect = null; break;
                }
                const clip = this._sceneWindowRect(parent, this.windows, {
                    x: 0, y: 0, width: parent.width, height: parent.height,
                });
                if (!clip) { rect = null; break; }
                const x = Math.max(rect.x, clip.x), y = Math.max(rect.y, clip.y);
                const right = Math.min(rect.x + rect.width, clip.x + clip.width);
                const bottom = Math.min(rect.y + rect.height, clip.y + clip.height);
                rect = right > x && bottom > y ? { x, y, width: right - x, height: bottom - y } : null;
                current = parent;
            }
            if (rect && current) rects.push(rect);
        }
        return rects;
    }

    _scheduleMediaBlink() {
        if (this.nativeMediaBlinkTimer !== null) return;
        this.nativeMediaBlinkTimer = this.setTimer(() => {
            this.nativeMediaBlinkTimer = null;
            if (this.nativeMediaStates.size === 0) return;
            this.nativeMediaBlinkVisible = !this.nativeMediaBlinkVisible;
            this.damaged = true;
            this.schedule();
            this._scheduleMediaBlink();
        }, 420);
    }

    _advanceOverview() {
        if (this.overviewProgress === this.overviewTarget) return;
        const elapsed = Math.max(0, this.now() - this.overviewStartedAt);
        const distance = Math.abs(
            this.overviewTarget - this.overviewStartedProgress,
        );
        const duration = Math.max(1, 280 * distance);
        const amount = Math.min(1, elapsed / duration);
        this.overviewProgress = this.overviewStartedProgress +
            (this.overviewTarget - this.overviewStartedProgress) * amount;
        if (amount >= 1) this.overviewProgress = this.overviewTarget;
    }

    _collectOverviewItems(root) {
        const x11 = [...this.windows.values()]
            .filter(window =>
                window.id !== root.id &&
                window.parent === root.id &&
                window.viewable &&
                !window.inputOnly &&
                !window.desktopLayer &&
                !window.shellLayer &&
                !window.ownedWindow &&
                (
                    !window.overrideRedirect ||
                    (
                        window.id >= RICEWM_FIRST_MANAGED_FRAME_ID &&
                        window.id < RICEWM_MANAGED_FRAME_ID_LIMIT
                    )
                )
            )
            .sort((left, right) =>
                right.stackIndex - left.stackIndex || right.id - left.id
            )
            .map(window => ({
                kind: "x11",
                id: window.id,
                outputArea: this.nativeCanvasWindow.snapshot?.shell?.scrollingClips?.find(c=>c.id===window.id) || null,
                active: !this.nativeCanvasWindow.foregroundActive && window.activeWindow === true,
                title: this.shellTexts.get(window.id)?.find(overlay=>overlay.role==='window-title')?.text || this.shellTexts.get(window.id)?.find(
                    overlay => overlay.text,
                )?.text || "Window",
                source: {
                    x: window.x - window.borderWidth,
                    y: window.y - window.borderWidth,
                    width: window.width + window.borderWidth * 2,
                    height: window.height + window.borderWidth * 2,
                },
            }));
        const native = this.nativeCanvasWindow.snapshots
            .filter(snapshot => snapshot.role !== "desktop" && snapshot.frame)
            .slice()
            .reverse()
            .map(snapshot => ({
                kind: "native",
                id: Number(snapshot.taskbarId),
                active: this.nativeCanvasWindow.foregroundActive && String(snapshot.app) === String(this.nativeCanvasWindow.activeApp),
                title: String(snapshot.title || snapshot.entrypoint || "Window"),
                source: { ...snapshot.frame },
                outputArea: snapshot.tileClip || null,
            }));
        const windows = [...native, ...x11];
        const query = this.overviewSearchQuery.trim().toLocaleLowerCase();
        if (!query) return windows;
        const matches = item => `${item.title} ${item.subtitle || ""} ${item.path || ""}`
            .toLocaleLowerCase().includes(query);
        const center = {
            x: root.width / 2 - 32,
            y: root.height / 2 - 32,
            width: 64,
            height: 64,
        };
        return [
            ...windows.filter(matches),
            ...this.applicationCatalog.filter(matches),
            ...this.overviewExternalResults.filter(matches),
        ].slice(0, 30).map(item => item.source ? item : {
            ...item,
            source: { ...center },
        });
    }

    _renderOverview(root) {
        const context = this.context;
        const colors=overviewTheme(this.nativeCanvasWindow.snapshot?.chrome);
        if (this.overviewItems === null) {
            this.overviewItems = this._collectOverviewItems(root);
        }
        const topPanel = this.windows.get(RICEWM_GNOME_PANEL_ID);
        const topPanelBottom = topPanel?.viewable
            ? Math.max(0, topPanel.y + topPanel.height)
            : 0;
        const searchArea = this.overviewInteraction?.searchArea(root) || {x:0, y:0, width:root.width};
        const searchY = Math.max(searchArea.y + 22, topPanelBottom + 14);
        const searchWidth = Math.min(620, searchArea.width - 48);
        const searchX = searchArea.x + (searchArea.width - searchWidth) / 2;
        this.overviewSearchRect = this.overviewSearchVisible ? {
            x: searchX,
            y: searchY,
            width: searchWidth,
            height: 46,
        } : null;
        const contentTop = this.overviewSearchVisible ? searchY + 64 : searchY;
        this.overviewLayout = this.overviewInteraction
            ? this.overviewInteraction.layout(this.overviewItems, root, contentTop - searchArea.y)
            : riceOverviewLayout(
            this.overviewItems,
            root.width,
            root.height,
            contentTop,
        );
        this.overviewCloseRects.clear();
        const selectedStillExists = this.overviewLayout.some(({ item }) =>
            this.overviewHover === `${item.kind}:${item.id}`
        );
        if (!selectedStillExists) {
            const activeX11 = this.overviewLayout.find(({ item }) =>
                item.kind === "x11" && item.active
            );
            const activeNative = this.overviewLayout.find(({ item }) =>
                item.kind === "native" && item.active
            );
            // Empty space is a valid pointer selection. Only keyboard navigation
            // and opening/searching the overview should choose a default window.
            const selected = this.overviewPointerActive
                ? null
                : activeX11 || activeNative || this.overviewLayout[0];
            this.overviewHover = selected
                ? `${selected.item.kind}:${selected.item.id}`
                : null;
        }
        const progress = overviewEase(this.overviewProgress);
        context.save();
        context.fillStyle = colors.background(.72 * progress);
        context.fillRect(0, 0, root.width, root.height);
        context.restore();

        if (this.overviewSearchVisible) {
            context.save();
            context.globalAlpha = progress;
            this._roundedRect(context, this.overviewSearchRect, colors.radius);
            context.fillStyle = colors.surface;
            context.fill();
            context.strokeStyle = this.overviewSearchQuery ? colors.accent : colors.line;
            context.stroke();
            context.fillStyle = this.overviewSearchQuery ? colors.ink : colors.muted;
            context.font = '500 16px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
            context.textAlign = "left";
            context.textBaseline = "middle";
            context.fillText(
                this.overviewSearchQuery || "Type to search apps, files, folders, and open windows",
                searchX + 22,
                searchY + 23,
                searchWidth - 44,
            );
            context.restore();
        }

        if (this.overviewLayout.length === 0) {
            if (this.overviewInteraction) {
                this.overviewInteraction.paint(context, root, progress);
                return;
            }
            context.save();
            context.globalAlpha = progress;
            context.fillStyle = colors.ink;
            context.font = '600 18px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
            context.textAlign = "center";
            context.textBaseline = "middle";
            context.fillText(t("No open windows"), root.width / 2, root.height / 2);
            context.restore();
            return;
        }

        for (const entry of this.overviewLayout) {
            const { item, preview } = entry;
            if (this.overviewInteraction?.dim(item)) continue;
            context.save();
            if(entry.clip){context.beginPath();context.rect(entry.clip.x,entry.clip.y,entry.clip.width,entry.clip.height);context.clip();}
            const source = item.source;
            const hovered = this.overviewHover === `${item.kind}:${item.id}`;
            const hoverScale = hovered ? 1 + 0.035 * progress : 1;
            const targetWidth = preview.width * hoverScale;
            const targetHeight = preview.height * hoverScale;
            const targetX = preview.x - (targetWidth - preview.width) / 2;
            const targetY = preview.y - (targetHeight - preview.height) / 2;
            const rect = {
                x: source.x + (targetX - source.x) * progress,
                y: source.y + (targetY - source.y) * progress,
                width: source.width + (targetWidth - source.width) * progress,
                height: source.height + (targetHeight - source.height) * progress,
            };
            context.save();
            context.globalAlpha *= this.overviewInteraction?.sceneOpacity ?? 1;
            context.shadowColor = `rgba(0, 0, 0, ${0.5 * progress})`;
            context.shadowBlur = 30 * progress;
            context.shadowOffsetY = 10 * progress;
            this._roundedRect(context, rect, colors.radius * progress);
            context.fillStyle = colors.surface;
            context.fill();
            context.clip();
            context.shadowBlur = 0;
            context.shadowOffsetY = 0;
            const rendered = item.kind === "native"
                ? this.nativeCanvasWindow.renderTaskPreview(
                    item.id,
                    context,
                    rect,
                )
                : item.kind === "x11"
                    ? this._renderX11TaskPreview(item.id, context, rect)
                    : false;
            if (!rendered) {
                context.fillStyle = colors.control;
                context.fillRect(rect.x, rect.y, rect.width, rect.height);
                context.fillStyle = colors.accent;
                context.font = '700 30px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
                context.textAlign = "center";
                context.textBaseline = "middle";
                context.fillText(
                    item.kind === "application" ? "APP" : item.kind === "folder" ? "DIR" : "FILE",
                    rect.x + rect.width / 2,
                    rect.y + rect.height / 2,
                );
            }
            context.restore();

            if (item.kind === "native" || item.kind === "x11") {
                const size = Math.max(18, Math.min(24, rect.width * 0.12));
                const close = {
                    x: rect.x + rect.width - size - 6,
                    y: rect.y + 6,
                    width: size,
                    height: size,
                };
                if(!entry.clip||(close.x>=entry.clip.x&&close.y>=entry.clip.y&&close.x+close.width<=entry.clip.x+entry.clip.width&&close.y+close.height<=entry.clip.y+entry.clip.height))this.overviewCloseRects.set(`${item.kind}:${item.id}`, close);
                const closeHovered = this.overviewCloseHover ===
                    `${item.kind}:${item.id}`;
                context.save();
                context.globalAlpha = Math.max(0, (progress - 0.12) / 0.88);
                context.beginPath();
                context.arc(
                    close.x + close.width / 2,
                    close.y + close.height / 2,
                    close.width / 2,
                    0,
                    Math.PI * 2,
                );
                context.fillStyle = closeHovered
                    ? colors.accent
                    : hovered
                        ? colors.background(.72)
                        : colors.background(.38);
                context.fill();
                context.strokeStyle = closeHovered
                    ? colors.onAccent
                    : colors.ink;
                context.lineWidth = 1.5;
                const inset = close.width * 0.32;
                context.beginPath();
                context.moveTo(close.x + inset, close.y + inset);
                context.lineTo(
                    close.x + close.width - inset,
                    close.y + close.height - inset,
                );
                context.moveTo(close.x + close.width - inset, close.y + inset);
                context.lineTo(close.x + inset, close.y + close.height - inset);
                context.stroke();
                context.restore();
            }

            context.save();
            context.globalAlpha = Math.max(0, (progress - 0.2) / 0.8);
            context.strokeStyle = hovered ? colors.accent : colors.line;
            context.lineWidth = hovered ? 3 : 1;
            this._roundedRect(context, rect, colors.radius);
            context.stroke();
            context.fillStyle = colors.ink;
            context.font = '600 14px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
            context.textAlign = "center";
            context.textBaseline = "middle";
            context.fillText(
                item.title,
                entry.direct ? preview.x + preview.width / 2 : entry.hit.x + entry.hit.width / 2,
                entry.labelY,
                Math.max(1, (entry.direct ? preview.width : entry.hit.width) - 12),
            );
            context.restore();
            context.restore();
        }
        this.overviewInteraction?.paint(context, root, progress);
    }

    _calendarInk(alpha = 1) {
        const embedded = this.nativeCanvasWindow.snapshot?.shell?.shellStyle === "Embedded";
        const color = embedded ? this.nativeCanvasWindow.snapshot?.chrome?.menu?.[5] ?? 0xffffff : 0xffffff;
        const opacity = alpha >= .48 ? Math.max(.8, alpha) : alpha;
        return `rgba(${color >>> 16 & 255},${color >>> 8 & 255},${color & 255},${opacity})`;
    }
    _calendarPanelPosition(root, width, height) {
        const shell = this.nativeCanvasWindow.snapshot?.shell;
        const edge = this.calendarFromTopBar ? "Top" : shell?.effectiveDockPosition || shell?.dockPosition || "Bottom";
        const attached = shell?.shellStyle === "Embedded";
        const bar = this.windows.get(this.calendarFromTopBar ? RICEWM_GNOME_PANEL_ID : RICEWM_TASKBAR_ID);
        const gap = attached ? 0 : 8;
        let x = this.calendarFromTopBar ? (root.width-width)/2 : root.width-width-8;
        let y = root.height-height-40;
        if (edge === "Top") y = (bar?.y || 0) + (bar?.height || 32) + gap;
        if (edge === "Bottom") y = (bar?.y ?? root.height-32) - height - gap;
        if (edge === "Left") {x = (bar?.x || 0) + (bar?.width || 32) + gap; y = (root.height-height)/2;}
        if (edge === "Right") {x = (bar?.x ?? root.width-32) - width - gap; y = (root.height-height)/2;}
        return {x: Math.max(0,Math.min(root.width-width,x)), y: Math.max(0,Math.min(root.height-height,y)), edge};
    }

    _paintCalendarPanel(root, panel, fallback) {
        const shell = this.nativeCanvasWindow.snapshot?.shell;
        const embedded = shell?.shellStyle === "Embedded";
        const edge = this._calendarPanelPosition(root, panel.width, panel.height).edge;
        const c = this.context;
        if (embedded) this._embeddedMenuJoin({...panel, parent: root.id}, shell, edge);
        c.shadowColor = "rgba(0,0,0,.35)";c.shadowBlur = embedded ? 10 : 30;
        c.beginPath();
        const radii = !embedded ? 15 : edge === "Top" ? [0,0,20,20] : edge === "Bottom" ? [20,20,0,0] : edge === "Left" ? [0,20,20,0] : [20,0,0,20];
        c.roundRect(panel.x,panel.y,panel.width,panel.height,radii);
        c.fillStyle = embedded ? pixelColor(shell.embeddedColor ?? 0x181820) : fallback;
        c.fill();c.shadowBlur = 0;
    }

    _renderCalendar(root) {
        if (root.width >= 640) {
            this._renderGnomeCalendar(root);
            return;
        }
        const context = this.context;
        const width = Math.min(420, root.width - 24);
        const height = Math.min(620, Math.max(354, root.height - 64));
        const taskbar = this.windows.get(RICEWM_TASKBAR_ID);
        const {x, y} = this._calendarPanelPosition(root, width, height);
        const panel = { x, y, width, height };
        const previous = { x: x + 14, y: y + 14, width: 38, height: 34 };
        const next = { x: x + width - 52, y: y + 14, width: 38, height: 34 };
        const agenda = {
            x: x + 12,
            y: y + 365,
            width: width - 24,
            height: Math.max(0, Math.min(142, height - 377)),
        };
        const notifications = {
            x: x + 12,
            y: agenda.y + agenda.height + 28,
            width: width - 24,
            height: Math.max(0, height - agenda.height - 405),
        };
        const notificationRowHeight = 58;
        const visibleNotifications = Math.max(
            0,
            Math.floor(notifications.height / notificationRowHeight),
        );
        const openDay = { x: x + width - 104, y: y + 325, width: 88, height: 30 };
        const dateCells = [];
        const calendarEventRects = [];
        this.calendarLayout = { panel, previous, next, agenda, openDay, dateCells,
            notifications, visibleNotifications, calendarEventRects };
        context.save();
        this._paintCalendarPanel(root, panel, "rgba(24, 31, 43, .98)");
        context.fillStyle = this._calendarInk();
        context.font = '700 17px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        context.textAlign = "center";
        context.textBaseline = "middle";
        context.fillText(
            this.calendarMonth.toLocaleDateString(riceLocale(), { month: "long", year: "numeric" }),
            x + width / 2,
            y + 31,
        );
        context.font = '700 22px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        context.fillText("‹", previous.x + previous.width / 2, previous.y + previous.height / 2);
        context.fillText("›", next.x + next.width / 2, next.y + next.height / 2);
        const weekdays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        context.font = '600 11px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        context.fillStyle = this._calendarInk(.58);
        const cellWidth = (width - 28) / 7;
        for (let day = 0; day < 7; day += 1) {
            context.fillText(weekdays[day], x + 14 + cellWidth * (day + .5), y + 72);
        }
        const year = this.calendarMonth.getFullYear();
        const month = this.calendarMonth.getMonth();
        const firstWeekday = new Date(year, month, 1).getDay();
        const days = new Date(year, month + 1, 0).getDate();
        const today = new Date();
        context.font = '600 13px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        for (let day = 1; day <= days; day += 1) {
            const index = firstWeekday + day - 1;
            const column = index % 7;
            const row = Math.floor(index / 7);
            const centerX = x + 14 + cellWidth * (column + .5);
            const centerY = y + 105 + row * 40;
            const isToday = day === today.getDate() && month === today.getMonth() &&
                year === today.getFullYear();
            const cellDate = new Date(year, month, day);
            const cellRect = { x: centerX - cellWidth / 2, y: centerY - 18,
                width: cellWidth, height: 36 };
            dateCells.push({ rect: cellRect, date: cellDate });
            const selected = this._calendarDateKey(cellDate) ===
                this._calendarDateKey(this.calendarSelectedDate);
            if (selected && !isToday) {
                context.beginPath();
                context.arc(centerX, centerY, 16, 0, Math.PI * 2);
                context.fillStyle = this._calendarInk(.14);
                context.fill();
            }
            if (isToday) {
                context.beginPath();
                context.arc(centerX, centerY, 16, 0, Math.PI * 2);
                context.fillStyle = "#368bd6";
                context.fill();
            }
            context.fillStyle = this._calendarInk();
            context.fillText(String(day), centerX, centerY);
            const dateKey = this._calendarDateKey(cellDate);
            const eventCount = this.calendarModel.events.filter(event =>
                this._calendarEventOccursOn(event, dateKey)
            ).length;
            if (eventCount > 0) {
                context.beginPath();
                context.arc(centerX, centerY + 14, eventCount > 2 ? 2.5 : 2, 0, Math.PI * 2);
                context.fillStyle = "#75b9ff";
                context.fill();
            }
        }
        context.fillStyle = this._calendarInk(.62);
        context.font = '500 12px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        context.fillText(t("Today") + " · " + today.toLocaleDateString(riceLocale(), {
            weekday: "long", month: "long", day: "numeric",
        }), x + width / 2 - 32, y + 340);
        context.fillStyle = "#368bd6";
        this._roundedRect(context, openDay, 7);
        context.fill();
        context.fillStyle = this._calendarInk();
        context.font = '700 12px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        context.fillText(t("Open Day"), openDay.x + openDay.width / 2,
            openDay.y + openDay.height / 2);
        if (agenda.height > 0) {
            const selectedKey = this._calendarDateKey(this.calendarSelectedDate);
            const selectedEvents = this.calendarModel.events
                .filter(event => this._calendarEventOccursOn(event, selectedKey)).slice(0, 3);
            context.textAlign = "left";
            context.fillStyle = this._calendarInk(.82);
            context.font = '700 13px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
            context.fillText(this.calendarSelectedDate.toLocaleDateString(riceLocale(), {
                weekday: "long", month: "short", day: "numeric",
            }), agenda.x + 4, agenda.y + 16);
            if (selectedEvents.length === 0) {
                context.fillStyle = this._calendarInk(.48);
                context.font = '500 12px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
                context.fillText("No events · your time is open", agenda.x + 4, agenda.y + 43);
            }
            selectedEvents.forEach((event, index) => {
                const rowY = agenda.y + 27 + index * 34;
                calendarEventRects.push({
                    id: event.id,
                    date: event.startDate,
                    rect: { x: agenda.x, y: rowY, width: agenda.width, height: 29 },
                });
                context.fillStyle = "rgba(54,139,214,.16)";
                this._roundedRect(context, { x: agenda.x, y: rowY,
                    width: agenda.width, height: 29 }, 7);
                context.fill();
                context.fillStyle = "#75b9ff";
                context.fillRect(agenda.x, rowY + 4, 3, 21);
                context.fillStyle = this._calendarInk(.66);
                context.font = '600 11px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
                context.fillText(
                    this._calendarEventTimeLabel(event, selectedKey),
                    agenda.x + 10,
                    rowY + 18,
                    56,
                );
                context.fillStyle = this._calendarInk();
                context.font = '600 12px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
                context.fillText(event.title, agenda.x + 72, rowY + 18,
                    agenda.width - 80);
            });
        }
        if (notifications.height > 0) {
            context.textAlign = "left";
            context.fillStyle = this._calendarInk(.82);
            context.font = '700 13px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
            context.fillText(
                `Notifications · ${this.notificationHistory.length}/50`,
                x + 16,
                notifications.y - 12,
            );
            context.save();
            context.beginPath();
            context.rect(
                notifications.x,
                notifications.y,
                notifications.width,
                notifications.height,
            );
            context.clip();
            const newestFirst = this.notificationHistory.slice().reverse();
            const shown = newestFirst.slice(
                this.notificationHistoryScroll,
                this.notificationHistoryScroll + visibleNotifications,
            );
            if (shown.length === 0) {
                context.fillStyle = this._calendarInk(.5);
                context.font = '500 13px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
                context.fillText("No notifications yet", notifications.x + 8, notifications.y + 24);
            }
            shown.forEach((entry, index) => {
                const rowY = notifications.y + index * notificationRowHeight;
                context.fillStyle = this._calendarInk(.07);
                this._roundedRect(context, {
                    x: notifications.x,
                    y: rowY + 2,
                    width: notifications.width,
                    height: notificationRowHeight - 6,
                }, 8);
                context.fill();
                context.fillStyle = this._calendarInk();
                context.font = '600 13px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
                context.fillText(entry.title, notifications.x + 10, rowY + 20,
                    notifications.width - 82);
                context.fillStyle = this._calendarInk(.58);
                context.font = '500 11px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
                context.fillText(entry.message, notifications.x + 10, rowY + 39,
                    notifications.width - 20);
                if (entry.timestamp) {
                    context.textAlign = "right";
                    context.fillText(
                        new Date(entry.timestamp).toLocaleTimeString(riceLocale(), {
                            hour: "2-digit", minute: "2-digit",
                        }),
                        notifications.x + notifications.width - 9,
                        rowY + 20,
                    );
                    context.textAlign = "left";
                }
            });
            context.restore();
        }
        context.restore();
    }

    _notificationTextLines(text, maximumWidth, maximumLines = Infinity) {
        const context = this.context;
        const words = String(text || "").trim().split(/\s+/u).filter(Boolean);
        if (words.length === 0) return [];
        const lines = [];
        let line = "";
        for (const word of words) {
            const candidate = line ? `${line} ${word}` : word;
            if (line && context.measureText(candidate).width > maximumWidth) {
                lines.push(line);
                line = word;
            } else {
                line = candidate;
            }
        }
        if (line) lines.push(line);
        if (lines.length <= maximumLines) return lines;
        const shown = lines.slice(0, maximumLines);
        let last = shown[shown.length - 1];
        while (last && context.measureText(`${last}…`).width > maximumWidth) {
            last = Array.from(last).slice(0, -1).join("");
        }
        shown[shown.length - 1] = `${last}…`;
        return shown;
    }

    _notificationTimeLabel(timestamp) {
        const elapsed = Math.max(0, Date.now() - Number(timestamp || 0));
        const minutes = Math.floor(elapsed / 60_000);
        if (minutes < 1) return "Just now";
        if (minutes < 60) return `${minutes} min${minutes === 1 ? "" : "s"} ago`;
        const hours = Math.floor(minutes / 60);
        if (hours < 24) return `${hours} hour${hours === 1 ? "" : "s"} ago`;
        const days = Math.floor(hours / 24);
        return `${days} day${days === 1 ? "" : "s"} ago`;
    }

    _renderGnomeCalendar(root) {
        const context = this.context;
        const calendarInk = this._calendarInk();
        const hasNotifications = this.notificationHistory.length > 0;
        const selectedKey = this._calendarDateKey(this.calendarSelectedDate);
        const selectedEvents = this.calendarModel.events
            .filter(event => this._calendarEventOccursOn(event, selectedKey)).slice(0, 2);
        const year = this.calendarMonth.getFullYear();
        const month = this.calendarMonth.getMonth();
        const firstWeekday = new Date(year, month, 1).getDay();
        const days = new Date(year, month + 1, 0).getDate();
        const calendarRows = Math.ceil((firstWeekday + days) / 7);
        const calendarGridBottom = 148 + (calendarRows - 1) * 35 + 16;
        const openDayOffset = calendarGridBottom + 12;
        const agendaOffset = openDayOffset + 42;
        const agendaHeight = selectedEvents.length === 0
            ? 54
            : 62 + (selectedEvents.length - 1) * 25;
        const calendarContentHeight = agendaOffset + agendaHeight + 12;
        const notificationContentHeight = hasNotifications
            ? 62 + this.notificationHistory.length * 90
            : 0;
        const availableHeight = Math.min(
            1000,
            Math.max(480, root.height - 56),
        );
        const width = Math.min(hasNotifications ? 760 : 360, root.width - 16);
        const height = Math.min(
            availableHeight,
            Math.max(calendarContentHeight, notificationContentHeight),
        );
        const taskbar = this.windows.get(RICEWM_TASKBAR_ID);
        const {x, y} = this._calendarPanelPosition(root, width, height);
        const panel = { x, y, width, height };
        const notificationWidth = hasNotifications
            ? Math.max(350, Math.round(width * 0.56))
            : 0;
        const notificationPane = hasNotifications ? {
            x,
            y,
            width: notificationWidth,
            height,
        } : null;
        const notifications = hasNotifications ? {
            x: x + 12,
            y: y + 10,
            width: notificationWidth - 28,
            height: height - 62,
        } : null;
        const notificationRowHeight = this.notificationHistory.some(entry=>entry.actions?.length || entry.actionLabel) ? 126 : 90;
        const visibleNotifications = hasNotifications
            ? Math.max(1, Math.floor(notifications.height / notificationRowHeight))
            : 0;
        const notificationClear = this.notificationHistory.length > 0
            ? { x: x + 16, y: y + height - 42, width: 72, height: 28 }
            : null;
        const notificationCards = [];
        const notificationDismissRects = [];
        const notificationActionRects = [];
        const track = hasNotifications ? {
            x: x + notificationWidth - 9,
            y: y + 12,
            width: 5,
            height: height - 68,
        } : null;
        const maximumScroll = Math.max(
            0,
            this.notificationHistory.length - visibleNotifications,
        );
        this.notificationHistoryScroll = Math.max(
            0,
            Math.min(maximumScroll, this.notificationHistoryScroll),
        );
        const thumbHeight = maximumScroll > 0
            ? Math.max(32, track.height * visibleNotifications /
                this.notificationHistory.length)
            : track?.height || 0;
        const thumbY = maximumScroll > 0
            ? track.y + (track.height - thumbHeight) *
                this.notificationHistoryScroll / maximumScroll
            : track?.y || 0;
        const notificationScrollbar = hasNotifications ? {
            track,
            thumb: { x: track.x, y: thumbY, width: track.width, height: thumbHeight },
        } : null;

        const rightX = x + notificationWidth;
        const rightWidth = width - notificationWidth;
        const previous = { x: rightX + 14, y: y + 70, width: 34, height: 32 };
        const next = { x: x + width - 48, y: y + 70, width: 34, height: 32 };
        const openDay = {
            x: rightX + rightWidth - 104,
            y: y + openDayOffset,
            width: 88,
            height: 30,
        };
        const agenda = {
            x: rightX + 14,
            y: openDay.y + 42,
            width: rightWidth - 28,
            height: agendaHeight,
        };
        const dateCells = [];
        const calendarEventRects = [];
        this.calendarLayout = {
            panel,
            previous,
            next,
            agenda,
            openDay,
            dateCells,
            notifications,
            notificationPane,
            visibleNotifications,
            notificationCards,
            notificationDismissRects,
            notificationActionRects,
            notificationClear,
            notificationScrollbar,
            calendarEventRects,
        };

        context.save();
        this._paintCalendarPanel(root, panel, "rgba(48, 49, 54, .99)");
        if (hasNotifications) {
            context.fillStyle = this._calendarInk(.10);
            context.fillRect(rightX, y + 12, 1, height - 24);
        }

        if (hasNotifications) {
            context.save();
            context.beginPath();
            context.rect(
                notifications.x,
                notifications.y,
                notifications.width,
                notifications.height,
            );
            context.clip();
            const newestFirst = this.notificationHistory.slice().reverse();
            const shown = newestFirst.slice(
                this.notificationHistoryScroll,
                this.notificationHistoryScroll + visibleNotifications + 1,
            );
            let rowY = notifications.y;
            for (const entry of shown) {
            context.font = '500 13px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
            const allLines = this._notificationTextLines(
                entry.message,
                notifications.width - 38,
            );
            const expanded = entry.id === this.notificationExpandedId;
            const messageLines = expanded
                ? allLines.slice(0, 6)
                : this._notificationTextLines(entry.message, notifications.width - 38, 2);
            const cardHeight = expanded
                ? Math.max(notificationRowHeight, 58 + messageLines.length * 17 + (entry.actions?.length || entry.actionLabel ? 34 : 0))
                : notificationRowHeight;
            const rect = {
                x: notifications.x,
                y: rowY,
                width: notifications.width,
                height: cardHeight - 8,
            };
            notificationCards.push({ id: entry.id, rect });
            context.fillStyle = this._calendarInk(.14);
            this._roundedRect(context, rect, 14);
            context.fill();

            context.textAlign = "left";
            context.textBaseline = "middle";
            context.fillStyle = this._calendarInk(.72);
            context.font = '700 14px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
            context.fillText(entry.title, rect.x + 14, rect.y + 20, rect.width - 142);
            context.fillStyle = this._calendarInk(.50);
            context.font = '500 11px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
            context.fillText(
                this._notificationTimeLabel(entry.timestamp),
                rect.x + Math.min(rect.width - 86, 18 + context.measureText(entry.title).width),
                rect.y + 20,
                70,
            );

            const dismissRect = {
                x: rect.x + rect.width - 34,
                y: rect.y + 8,
                width: 26,
                height: 26,
            };
            notificationDismissRects.push({ id: entry.id, rect: dismissRect });
            context.beginPath();
            context.arc(
                dismissRect.x + dismissRect.width / 2,
                dismissRect.y + dismissRect.height / 2,
                13,
                0,
                Math.PI * 2,
            );
            context.fillStyle = this._calendarInk(.16);
            context.fill();
            context.fillStyle = this._calendarInk(.86);
            context.font = '500 18px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
            context.textAlign = "center";
            context.fillText("×", dismissRect.x + 13, dismissRect.y + 13);

            if (allLines.length > 2) {
                context.beginPath();
                context.arc(rect.x + rect.width - 64, rect.y + 21, 13, 0, Math.PI * 2);
                context.fillStyle = this._calendarInk(.16);
                context.fill();
                context.fillStyle = this._calendarInk(.86);
                context.font = '700 16px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
                context.fillText(expanded ? "⌃" : "⌄", rect.x + rect.width - 64, rect.y + 20);
            }

            context.textAlign = "left";
            context.fillStyle = this._calendarInk(.94);
            context.font = '500 13px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
            messageLines.forEach((line, index) => {
                context.fillText(line, rect.x + 14, rect.y + 48 + index * 17);
            });
            const buttons=[...(entry.actions || []),...(entry.actionLabel?[{label:entry.actionLabel}]:[])];
            if(buttons.length){
                const gap=6,width=(rect.width-28-gap*(buttons.length-1))/buttons.length;
                buttons.forEach((button,index)=>{
                    const actionRect={x:rect.x+14+index*(width+gap),y:rect.y+rect.height-34,width,height:26};
                    notificationActionRects.push({id:entry.id,actionId:button.id,rect:actionRect});
                    context.fillStyle=this._calendarInk(.18);this._roundedRect(context,actionRect,6);context.fill();
                    context.fillStyle=this._calendarInk(.94);context.font='500 11px "RiceWM Sans", system-ui, sans-serif';context.textAlign='center';
                    context.fillText(button.label,actionRect.x+width/2,actionRect.y+13,width-10);
                });
            }
                rowY += cardHeight;
                if (rowY >= notifications.y + notifications.height) break;
            }
            context.restore();
        }

        if (maximumScroll > 0) {
            context.fillStyle = this._calendarInk(.12);
            this._roundedRect(context, track, 3);
            context.fill();
            context.fillStyle = this._calendarInk(.48);
            this._roundedRect(context, notificationScrollbar.thumb, 3);
            context.fill();
        }
        if (notificationClear) {
            context.fillStyle = this._calendarInk(.14);
            this._roundedRect(context, notificationClear, 14);
            context.fill();
            context.fillStyle = calendarInk;
            context.font = '700 12px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
            context.textAlign = "center";
            context.textBaseline = "middle";
            context.fillText(
                t("Clear"),
                notificationClear.x + notificationClear.width / 2,
                notificationClear.y + notificationClear.height / 2,
            );
        }

        const selected = this.calendarSelectedDate;
        context.textAlign = "left";
        context.textBaseline = "middle";
        context.fillStyle = this._calendarInk(.52);
        context.font = '700 13px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        context.fillText(
            selected.toLocaleDateString(riceLocale(), { weekday: "long" }),
            rightX + 24,
            y + 26,
        );
        context.fillStyle = this._calendarInk(.70);
        context.font = '700 20px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        context.fillText(
            selected.toLocaleDateString(riceLocale(), {
                month: "long", day: "numeric", year: "numeric",
            }),
            rightX + 24,
            y + 49,
            rightWidth - 42,
        );
        context.textAlign = "center";
        context.fillStyle = calendarInk;
        context.font = '700 15px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        context.fillText(
            this.calendarMonth.toLocaleDateString(riceLocale(), { month: "long" }),
            rightX + rightWidth / 2,
            y + 86,
        );
        context.font = '700 22px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        context.fillText("‹", previous.x + previous.width / 2, previous.y + previous.height / 2);
        context.fillText("›", next.x + next.width / 2, next.y + next.height / 2);

        const weekdays = ["S", "M", "T", "W", "T", "F", "S"];
        const cellWidth = (rightWidth - 28) / 7;
        context.font = '600 11px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        context.fillStyle = this._calendarInk(.52);
        weekdays.forEach((day, index) => {
            context.fillText(day, rightX + 14 + cellWidth * (index + 0.5), y + 119);
        });
        const today = new Date();
        context.font = '600 12px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        for (let day = 1; day <= days; day += 1) {
            const index = firstWeekday + day - 1;
            const column = index % 7;
            const row = Math.floor(index / 7);
            const centerX = rightX + 14 + cellWidth * (column + 0.5);
            const centerY = y + 148 + row * 35;
            const cellDate = new Date(year, month, day);
            dateCells.push({
                rect: { x: centerX - cellWidth / 2, y: centerY - 15,
                    width: cellWidth, height: 31 },
                date: cellDate,
            });
            const isToday = this._calendarDateKey(cellDate) === this._calendarDateKey(today);
            const isSelected = this._calendarDateKey(cellDate) ===
                this._calendarDateKey(this.calendarSelectedDate);
            if (isToday || isSelected) {
                context.beginPath();
                context.arc(centerX, centerY, 15, 0, Math.PI * 2);
                context.fillStyle = isToday ? "#e84716" : this._calendarInk(.15);
                context.fill();
            }
            context.fillStyle = calendarInk;
            context.fillText(String(day), centerX, centerY);
            if (this.calendarModel.events.some(event =>
                this._calendarEventOccursOn(event, this._calendarDateKey(cellDate))
            )) {
                context.beginPath();
                context.arc(centerX, centerY + 13, 2, 0, Math.PI * 2);
                context.fillStyle = "#75b9ff";
                context.fill();
            }
        }

        context.fillStyle = "#368bd6";
        this._roundedRect(context, openDay, 7);
        context.fill();
        context.fillStyle = calendarInk;
        context.font = '700 12px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        context.fillText(t("Open Day"), openDay.x + openDay.width / 2,
            openDay.y + openDay.height / 2);

        context.fillStyle = this._calendarInk(.10);
        this._roundedRect(context, agenda, 12);
        context.fill();
        context.textAlign = "left";
        context.fillStyle = this._calendarInk(.58);
        context.font = '700 13px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        context.fillText(t("Today"), agenda.x + 14, agenda.y + 20);
        if (selectedEvents.length === 0) {
            context.fillStyle = this._calendarInk(.48);
            context.font = '500 12px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
            context.fillText("No events", agenda.x + 14, agenda.y + 44);
        } else {
            selectedEvents.forEach((event, index) => {
                const eventRect = {
                    x: agenda.x + 8,
                    y: agenda.y + 31 + index * 25,
                    width: agenda.width - 16,
                    height: 23,
                };
                calendarEventRects.push({
                    id: event.id,
                    date: event.startDate,
                    rect: eventRect,
                });
                context.fillStyle = this._calendarInk(.08);
                this._roundedRect(context, eventRect, 6);
                context.fill();
                context.fillStyle = calendarInk;
                context.font = '600 12px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
                context.fillText(
                    `${this._calendarEventTimeLabel(event, selectedKey)}  ${event.title}`,
                    agenda.x + 14,
                    agenda.y + 44 + index * 25,
                    agenda.width - 28,
                );
            });
        }
        context.restore();
    }

    _nativeMediaSurface(request) {
        // Decoder frames are leased and can change between layout snapshots.
        // Resolve the current frame at paint time rather than retaining it in
        // the immutable window command list.
        const live = this.nativeMediaSurfaceProvider?.(request);
        if (live) return live;
        const key = String(request);
        const surface = this.nativeMediaSurfaces.get(key) || null;
        if (!surface && this.nativeMediaHotRequests.has(key)) {
            this._queueNativeMediaRefresh(key);
        }
        return surface;
    }

    _queueNativeMediaRefresh(key) {
        if (
            this.nativeMediaRefreshAttempts.get(key) ===
                this.nativeMediaRefreshGeneration
        ) return;
        this.nativeMediaRefreshAttempts.set(
            key,
            this.nativeMediaRefreshGeneration,
        );
        this.nativeMediaRefreshQueue.add(key);
        if (this.nativeMediaRefreshScheduled) return;
        this.nativeMediaRefreshScheduled = true;
        queueMicrotask(() => {
            this.nativeMediaRefreshScheduled = false;
            const requests = [...this.nativeMediaRefreshQueue].filter(key =>
                this.nativeMediaHotRequests.has(key) &&
                !this.nativeMediaSurfaces.has(key)
            );
            this.nativeMediaRefreshQueue.clear();
            if (requests.length === 0) return;
            try {
                this.onNativeMediaMissing(Object.freeze(requests));
            } catch (error) {
                this.onFault(error);
            }
        });
    }

    _reconcileNativeMedia(snapshot) {
        const { all: next, hot } = nativeMediaRequests(snapshot);
        const previous = this.nativeMediaLiveRequests;
        const previousHot = this.nativeMediaHotRequests;
        const changed = !equalSets(previous, next);
        const hotChanged = !equalSets(previousHot, hot);
        if (hotChanged) this.nativeMediaRefreshGeneration += 1;
        for (const key of previous) {
            if (next.has(key)) continue;
            this.nativeMediaRetiredRequests.add(key);
            this.nativeMediaRefreshAttempts.delete(key);
            this.nativeMediaRefreshQueue.delete(key);
        }
        while (
            this.nativeMediaRetiredRequests.size >
                MAX_NATIVE_MEDIA_REQUEST_TRACKING
        ) {
            this.nativeMediaRetiredRequests.delete(
                this.nativeMediaRetiredRequests.values().next().value,
            );
        }
        for (const key of next) this.nativeMediaRetiredRequests.delete(key);
        this.nativeMediaLiveRequests = next;
        this.nativeMediaHotRequests = hot;
        for (const key of this.nativeMediaSurfaces.keys()) {
            // The bitmap message can arrive before its scene snapshot. Keep a
            // bounded pending panorama until first use; retire old scenes normally.
            if (!hot.has(key) && (!this.nativeMediaWallpaperRequests.has(key) || previous.has(key))) this._evictNativeMediaSurface(key);
        }
        for (const key of this.nativeMediaStates.keys()) {
            if (!hot.has(key)) this.nativeMediaStates.delete(key);
        }
        if (this.nativeMediaStates.size === 0) this._stopMediaBlink();
        if (changed) {
            try {
                this.onNativeMediaRequestsChanged(Object.freeze([...next]));
            } catch (error) {
                this.onFault(error);
            }
        }
    }

    _evictNativeMediaSurface(key) {
        const surface = this.nativeMediaSurfaces.get(key);
        const bytes = this.nativeMediaSurfaceBytes.get(key) || 0;
        this.nativeMediaSurfaces.delete(key);
        this.nativeMediaWallpaperRequests.delete(key);
        this.nativeMediaSurfaceBytes.delete(key);
        this.nativeMediaBytes = Math.max(0, this.nativeMediaBytes - bytes);
        surface?.close?.();
    }

    _stopMediaBlink() {
        this.nativeMediaBlinkVisible = true;
        if (this.nativeMediaBlinkTimer !== null) {
            this.clearTimer(this.nativeMediaBlinkTimer);
            this.nativeMediaBlinkTimer = null;
        }
    }

    _renderEmbeddedWindowJoins(root) {
        const snapshot = this.nativeCanvasWindow.snapshot;
        const shell = snapshot?.shell;
        if (shell?.shellStyle !== "Embedded") return;
        // Only surfaces touching a shell edge attach; ordinary floating apps
        // retain their own silhouette and input bounds.
        const dockEdge = shell.effectiveDockPosition || shell.dockPosition || "Bottom";
        const edges = new Set(shell.gnomePanelVisible ? ["Top"] : []);
        if (shell.dockVisible && !shell.dockAutoHide && shell.dockStyle !== "Compact dock") edges.add(dockEdge);
        for (const native of this.nativeCanvasWindow.snapshots || []) {
            if (native.role === "desktop" || native.dialog || native.minimized || native.layerVisible === false || !native.frame) continue;
            const f = native.frame;
            for (const edge of edges) {
                const distance = edge === "Top" ? f.y : edge === "Left" ? f.x : edge === "Right" ? root.width-f.x-f.width : root.height-f.y-f.height;
                if (distance >= 0 && distance <= 32) this._embeddedMenuJoin({...f, parent: root.id}, shell, edge);
            }
        }
    }

    _renderEmbeddedFrame(root) {
        const shell = this.nativeCanvasWindow.snapshot?.shell;
        if (shell?.shellStyle !== "Embedded") return;
        const context = this.context;
        const edge = shell.effectiveDockPosition || shell.dockPosition || "Bottom";
        const attached = shell.dockVisible && !shell.dockAutoHide && shell.dockStyle !== "Compact dock";
        if (!attached && !shell.gnomePanelVisible) return;
        const corners = [0, 0, 0, 0];
        const bounds = {x: 0, y: 0, width: root.width, height: root.height};
        if (attached) {
            if (edge === "Left") { bounds.x += 32; bounds.width -= 32; corners[0] = corners[3] = 22; }
            if (edge === "Right") { bounds.width -= 32; corners[1] = corners[2] = 22; }
            if (edge === "Top") { bounds.y += 32; bounds.height -= 32; corners[0] = corners[1] = 22; }
            if (edge === "Bottom") { bounds.height -= 32; corners[2] = corners[3] = 22; }
        }
        if (shell.gnomePanelVisible) { bounds.y += 32; bounds.height -= 32; corners[0] = corners[1] = 22; }
        context.save();
        context.fillStyle = pixelColor(shell.embeddedColor ?? 0x181820);
        context.beginPath(); context.rect(0,0,root.width,root.height);
        context.roundRect(bounds.x,bounds.y,Math.max(0,bounds.width),Math.max(0,bounds.height),corners);
        context.fill("evenodd");
        context.restore();
    }

    _embeddedMenuBounds(window) {
        // Cascading menus allocate a tall transparent surface so submenus can
        // grow without moving the anchor. Round the painted rows, not that allocation.
        const rows = (this.shellTexts.get(window.id) || []).filter(row => !row.text.startsWith("__"));
        if (!rows.length) return window;
        const top = Math.max(0, Math.min(...rows.map(row => row.y)) - 3);
        const bottom = Math.min(window.height, Math.max(...rows.map(row => row.y + row.height)) + 4);
        return {...window, y: window.y + top, height: Math.max(1, bottom - top)};
    }

    _embeddedMenuJoin(window, shell, attachment = null) {
        if (shell?.shellStyle !== "Embedded" || (!attachment && window.id !== RICEWM_MENU_ID)) return;
        if (!attachment) window = this._embeddedMenuBounds(window);
        const c = this.context, r = 20;
        const edge = attachment || shell.effectiveDockPosition || shell.dockPosition || "Bottom";
        const root = this.windows.get(window.parent);
        if (!root) return;
        const color = pixelColor(shell.embeddedColor ?? 0x181820);
        c.save(); c.fillStyle = color;
        // Concave shoulders merge the panel into the screen border. The
        // menu's compositor clip and native hit rectangle remain aligned.
        if (edge === "Bottom" || edge === "Top") {
            const top = edge === "Top";
            const y = top ? 0 : window.y + window.height;
            c.fillRect(window.x,y,window.width,top ? window.y : root.height-y);
            const anchor = top ? window.y : window.y+window.height;
            for (const side of [-1,1]) {
                const x = side < 0 ? window.x : window.x+window.width;
                const dy = top ? r : -r;
                c.beginPath();c.moveTo(x,anchor);c.lineTo(x+side*r,anchor);
                c.quadraticCurveTo(x,anchor,x,anchor+dy);c.closePath();c.fill();
            }
        } else {
            const left = edge === "Left", x = left ? 0 : window.x+window.width;
            c.fillRect(x,window.y,left ? window.x : root.width-x,window.height);
            const anchor = left ? window.x : window.x+window.width;
            for (const side of [-1,1]) {
                const y = side < 0 ? window.y : window.y+window.height;
                c.beginPath();c.moveTo(anchor,y);c.lineTo(anchor,y+side*r);
                c.quadraticCurveTo(anchor,y,anchor+(left?r:-r),y);c.closePath();c.fill();
            }
        }
        c.restore();
    }

    _renderChildren(parent, children, ancestors, include = () => true) {
        if (ancestors.has(parent.id)) return;
        ancestors.add(parent.id);
        const context = this.context;
        for (const window of children.get(parent.id) || []) {
            if (!window.viewable || window.inputOnly || !include(window)) {
                continue;
            }
            if (window.id >= 0xe0000100 && window.id < 0xe0000120) {
                // Render each output's real retained panel through the same
                // icon/material path as its interactive canonical surface.
                const sourceId = (window.id - 0xe0000100) % 2 ? RICEWM_TASKBAR_ID : RICEWM_GNOME_PANEL_ID;
                const canonical = this.windows.get(sourceId), texts = this.shellTexts.get(sourceId);
                const topbar = this.topbarController;
                const interaction = topbar && {hover:topbar.hover,press:topbar.press,drag:topbar.drag};
                this.windows.set(sourceId, {...window,id:sourceId});
                this.shellTexts.set(sourceId,this.shellTexts.get(window.id)||[]);
                if(topbar)Object.assign(topbar,{hover:null,press:null,drag:null});
                try {
                    const nested = new Set(ancestors); nested.delete(parent.id);
                    this._renderChildren(parent,new Map([[parent.id,[this.windows.get(sourceId)]]]),nested);
                } finally {
                    if(canonical)this.windows.set(sourceId,canonical);else this.windows.delete(sourceId);
                    if(texts)this.shellTexts.set(sourceId,texts);else this.shellTexts.delete(sourceId);
                    if(topbar){Object.assign(topbar,interaction);topbar.refresh();}
                }
                continue;
            }
            context.save();
            const tileClip=this.nativeCanvasWindow.snapshot?.shell?.scrollingClips?.find(c=>c.id===window.id);
            if(tileClip){context.beginPath();context.rect(tileClip.x,tileClip.y,tileClip.width,tileClip.height);context.clip();}
            if (!this._applyWindowTransition(window)) {
                context.restore();
                continue;
            }
            const applicationWindow = parent.parent === parent.id &&
                !window.desktopLayer && !window.shellLayer &&
                !window.overrideRedirect;
            const shell = this.nativeCanvasWindow.snapshot?.shell;
            if(applicationWindow&&shell?.scrollingTiling) {
                const motion=this.windowTransitions.get(window.id);
                const duration=Math.max(1,shell.motion?.definitions?.['window.layout']?.duration??320);
                const remaining=motion?.direction==='layout'?(1-Math.max(0,Math.min(1,(this.now()-motion.startedAt)/duration)))**4:0;
                const left=window.x+(motion?.direction==='layout'?(motion.from.x-motion.to.x)*remaining:0);
                const right=left+window.width;
                if(right < (tileClip?.x??0)-32 || left > (tileClip?tileClip.x+tileClip.width:parent.width)+32) {context.restore();continue;}
            }
            const inactive = applicationWindow && !window.activeWindow &&
                shell?.inactiveWindowsTranslucent === true;
            const menuSurface = parent.parent === parent.id &&
                (window.id === RICEWM_MENU_ID || window.overrideRedirect);
            if (inactive) context.globalAlpha *= 0.88;
            this._embeddedMenuJoin(window, shell);
            if (window.id === RICEWM_MENU_ID && shell?.shellStyle === "Embedded") {
                const edge = shell.effectiveDockPosition || shell.dockPosition || "Bottom";
                const radii = edge === "Bottom" ? [20,20,0,0] : edge === "Top" ? [0,0,20,20] : edge === "Left" ? [0,20,20,0] : [20,0,0,20];
                const bounds = this._embeddedMenuBounds(window);
                context.beginPath(); context.roundRect(bounds.x,bounds.y,bounds.width,bounds.height,radii); context.clip();
            }
            const transparentTopbar = window.id === RICEWM_GNOME_PANEL_ID && this.topbarController?.transparent;
            if (!transparentTopbar) this._renderBackdropMaterial(window, parent);
            if (menuSurface && shell?.menuShadow === true) {
                this._renderWindowShadow(window, parent, true);
            } else if (
                applicationWindow &&
                window.activeWindow &&
                !window.maximizedWindow &&
                shell?.activeWindowShadow === true
            ) {
                this._renderWindowShadow(window, parent);
            }
            context.translate(window.x, window.y);
            if (window.id === RICEWM_MENU_ID && shell?.shellStyle === "Embedded") {
                const edge = shell.effectiveDockPosition || shell.dockPosition || "Bottom";
                const radii = edge === "Bottom" ? [20,20,0,0] : edge === "Top" ? [0,0,20,20] : edge === "Left" ? [0,20,20,0] : [20,0,0,20];
                const bounds = this._embeddedMenuBounds(window);
                context.beginPath(); context.roundRect(0,bounds.y-window.y,bounds.width,bounds.height,radii); context.clip();
            }
            if (this._wedgedMenu(window, shell)) {
                context.beginPath();
                appendWedgedSurfacePath(context, {
                    x: 0,
                    y: 0,
                    width: window.width,
                    height: window.height,
                });
                context.clip();
            }
            if (window.id === RICEWM_TASK_PREVIEW_ID) {
                this._renderTaskPreview(window);
                context.restore();
                continue;
            }
            if (window.id === RICEWM_NOTIFICATION_ID) {
                if (this.nativeCanvasWindow.snapshot?.chrome?.chromeProfile === "orchard") {
                    this._renderOrchardNotification(window);
                } else {
                    this._renderMatrixNotification(window);
                }
                context.restore();
                continue;
            }
            if (window.borderWidth > 0) {
                context.fillStyle = "#64748b";
                context.fillRect(
                    -window.borderWidth,
                    -window.borderWidth,
                    window.width + window.borderWidth * 2,
                    window.height + window.borderWidth * 2,
                );
            }
            if (transparentTopbar) {
                this._renderTransparentTopbarSurface(window);
            } else if (window.surface) {
                this._renderWindowSurface(window, shell);
            } else {
                context.fillStyle = pixelColor(window.backgroundPixel);
                context.fillRect(0, 0, window.width, window.height);
            }
            if (inactive) {
                context.fillStyle = "rgba(255, 255, 255, 0.055)";
                context.fillRect(0, 0, window.width, window.height);
            }
            if(window.shellLayer&&window.id!==RICEWM_MENU_ID&&window.id!==RICEWM_NOTIFICATION_ID){
                this.customEffects.render(context,{width:parent.width,height:parent.height,target:'shell',
                    rects:[{x:0,y:0,width:window.width,height:window.height}],reducedMotion:shell?.motionEnabled===false});
            }
            const themedChrome=this.nativeCanvasWindow.snapshot?.chrome;
            this.themeSkins ||= new RiceThemeSkinRenderer(()=>{this.damaged=true;this.schedule();});
            const role=window.shellLayer&&![RICEWM_MENU_ID,RICEWM_NOTIFICATION_ID].includes(window.id)?'dock':null;
            if(role)this.themeSkins.paint(context,{x:0,y:0,width:window.width,height:window.height},themedChrome,role);
            this._renderShellText(window);
            // A dock menu caret extends outside its surface; other chrome is
            // clipped to its owner and painted before higher stacked menus.
            if (window.id === RICEWM_MENU_ID && parent.parent === parent.id) {
                context.save();
                context.translate(-window.x, -window.y);
                this._renderNativeChrome(parent, window.id);
                context.restore();
            }
            context.save();
            context.beginPath();
            context.rect(0, 0, window.width, window.height);
            context.clip();
            if (parent.parent === parent.id && window.shellLayer && window.id !== RICEWM_MENU_ID) {
                context.save();
                context.translate(-window.x, -window.y);
                this._renderNativeChrome(parent, window.id);
                context.restore();
            }
            this._renderChildren(window, children, ancestors, include);
            context.restore();
            // Managed X11 clients use the same theme-bundle frame artwork as
            // Rice windows. Paint after the client so its backing pixmap cannot
            // erase the corners. Client-side decorated windows retain ownership
            // of their frame (their child starts at y=0 or the border inset).
            if (parent.parent === parent.id && !window.shellLayer &&
                window.id >= RICEWM_FIRST_MANAGED_FRAME_ID &&
                window.id < RICEWM_MANAGED_FRAME_ID_LIMIT &&
                (children.get(window.id) || []).some(child =>
                    !child.inputOnly && child.viewable && child.y > child.x
                )) {
                // The client already occupies the inner frame. Expand the
                // decoration bounds by the theme inset, keeping the corner
                // brackets outside that frame instead of over its pixels.
                const inset = shell?.surface?.inset ?? 3;
                this.nativeCanvasWindow.renderSurfaceFrame({ shell }, {
                    x: -inset, y: -inset,
                    width: window.width + inset * 2,
                    height: window.height + inset * 2,
                });
            }
            context.restore();
        }
        ancestors.delete(parent.id);
    }

    _applyWindowTransition(window) {
        const transition = this.windowTransitions.get(window.id);
        if (transition === undefined) return true;
        const startedAt = typeof transition === "number"
            ? transition
            : transition.startedAt;
        const direction = typeof transition === "number"
            ? "open"
            : transition.direction || "open";
        const shell = this.nativeCanvasWindow.snapshot?.shell;
        if (shell?.motionEnabled !== true) {
            this.windowTransitions.delete(window.id);
            this.closingSceneWindows.delete(window.id);
            return direction !== "close";
        }
        if(direction==='layout') {
            const duration=Math.max(1,shell.motion?.definitions?.['window.layout']?.duration??320);
            const t=Math.max(0,Math.min(1,(this.now()-startedAt)/duration));
            if(t>=1||!shell.scrollingTiling){this.windowTransitions.delete(window.id);return true;}
            const remaining=(1-t)**4;
            this.context.translate((transition.from.x-transition.to.x)*remaining,(transition.from.y-transition.to.y)*remaining);
            this.damaged=true;this.schedule();return true;
        }
        const menu = window.id === RICEWM_MENU_ID || window.overrideRedirect;
        const foldout = !window.overrideRedirect &&
            shell?.motion?.window === "Foldout";
        const duration = riceWindowTransitionDuration({
            speed: shell?.motion?.speed,
            kind: menu ? "menu" : "window",
            direction,
            foldout,
        });
        const progress = Math.max(0, Math.min(1, (this.now() - startedAt) / duration));
        if (progress >= 1) {
            this.windowTransitions.delete(window.id);
            this.closingSceneWindows.delete(window.id);
            return direction !== "close";
        }
        this.damaged = true;
        this.schedule();
        const motionProgress = direction === "close" ? 1 - progress : progress;
        const remaining = 1 - motionProgress;
        const ease = direction === "close"
            ? motionProgress * motionProgress
            : 1 - remaining * remaining * remaining;
        if (window.id === RICEWM_NOTIFICATION_ID) {
            const notificationProgress = Math.max(
                0,
                Math.min(1, (this.now() - startedAt) / 160),
            );
            if (notificationProgress >= 1) {
                this.windowTransitions.delete(window.id);
                return true;
            }
            const notificationEase = 1 - Math.pow(1 - notificationProgress, 3);
            const scale = .98 + .02 * notificationEase;
            const centerX = window.x + window.width / 2;
            const centerY = window.y + window.height / 2;
            this.context.translate(centerX, centerY - 8 * (1 - notificationEase));
            this.context.scale(scale, scale);
            this.context.translate(-centerX, -centerY);
            return true;
        }
        if (window.id === RICEWM_MENU_ID && shell?.shellStyle === "Embedded") {
            const edge = shell.effectiveDockPosition || shell.dockPosition || "Bottom";
            const vertical = edge === "Left" || edge === "Right";
            const x = edge === "Right" ? window.x + window.width : window.x;
            const y = edge === "Bottom" ? window.y + window.height : window.y;
            this.context.translate(x,y);
            this.context.scale(vertical ? Math.max(.001,ease) : 1, vertical ? 1 : Math.max(.001,ease));
            this.context.translate(-x,-y);
            return true;
        }
        if (window.id === RICEWM_MENU_ID) {
            // The launcher changes width as hover opens and closes cascade
            // panes. Retiring its transition immediately keeps those hover
            // updates stable instead of treating them as another menu open.
            this.windowTransitions.delete(window.id);
            return true;
        }
        const style = menu
            ? shell?.motion?.menu || "Spring Expand"
            : shell?.motion?.window || "Spring";
        if (style === "None") {
            this.windowTransitions.delete(window.id);
            this.closingSceneWindows.delete(window.id);
            return direction !== "close";
        }
        if (style === "Foldout" && !menu) {
            const frameProgress = direction === "close"
                ? motionProgress
                : Math.min(1, motionProgress / .72);
            const frame = frameProgress * frameProgress * (3 - 2 * frameProgress);
            const origin = riceWindowTransitionOrigin(window, false);
            this.context.translate(origin.x, origin.y);
            this.context.scale(.055 + .945 * frame, .025 + .975 * frame);
            this.context.translate(-origin.x, -origin.y);
            if (direction === "close") {
                this.context.globalAlpha *= Math.min(1, motionProgress * 3.2);
            }
            return true;
        }
        const spring = direction === "close" || shell?.motion?.elastic === false
            ? ease
            : Math.max(0, Math.min(
                1.075,
                1 - Math.exp(-7.5 * motionProgress) *
                    Math.cos(10.5 * motionProgress),
            ));
        const value = style.includes("Spring") ? spring : ease;
        const origin = riceWindowTransitionOrigin(window, menu);
        let scaleX = 1;
        let scaleY = 1;
        let translateX = 0;
        let translateY = 0;
        if (style === "Slide") translateX = 34 * (1 - value);
        else if (style === "Glide") translateY = 28 * (1 - value);
        else if (style === "Bloom") scaleX = scaleY = .86 + .14 * value;
        else if (menu) {
            scaleX = .96 + .04 * value;
            scaleY = .72 + .28 * value;
        } else {
            scaleX = scaleY = .88 + .12 * value;
            translateY = 18 * (1 - value);
        }
        this.context.translate(origin.x, origin.y);
        this.context.translate(translateX, translateY);
        this.context.scale(scaleX, scaleY);
        this.context.translate(-origin.x, -origin.y);
        this.context.globalAlpha *= Math.min(1, ease * 1.4);
        return true;
    }

    _backdropDamage(damage) {
        if (!Array.isArray(damage) || damage.length === 0) return damage;
        const nativeRenderer = this.nativeCanvasWindow;
        const root = [...this.windows.values()].find(window=>window.parent===window.id);
        const panes = nativeRenderer.snapshots.filter(native=>native.role!=='desktop' && native.layerVisible!==false);
        const desktopKey = root && `${root.width}:${root.height}:${root.backgroundPixel}:${this.sceneBackdropRevision || 0}`;
        // Retained, disjoint CPU glass reads a separate desktop underlay, not
        // the previous composed frame. Content-only damage can therefore stay
        // regional: animated images/cursors must not repaint the whole desktop.
        const retainedTileDamage = nativeRenderer.tileBackdrop && desktopKey === nativeRenderer.desktopBackdropKey &&
            nativeRenderer.tileBackdropKey === JSON.stringify([desktopKey,nativeRenderer.snapshots.filter(native=>native.role==='desktop')],
                (_key,value)=>typeof value==='bigint'?String(value):value) &&
            !this.gnomeEffectRenderer.effects?.length && !this.widgetHost?.state?.instances?.some(item=>item.visible) &&
            !nativeRenderer.wallpaper.flight && !nativeRenderer.dragVisual &&
            panes.every((native,index)=>native.chrome?.frostedIce?.model==='hyprglass' &&
                panes.slice(0,index).every(other=>native.frame.x>=other.frame.x+other.frame.width || other.frame.x>=native.frame.x+native.frame.width ||
                    native.frame.y>=other.frame.y+other.frame.height || other.frame.y>=native.frame.y+native.frame.height)) &&
            damage.every(rect=>panes.some(({frame})=>rect.x>=frame.x && rect.y>=frame.y+frame.titleHeight &&
                rect.x+rect.width<=frame.x+frame.width && rect.y+rect.height<=frame.y+frame.height));
        // QML widgets sample the desktop before their own surfaces are painted.
        // Regional clips otherwise leave old widgets/windows in that sample.
        const widgetState = this.widgetHost?.state;
        if (widgetState?.instances.some(instance => instance.visible &&
            widgetState.packages.some(pkg => pkg.id === instance.package && pkg.schema === 'plasma.applet/v1'))) return null;
        // Native glass captures the lower composition before painting its
        // window. Outside a regional clip that canvas still contains the
        // previous complete window, including its sidebar and titlebar.
        // Reconstruct the lower planes before any such capture; otherwise
        // hover and drag updates feed old window pixels back into the glass.
        for (const native of this.nativeCanvasWindow.snapshots) {
            if (native.layerVisible === false) continue;
            // Popup shell material samples beyond the menu's own damage.
            if (native.popup?.rect) return null;
            // A new/resized translucent terminal needs its complete lower
            // plane once before subsequent frames can update a small tile.
            for (const command of native.commands || []) {
                const portal = command.type === 'terminal-portal';
                const translucentGrid = command.type === 'terminal-grid' &&
                    !retainedTileDamage &&
                    // Normal grids composite directly into the damaged region.
                    // The render-time flag is cleared between frames, so it
                    // cannot tell us whether a retained backdrop is needed.
                    Boolean(native.dialog) && native.commands.some(
                        item => item.type === 'terminal-row' && item.row?.transparency > 0);
                if (!portal && !translucentGrid) continue;
                const backdrop = this.nativeCanvasWindow.terminalBackdrops.get(
                    `${String(native.app)}:${String(command.session)}`);
                if (!backdrop || backdrop.width !== Math.max(1, Number(command.rect?.width) || 1) ||
                    backdrop.height !== Math.max(1, Number(command.rect?.height) || 1)) return null;
            }
            const hasGlass = native.chrome?.glassTitlebar === true ||
                native.chrome?.material === "frosted-ice" ||
                native.shell?.frostedIce?.enabled === true ||
                [...(native.commands || []), ...(native.popupCommands || [])]
                    .some(command => command.type === "glass");
            if (hasGlass && (!retainedTileDamage || [...(native.commands || []),...(native.popupCommands || [])].some(command=>command.type==='glass'))) return null;
        }
        // A blur reads beyond its output bounds. Reconstruct the lower planes
        // before sampling: retained pixels outside a regional clip still hold
        // last frame's menus/windows and would feed them back into the glass.
        for (const window of this.windows.values()) {
            if (!window.viewable || !window.backdropMaterial ||
                !window.materialAlpha || !window.surface) continue;
            const parent = this.windows.get(window.parent);
            if (!parent || parent.parent !== parent.id) continue;
            const padding = 72; // Three standard deviations of the 24px blur.
            if (damage.some(rect =>
                rect.x < window.x + window.width + padding &&
                rect.x + rect.width > window.x - padding &&
                rect.y < window.y + window.height + padding &&
                rect.y + rect.height > window.y - padding
            )) return null;
        }
        return damage;
    }

    _renderBackdropMaterial(window, parent) {
        if (
            !window.backdropMaterial ||
            !window.materialAlpha ||
            !window.surface ||
            parent.parent !== parent.id
        ) {
            return;
        }
        // The material is clipped to this root-level window. If none of its
        // output pixels are damaged, do not issue a desktop-sized filtered
        // readback merely because an unrelated client submitted another frame.
        // Full invalidation (resize, uncover, lighting) still refreshes it.
        const damage = this.nativeCanvasWindow.paintDamageRects;
        if (!(this.overviewProgress > 0) && !this.windowTransitions?.has(window.id) &&
            Array.isArray(damage) && !damage.some(rect =>
            rect.x < window.x + window.width &&
            rect.x + rect.width > window.x &&
            rect.y < window.y + window.height &&
            rect.y + rect.height > window.y
        )) return;
        const opticalProfile = this.nativeCanvasWindow.snapshot?.shell?.frostedIce;
        if (opticalProfile?.enabled && opticalProfile.model === 'hyprglass' &&
            (opticalProfile.system || (window.id === RICEWM_TASKBAR_ID && opticalProfile.panels) ||
             (window.id === RICEWM_MENU_ID && opticalProfile.menus))) {
            const panel = window.height <= 80 && window.width >= parent.width / 2 &&
                window.width * window.height <= 524288 &&
                !this.gnomeEffectRenderer.effects?.length && !this.widgetHost?.state?.instances?.some(item=>item.visible) &&
                !this.nativeCanvasWindow.wallpaper.flight &&
                !this.nativeCanvasWindow.snapshots.some(native=>native.role!=='desktop' && native.layerVisible!==false &&
                    native.frame.x < window.x+window.width && native.frame.x+native.frame.width > window.x &&
                    native.frame.y < window.y+window.height && native.frame.y+native.frame.height > window.y);
            const panelKey=panel ? JSON.stringify([this.sceneBackdropRevision,window.x,window.y,window.width,window.height,opticalProfile]) : null;
            this.liquidPanelCache ||= new Map();
            const retained=panelKey && this.liquidPanelCache.get(window.id);
            if (retained?.key===panelKey) {
                this.context.drawImage(retained.canvas,window.x,window.y);
                return;
            }
            // A thin panel needs a panel-sized mask, not two full-desktop
            // copies. Keep optical sampling in desktop coordinates.
            if (!this.shellGlassTile || this.shellGlassTile.width !== window.width || this.shellGlassTile.height !== window.height)
                this.shellGlassTile = this.canvasFactory(window.width, window.height);
            const tile = this.shellGlassTile.getContext('2d');
            tile.clearRect(0, 0, window.width, window.height);
            this.shellFrostedIceMaterial.invalidateBackdrop();
            this.shellFrostedIceMaterial.draw(tile, this.context.canvas || this.canvas,
                {x:0,y:0,width:window.width,height:window.height}, window.id === RICEWM_MENU_ID ? 12 : 0,
                {...opticalProfile, pointerLight:false, rim:false}, this.now(),
                {x:window.x,y:window.y,width:window.width,height:window.height});
            tile.globalCompositeOperation = 'destination-in';
            tile.drawImage(window.surface,0,0);
            tile.globalCompositeOperation = 'source-over';
            this.context.drawImage(this.shellGlassTile,window.x,window.y);
            if (panelKey) {
                this.liquidPanelCache.set(window.id,{key:panelKey,canvas:this.shellGlassTile});
                this.shellGlassTile=null;
                while(this.liquidPanelCache.size>4)this.liquidPanelCache.delete(this.liquidPanelCache.keys().next().value);
            } else this.liquidPanelCache.delete(window.id);
            return;
        }
        this.liquidPanelCache?.clear();
        if (
            !this.shellGlassBackdrop ||
            this.shellGlassBackdrop.width !== parent.width ||
            this.shellGlassBackdrop.height !== parent.height
        ) {
            this.shellGlassBackdrop ||= this.canvasFactory(parent.width, parent.height);
            resizeCanvasBackingStore(this.shellGlassBackdrop, parent.width, parent.height);
        }
        const backdrop = this.shellGlassBackdrop.getContext("2d", {
            alpha: true,
        });
        if (!backdrop) return;
        const shell = this.nativeCanvasWindow.snapshot?.shell;
        const frostedIce = shell?.frostedIce;
        const riceWmSurface = window.id === RICEWM_TASKBAR_ID ||
            window.id === RICEWM_MENU_ID;
        const frostedRiceWmSurface = riceWmSurface &&
            frostedIce?.enabled === true &&
            (window.id === RICEWM_TASKBAR_ID
                ? frostedIce?.panels === true
                : frostedIce?.menus === true);
        backdrop.save();
        try {
            backdrop.setTransform(1, 0, 0, 1, 0, 0);
            backdrop.clearRect(0, 0, parent.width, parent.height);
            if (frostedRiceWmSurface) {
                // X11 shell layers are composed after the native/window
                // layers. Start from their current pixels, then share the
                // resulting capture within this material layer.
                this.shellFrostedIceMaterial.invalidateBackdrop();
                this.shellFrostedIceMaterial.draw(
                    backdrop,
                    this.context.canvas || this.compositionSurface || this.canvas,
                    {
                        x: window.x,
                        y: window.y,
                        width: window.width,
                        height: window.height,
                    },
                    window.id === RICEWM_MENU_ID ? 12 : 0,
                    {
                        ...frostedIce,
                        pointerLight: frostedIce.pointerLight === true &&
                            frostedIce.ricewmMenuPointerLight !== false,
                        rim: false,
                    },
                    this.now(),
                );
            } else {
                backdrop.filter = "blur(24px) saturate(1.45) contrast(1.08)";
                // Filter only the source neighborhood needed by this surface.
                const x = Math.max(0, window.x - 72);
                const y = Math.max(0, window.y - 72);
                const width = Math.min(parent.width, window.x + window.width + 72) - x;
                const height = Math.min(parent.height, window.y + window.height + 72) - y;
                if (width > 0 && height > 0) backdrop.drawImage(
                    this.context.canvas || this.compositionSurface || this.canvas,
                    x, y, width, height, x, y, width, height,
                );
                backdrop.filter = "none";
            }
            backdrop.globalCompositeOperation = "destination-in";
            backdrop.drawImage(window.surface, window.x, window.y);
        } finally {
            backdrop.restore();
        }
        this.context.save();
        if (this._wedgedMenu(window, shell)) {
            this.context.beginPath();
            appendWedgedSurfacePath(this.context, {
                x: window.x,
                y: window.y,
                width: window.width,
                height: window.height,
            });
            this.context.clip();
        }
        this.context.drawImage(this.shellGlassBackdrop, 0, 0);
        this.context.restore();
    }

    _wedgedMenu(window, shell = this.nativeCanvasWindow.snapshot?.shell) {
        if (window.id !== RICEWM_MENU_ID) return false;
        const configured = shell?.windowDecorations?.windowShape;
        return configured === "Wedged" ||
            (configured !== "Default" &&
                this.nativeCanvasWindow.snapshot?.chrome?.windowShape === "Wedged");
    }

    _renderTransparentTopbarSurface(window) {
        if (!window.surfaceContext || !window.surface) return;
        // Use the original upload, not a canvas readback: privacy protections
        // can perturb readback colors and leave speckles in the color-key mask.
        // Keep the opaque source intact, including extension-owned artwork.
        const cache = this.transparentTopbarSurfaces ||= new WeakMap();
        let entry = cache.get(window.surface);
        if (!entry || entry.revision !== window.spatialRevision) {
            const retained = window.topbarPixels;
            if (!retained) return;
            const image = window.surfaceContext.createImageData(window.width, window.height);
            image.data.set(retained.data);
            const pixels = image.data;
            const [red, green, blue] = pixels;
            for (let offset = 0; offset < pixels.length; offset += 4) {
                if (pixels[offset] === red && pixels[offset + 1] === green && pixels[offset + 2] === blue) {
                    pixels[offset + 3] = 0;
                }
            }
            const surface = entry?.surface || this.canvasFactory(window.width, window.height);
            surface.getContext('2d').putImageData(image, 0, 0);
            entry = {...entry, surface, revision: window.spatialRevision};
            cache.set(window.surface, entry);
        }
        if (!entry.backdrop || entry.backdrop.canvas.width !== window.width || entry.backdrop.canvas.height !== window.height) {
            entry.backdrop = this.canvasFactory(window.width, window.height).getContext('2d');
        }
        // Retain the clean underlay before any panel artwork is painted.
        // Small compositor tiles cannot sample the panel's global origin;
        // update only their intersection, leaving the other underlay intact.
        const origin = this.nativeCanvasWindow.paintOrigin;
        const source = this.context.canvas;
        const regions = this.lastDamageRects || [{ x: window.x, y: window.y, width: window.width, height: window.height }];
        for (const rect of regions) {
            const x = Math.max(window.x, rect.x, origin?.x || 0);
            const y = Math.max(window.y, rect.y, origin?.y || 0);
            const right = Math.min(window.x + window.width, rect.x + rect.width, (origin?.x || 0) + source.width);
            const bottom = Math.min(window.y + window.height, rect.y + rect.height, (origin?.y || 0) + source.height);
            if (right <= x || bottom <= y) continue;
            entry.backdrop.clearRect(x - window.x, y - window.y, right - x, bottom - y);
            entry.backdrop.drawImage(source, x - (origin?.x || 0), y - (origin?.y || 0), right - x, bottom - y,
                x - window.x, y - window.y, right - x, bottom - y);
        }
        window.topbarBackdrop = entry.backdrop;
        this.context.drawImage(entry.surface, 0, 0);
    }

    _renderTransparentTopbarArtwork(context, panel) {
        const entry = this.transparentTopbarSurfaces?.get(panel.surface);
        if (!entry) return;
        context.drawImage(entry.surface, 0, 0);
        this._renderShellTextInto(context, panel);
        const previous = this.context;
        const icons = this.nativeCanvasWindow.iconRenderer;
        const previousIcons = icons.context;
        context.save();
        context.translate(-panel.x, -panel.y);
        this.context = context;
        icons.context = context;
        try {
            this.topbarController.paint(context, {panel, foregroundOnly: true});
        } finally {
            this.context = previous;
            icons.context = previousIcons;
            context.restore();
        }
    }

    _renderWindowSurface(window, shell) {
        const context = this.context;
        const surface = window.scaledUploadSurface || window.surface;
        if (window.scaledUploadSurface) {
            context.imageSmoothingEnabled = true;
            context.imageSmoothingQuality = "low";
            context.drawImage(surface, 0, 0, window.width, window.height);
            return;
        }
        if (window.id !== RICEWM_TASKBAR_ID || window.height < 3) {
            context.drawImage(surface, 0, 0);
            return;
        }
        if (this.nativeCanvasWindow.snapshot?.chrome?.chromeProfile === "orchard") {
            // Orchard has a continuous bar, with no legacy ridge to relocate.
            context.drawImage(surface, 0, 0);
            return;
        }
        if (this.nativeCanvasWindow.snapshot?.chrome?.redmond98 === true) {
            // The retained Redmond surface already contains the exact etched
            // top ridge and classic button bevels. The configurable modern
            // bar-edge pass would repaint those source rows as a gray slab.
            context.drawImage(surface, 0, 0);
            return;
        }
        const border = riceWmBarBorderPixels(
            shell?.windowDecorations?.ricewmMenuBorder,
        );
        // The native bar stores its theme ridge in the first two source rows.
        // Paint the ordinary bar color over that legacy ridge, then restore a
        // configurable fraction at the desktop-facing edge.
        context.drawImage(surface, 0, 0);
        context.drawImage(
            window.surface,
            0,
            2,
            window.width,
            1,
            0,
            0,
            window.width,
            2,
        );
        if (border <= 0) return;
        const edgeY = window.y <= 0 ? window.height - border : 0;
        context.drawImage(
            window.surface,
            0,
            0,
            window.width,
            2,
            0,
            edgeY,
            window.width,
            border,
        );
    }

    _renderWindowShadow(window, parent, menu = false) {
        const context = this.context;
        const redmondMenuSurface = menu && window.surface &&
            this.nativeCanvasWindow.snapshot?.chrome?.redmond98 === true;
        context.save();
        try {
            context.beginPath();
            context.rect(0, 0, parent.width, parent.height);
            if (redmondMenuSurface) {
                // Menu windows retain all cascade levels in one large ARGB
                // surface. Exclude that complete allocation here; the actual
                // alpha-bearing pixels below are the shadow caster.
                context.rect(window.x, window.y, window.width, window.height);
            } else if (this._wedgedMenu(window)) {
                appendWedgedSurfacePath(context, {
                    x: window.x,
                    y: window.y,
                    width: window.width,
                    height: window.height,
                });
            } else if (typeof context.roundRect === "function") {
                context.roundRect(
                    window.x,
                    window.y,
                    window.width,
                    window.height,
                    8,
                );
            } else {
                context.rect(window.x, window.y, window.width, window.height);
            }
            context.clip("evenodd");
            context.shadowColor = menu
                ? "rgba(0, 0, 0, 0.42)"
                : "rgba(0, 0, 0, 0.46)";
            context.shadowBlur = menu ? 18 : 28;
            context.shadowOffsetX = 0;
            context.shadowOffsetY = menu ? 6 : 10;
            if (redmondMenuSurface) {
                // drawImage makes the shadow follow the surface alpha instead
                // of the allocation bounds. The source itself is clipped out,
                // so transparent cascade space cannot become a giant shadow.
                context.drawImage(window.surface, window.x, window.y);
            } else {
                context.fillStyle = "rgba(0, 0, 0, 0.02)";
                if (this._wedgedMenu(window)) {
                    context.beginPath();
                    appendWedgedSurfacePath(context, {
                        x: window.x,
                        y: window.y,
                        width: window.width,
                        height: window.height,
                    });
                } else {
                    this._roundedRect(context, {
                        x: window.x,
                        y: window.y,
                        width: window.width,
                        height: window.height,
                    }, 8);
                }
                context.fill();
            }
        } finally {
            context.restore();
        }
    }

    _renderShellText(window) {
        const overlays = this.shellTexts.get(window.id) || [];
        if (overlays.length === 0) return;
        const context = this.context;
        const transparentTopbar = window.id === RICEWM_GNOME_PANEL_ID && this.topbarController?.transparent;
        const redmond98 = this.nativeCanvasWindow.snapshot?.chrome?.redmond98 === true;
        const menuLayers = window.id === RICEWM_MENU_ID
            ? overlays.filter(overlay => overlay.text === "__menu_layer") : [];
        const menuRoot = menuLayers.length ? this.windows.get(window.parent) : null;
        const compactMenu = menuRoot && (menuRoot.width < 700 || menuRoot.height < 500);
        let menuLayer = -1;
        const overlaps = (a, b) => a.x < b.x + b.width && b.x < a.x + a.width &&
            a.y < b.y + b.height && b.y < a.y + a.height;
        const excludeLaterMenus = () => {
            // Separate clips subtract the union even when several children
            // overlap each other; one even-odd path would reopen their overlap.
            for (const later of menuLayers.slice(menuLayer + 1)) {
                context.beginPath();
                context.rect(0, 0, window.width, window.height);
                context.rect(later.x, later.y, later.width, later.height);
                context.clip("evenodd");
            }
        };
        for (const [overlayIndex, overlay] of overlays.entries()) {
            if (window.id === RICEWM_MENU_ID && overlay.text === "__menu_layer") {
                menuLayer += 1;
                context.save();
                excludeLaterMenus();
                this.themeSkins ||= new RiceThemeSkinRenderer(()=>{this.damaged=true;this.schedule();});
                this.themeSkins.paint(context,overlay,this.nativeCanvasWindow.snapshot?.chrome,'menu');
                context.restore();
                if (compactMenu && menuLayers.slice(0, menuLayer).some(parent => overlaps(parent, overlay))) {
                    context.save();
                    excludeLaterMenus();
                    context.strokeStyle = pixelColor(overlay.foreground);
                    context.lineWidth = 1;
                    context.shadowColor = "rgba(0, 0, 0, 0.4)";
                    context.shadowBlur = 8;
                    context.shadowOffsetX = -2;
                    context.shadowOffsetY = 2;
                    context.strokeRect(overlay.x + .5, overlay.y + .5,
                        overlay.width - 1, overlay.height - 1);
                    context.restore();
                }
                continue;
            }
            if (
                overlay.role === "window-title" || !overlay.text || overlay.text.startsWith("__topbar\t") ||
                overlay.width <= 0 ||
                overlay.height <= 0
            ) {
                continue;
            }
            if (overlay.text.startsWith("__dock_icon:")) {
                const [,kind,id] = overlay.text.split(":");
                const native = kind === "native" ? this.nativeCanvasWindow.snapshots.find(window=>
                    Number(window.taskbarId)===Number(id) || Number(window.dialogTaskbarId)===Number(id)) : null;
                const shortcut = kind === "quick" ? this.quickApplicationIcons.find(icon=>icon.index===Number(id)) : null;
                const icon = kind === "logo" ? "ricewm-logo" : kind === "activities" ? "view-grid" : shortcut ? shortcut.iconName : native ? native.iconName || nativeEntrypointIconName(native.applicationId?.startsWith("native:") ? native.applicationId.slice(7) : native.entrypoint) : null;
                if (icon) {
                    const rect={x:overlay.x,y:overlay.y,width:overlay.width,height:overlay.height};
                    context.save();
                    if(kind==='native'){this.themeSkins ||= new RiceThemeSkinRenderer(()=>{this.damaged=true;this.schedule();});this.themeSkins.paint(context,{x:rect.x+rect.width/2-8,y:rect.y+rect.height+3,width:16,height:3},this.nativeCanvasWindow.snapshot?.chrome,'runningIndicator',native?.active?'selected':'normal');}
                    const renderer=this.nativeCanvasWindow.iconRenderer;
                    const draw=kind === "logo" || icon === "desktop" ? renderer.renderBuiltInNamed : renderer.renderNamed;
                    const ink=this.themeSkins?.style(this.nativeCanvasWindow.snapshot?.chrome,'dock','normal').foreground;
                    const applicationId=shortcut?.applicationId||native?.riceAppId||native?.applicationId;
                    (ink?renderer.renderNamedWithColor:draw).call(renderer,{icon,rect,...(applicationId?{applicationId}:{}),color:ink?themeColor(ink,[0,0,0,255]):kind==='logo' && this.nativeCanvasWindow.snapshot?.shell?.surface?.frame==='bracket' ? [255,255,255,255] : [0,0,0,255]});
                    context.restore();
                }
                continue;
            }
            if (overlay.text.startsWith("__dock_preview:")) {
                const [, kind, id] = overlay.text.split(":");
                const rect = {x:overlay.x,y:overlay.y,width:overlay.width,height:overlay.height};
                context.save(); this._roundedRect(context,rect,8); context.clip();
                context.fillStyle = pixelColor(overlay.background); context.fillRect(rect.x,rect.y,rect.width,rect.height);
                if (kind === "native") this.nativeCanvasWindow.renderTaskPreview(Number(id),context,rect);
                else this._renderX11TaskPreview(Number(id),context,rect);
                context.restore();
                const close={x:rect.x+rect.width-28,y:rect.y+4,width:24,height:24};
                const pointer=this.pointerPosition;
                const hovered=pointer && pointer.x>=window.x+close.x && pointer.x<window.x+close.x+24 && pointer.y>=window.y+close.y && pointer.y<window.y+close.y+24;
                context.save();context.fillStyle=hovered?"#b42335":"rgba(24,28,34,.88)";this._roundedRect(context,close,12);context.fill();
                context.strokeStyle="#fff";context.lineWidth=1.5;context.beginPath();context.moveTo(close.x+8,close.y+8);context.lineTo(close.x+16,close.y+16);context.moveTo(close.x+16,close.y+8);context.lineTo(close.x+8,close.y+16);context.stroke();context.restore();continue;
            }
            context.save();
            if (menuLayer >= 0) {
                const layer = menuLayers[menuLayer];
                context.beginPath();
                context.rect(layer.x, layer.y, layer.width, layer.height);
                context.clip();
                excludeLaterMenus();
            }
            const backgroundAlpha = overlay.background >>> 24;
            const skinChrome=this.nativeCanvasWindow.snapshot?.chrome;
            const skinRole=window.id===RICEWM_MENU_ID?'menuItem':null;
            const skinRect={x:Math.max(0,overlay.x-36),y:Math.max(0,overlay.y-3),width:Math.min(window.width,overlay.width+40),height:overlay.height+7};
            const skinHover=this.pointerPosition&&this.pointerPosition.x-window.x>=skinRect.x&&this.pointerPosition.x-window.x<skinRect.x+skinRect.width&&this.pointerPosition.y-window.y>=skinRect.y&&this.pointerPosition.y-window.y<skinRect.y+skinRect.height;
            this.themeSkins ||= new RiceThemeSkinRenderer(()=>{this.damaged=true;this.schedule();});
            if(skinRole)this.themeSkins.paint(context,skinRect,skinChrome,skinRole,skinHover?'hover':'normal');
            const popupRow = {
                x: Math.max(0, overlay.x - 35),
                y: overlay.y,
                width: overlay.width + 38,
                height: overlay.height,
            };
            const overlapsEarlierMenuRow = !menuLayers.length && window.id === RICEWM_MENU_ID &&
                overlays.slice(0, overlayIndex).some(previous => {
                    if (previous.x !== overlay.x) return false;
                    const previousRow = {
                        x: Math.max(0, previous.x - 35),
                        y: previous.y,
                        width: previous.width + 38,
                        height: previous.height,
                    };
                    return popupRow.x < previousRow.x + previousRow.width &&
                        popupRow.x + popupRow.width > previousRow.x &&
                        popupRow.y < previousRow.y + previousRow.height &&
                        popupRow.y + popupRow.height > previousRow.y;
                });
            if (overlapsEarlierMenuRow &&
                (backgroundAlpha === 0 || backgroundAlpha >= 254)) {
                // A context popup may intentionally overlap its parent menu.
                // Shell text is composited after the shared X11 surface, so
                // repaint the later popup's complete inner row before its
                // glyphs. This gives the child true z-order without erasing
                // the three-pixel classic popup bevel.
                context.fillStyle = pixelColor(overlay.background);
                context.fillRect(
                    popupRow.x,
                    popupRow.y,
                    popupRow.width,
                    popupRow.height,
                );
            }
            const commandDeckPill = window.id === RICEWM_MENU_ID &&
                backgroundAlpha === 254;
            const semanticMenuIcon = overlay.menuIconKind === "image"
                ? "image"
                : overlay.menuIconKind === "folder"
                    ? "folder"
                    : null;
            // The vertical branding strip is text, not a selectable menu row.
            // Inferring an icon from its label paints over the rotated wordmark.
            const namedMenuIcon = window.id === RICEWM_MENU_ID && !overlay.rotateCcw
                ? semanticMenuIcon ||
                    this.menuApplicationIcons.get(String(overlay.text || "")) ||
                    this.menuBuiltinIcons.get(String(overlay.text || "")) ||
                    riceNativeMenuIconName(overlay.text)
                : null;
            const namedTitleIcon = window.id !== RICEWM_MENU_ID &&
                window.id !== RICEWM_TASKBAR_ID &&
                overlay.x === 27 && overlay.y <= 3
                ? this.menuApplicationIcons.get(String(overlay.text || ""))
                : null;
            const namedTaskIcon = window.id === RICEWM_TASKBAR_ID &&
                (overlay.y === 5 || (redmond98 && overlay.y === 6)) &&
                overlay.height === 22
                ? this.menuApplicationIcons.get(String(overlay.text || "")) ||
                    (() => {
                        const title = String(overlay.text || "")
                            .replace(/\s+\(\d+\)$/u, "");
                        const candidates = this.nativeCanvasWindow.snapshots.filter(candidate =>
                            candidate.role !== "desktop" && title &&
                            String(candidate.title || "").startsWith(title));
                        // The shell clips task labels to the button's width.
                        // A unique prefix still identifies the owning app.
                        const native = candidates.find(candidate => candidate.title === title) ||
                            (candidates.length === 1 ? candidates[0] : null);
                        return native?.iconName || (native
                            ? nativeEntrypointIconName(
                                native.applicationId?.startsWith("native:")
                                    ? native.applicationId.slice(7)
                                    : native.entrypoint,
                            ) : null);
                    })() ||
                    riceNativeTaskIconName(overlay.text)
                : null;
            const redmondStartIcon = redmond98 && namedTaskIcon &&
                overlay.text === "Start";
            const namedIcon = namedMenuIcon || namedTitleIcon || namedTaskIcon;
            if (namedIcon) {
                const iconRect = namedTaskIcon ? {
                    x: redmondStartIcon
                        ? 5 + Math.max(0, overlay.x - 23)
                        : redmond98
                            ? Math.max(0, overlay.x - 21)
                            : Math.max(0, overlay.x - 22),
                    y: redmondStartIcon
                        ? 8 + Math.max(0, overlay.y - 5)
                        : redmond98
                            ? 8 + Math.max(0, overlay.y - 5)
                            : 7,
                    width: redmond98 ? 16 : 20,
                    height: redmond98 ? 16 : 18,
                } : namedTitleIcon ? {
                    x: 6,
                    y: 5,
                    width: 16,
                    height: 16,
                } : overlay.menuIconSize ? {
                    x: Math.max(0, overlay.x - overlay.menuIconSize - 9),
                    y: Math.max(0, overlay.y + Math.floor(
                        (overlay.height + 1 - overlay.menuIconSize) / 2,
                    )),
                    width: overlay.menuIconSize,
                    height: overlay.menuIconSize,
                } : {
                    x: Math.max(0, overlay.x - 29),
                    y: overlay.y + 6,
                    width: 20,
                    height: Math.min(18, overlay.height),
                };
                const eraseFallback = !(namedMenuIcon && overlay.iconFallbackOmitted);
                if (eraseFallback && commandDeckPill) {
                    // The X11 surface has already painted the complete LCARS
                    // selection pill. Clear the fallback raster icon without
                    // letting a rectangular backplate flatten its left cap.
                    context.save();
                    context.beginPath();
                    context.roundRect(
                        Math.max(0, overlay.x - 35),
                        overlay.y,
                        overlay.width + 70,
                        overlay.height + 1,
                        Math.min(15, (overlay.height + 1) / 2),
                    );
                    context.clip();
                    context.fillStyle = pixelColor(overlay.background);
                    context.fillRect(
                        Math.max(0, iconRect.x - 5),
                        Math.max(0, overlay.y + 1),
                        28,
                        Math.min(28, overlay.height + 4),
                    );
                    context.restore();
                } else if (eraseFallback &&
                    (backgroundAlpha === 0 || backgroundAlpha === 255) &&
                    !redmondStartIcon) {
                    context.fillStyle = pixelColor(overlay.background);
                    if (redmond98 && (namedMenuIcon || namedTaskIcon || namedTitleIcon)) {
                        // The retained fallback is wholly inside this slot.
                        // Clear only the icon pixels: a wider erase box paints
                        // through the classic button or popup bevel.
                        context.fillRect(
                            iconRect.x,
                            iconRect.y,
                            iconRect.width,
                            iconRect.height,
                        );
                    } else {
                        context.fillRect(
                            namedTitleIcon ? 2 : namedTaskIcon
                                ? Math.max(0, iconRect.x - 3)
                                : Math.max(0, iconRect.x - 5),
                            namedTitleIcon ? 1 : namedTaskIcon
                                ? 5
                                : Math.max(0, overlay.y + 1),
                            28,
                            namedTitleIcon ? 24 : namedTaskIcon
                                ? 22
                                : Math.min(28, overlay.height + 4),
                        );
                    }
                }
                const localPointer = this.pointerPosition && {
                    x: this.pointerPosition.x - window.x,
                    y: this.pointerPosition.y - window.y,
                };
                const hovered = Boolean(namedMenuIcon && localPointer &&
                    localPointer.x >= iconRect.x - 9 &&
                    localPointer.x < overlay.x + overlay.width + 20 &&
                    localPointer.y >= overlay.y - 3 &&
                    localPointer.y < overlay.y + overlay.height + 3);
                context.save();
                context.beginPath();
                context.rect(
                    iconRect.x,
                    iconRect.y,
                    iconRect.width,
                    iconRect.height,
                );
                context.clip();
                const menuApplicationId=this.menuApplicationIds?.get(String(overlay.text||''));
                const iconCommand = {
                    ...(menuApplicationId?{applicationId:menuApplicationId}:{}),
                    icon: namedIcon,
                    ...(this.menuColorArtworkLabels.has(String(overlay.text || ""))
                        ? { colorArtwork: true } : {}),
                    rect: iconRect,
                    hovered,
                    ...(namedMenuIcon ? { background: [
                        (overlay.background >>> 16) & 0xff,
                        (overlay.background >>> 8) & 0xff,
                        overlay.background & 0xff,
                        0xff,
                    ] } : {}),
                    color: [
                        (overlay.foreground >>> 16) & 0xff,
                        (overlay.foreground >>> 8) & 0xff,
                        overlay.foreground & 0xff,
                        0xff,
                    ],
                };
                if (redmondStartIcon) {
                    this.nativeCanvasWindow.iconRenderer.renderBuiltInNamed(iconCommand);
                } else {
                    this.nativeCanvasWindow.iconRenderer.renderNamed(iconCommand);
                }
                const sourceIcon = namedMenuIcon &&
                    this.menuSourceIcons.get(String(overlay.text || ""));
                if (sourceIcon && sourceIcon !== namedMenuIcon) {
                    const badgeSize = Math.max(
                        10,
                        Math.min(
                            13,
                            Math.round(Math.min(iconRect.width, iconRect.height) * .58),
                        ),
                    );
                    const badgeRect = {
                        x: iconRect.x + iconRect.width - badgeSize,
                        y: iconRect.y + iconRect.height - badgeSize,
                        width: badgeSize,
                        height: badgeSize,
                    };
                    context.fillStyle = pixelColor(overlay.background);
                    context.beginPath();
                    context.roundRect(
                        badgeRect.x - 1,
                        badgeRect.y - 1,
                        badgeRect.width + 2,
                        badgeRect.height + 2,
                        Math.ceil(badgeSize / 2),
                    );
                    context.fill();
                    this.nativeCanvasWindow.iconRenderer.renderNamed({
                        icon: sourceIcon,
                        ...(iconCommand.colorArtwork ? { colorArtwork: true } : {}),
                        rect: badgeRect,
                        hovered,
                        color: iconCommand.color,
                    });
                }
                context.restore();
            }
            if (!(skinRole&&Object.keys(this.themeSkins.style(skinChrome,skinRole,skinHover?'hover':'normal')).length) && !transparentTopbar &&
                !redmond98 && !commandDeckPill &&
                (backgroundAlpha === 0 || backgroundAlpha === 255)) {
                context.fillStyle = pixelColor(overlay.background);
                context.fillRect(
                    overlay.x,
                    overlay.y,
                    overlay.width,
                    overlay.height,
                );
            }
            const weight = Math.max(
                400,
                Number(overlay.weight) || 400,
            );
            const maximumWidth = Math.max(
                1,
                (overlay.rotateCcw ? overlay.height : overlay.width) - 4,
            );
            let fontPx = Math.min(
                Math.max(9, this.nativeCanvasWindow.snapshot?.chrome?.typography?.bodyPx??(Number(overlay.fontPx) || 12)),
                Math.max(
                    9,
                    (overlay.rotateCcw ? overlay.width : overlay.height) - 4,
                ),
            );
            const setFont = () => {
                context.font =
                    `${weight} ${fontPx}px ` +
                    (window.id === RICEWM_GNOME_BACKGROUND_ID
                        ? '"RiceWM Mono", "RiceWM CJK", monospace'
                        : redmond98
                            ? classicUiFont
                        : '"RiceWM Sans", "RiceWM CJK", sans-serif');
                const typography=this.nativeCanvasWindow.snapshot?.chrome?.typography;if(typography&&Object.keys(typography).length)context.font=desktopThemeFont(typography,'body',fontPx);
            };
            setFont();
            while (
                fontPx > 9 &&
                context.measureText(overlay.text).width > maximumWidth
            ) {
                fontPx -= 1;
                setFont();
            }
            let fittedText = overlay.text;
            if (context.measureText(fittedText).width > maximumWidth) {
                const codepoints = Array.from(fittedText);
                while (
                    codepoints.length > 1 &&
                    context.measureText(`${codepoints.join("")}…`).width >
                        maximumWidth
                ) {
                    codepoints.pop();
                }
                fittedText = codepoints.length < Array.from(overlay.text).length
                    ? `${codepoints.join("")}…`
                    : fittedText;
            }
            context.textAlign = ["left", "center", "right"].includes(
                overlay.align,
            ) ? overlay.align : "left";
            context.textBaseline = "middle";
            const topbarPaper = transparentTopbar && window.topbarBackdrop
                ? window.topbarBackdrop.getImageData(Math.min(window.width - 1, Math.max(0, Math.floor(overlay.x + overlay.width / 2))),
                    Math.min(window.height - 1, Math.max(0, Math.floor(overlay.y + overlay.height / 2))), 1, 1).data
                : null;
            context.fillStyle = transparentTopbar&&this.spatialShellOnly
                ? (this.spatialShellLease?.session?.shellInk? '#'+this.spatialShellLease.session.shellInk:pixelColor(overlay.foreground)) : topbarPaper
                ? rgbaColor(topbarPaper[3]===0?[(overlay.foreground >>> 16) & 255,(overlay.foreground >>> 8) & 255,overlay.foreground & 255,255]:readableInk([(overlay.foreground >>> 16) & 255, (overlay.foreground >>> 8) & 255, overlay.foreground & 255, 255], topbarPaper))
                : pixelColor(overlay.foreground);
            const surfaceInk=this.themeSkins.style(skinChrome,skinRole||'dock',skinHover?'hover':'normal').foreground||(skinRole?this.themeSkins.style(skinChrome,'menu','normal').foreground:undefined);if(surfaceInk)context.fillStyle=surfaceInk;
            if (overlay.rotateCcw) {
                context.translate(
                    overlay.x + overlay.width / 2,
                    overlay.y + overlay.height / 2,
                );
                context.rotate(-Math.PI / 2);
                context.textAlign = "center";
                context.fillText(fittedText, 0, 0);
                context.restore();
                continue;
            }
            const inset = context.textAlign === "left"
                ? 2
                : context.textAlign === "right"
                    ? overlay.width - 2
                    : overlay.width / 2;
            context.fillText(
                fittedText,
                overlay.x + inset,
                overlay.y + overlay.height / 2,
            );
            context.restore();
        }
    }

    _renderOrchardNotification(window) {
        const context = this.context;
        const overlays = this.shellTexts.get(window.id) || [];
        const rect = {x:0, y:0, width:window.width, height:window.height};
        const profile = this.nativeCanvasWindow.snapshot?.shell?.frostedIce;
        context.save();
        context.shadowColor = "rgba(28,57,88,.18)";
        context.shadowBlur = 24;
        context.shadowOffsetY = 8;
        this._roundedRect(context, rect, 22);
        context.fillStyle = "rgba(237,245,252,.90)";
        context.fill();
        context.shadowColor = "transparent";
        if (profile?.enabled && profile.system) {
            this.shellFrostedIceMaterial.invalidateBackdrop();
            this.shellFrostedIceMaterial.draw(context, context.canvas, rect, 22,
                {...profile, tint:[237,245,252], opacity:82, refraction:18, chroma:0, rim:false, sheet:true},
                this.now(), {x:window.x,y:window.y,width:window.width,height:window.height});
        }
        this._roundedRect(context, {x:.5,y:.5,width:rect.width-1,height:rect.height-1}, 22);
        context.strokeStyle = "rgba(128,152,173,.38)";
        context.lineWidth = 1;context.stroke();
        this.themeSkins ||= new RiceThemeSkinRenderer(()=>{this.damaged=true;this.schedule();});
        this.themeSkins.paint(this.context,{x:0,y:0,width:window.width,height:window.height},this.nativeCanvasWindow.snapshot?.chrome,'notification');
        const iconColor = window.matrixNotificationKind === "error" ? [204,60,70,255]
            : window.matrixNotificationKind === "warning" ? [170,108,16,255] : [0,136,255,255];
        this._roundedRect(context, {x:14,y:24,width:32,height:32}, 10);
        context.fillStyle = "rgba(255,255,255,.86)";context.fill();
        const icons = this.nativeCanvasWindow.iconRenderer;
        const previous = icons.context;icons.context = context;
        try { icons.renderBuiltInNamed({icon:'ricewm-logo',rect:{x:20,y:30,width:20,height:20},color:iconColor}); }
        finally { icons.context = previous; }
        context.fillStyle = this.themeSkins.style(this.nativeCanvasWindow.snapshot?.chrome,'notification','normal').foreground||'#1d2935';
        context.textAlign = "left";context.textBaseline = "middle";
        context.font = '600 13px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        context.fillText(String(overlays[0]?.text || 'RiceWM'), 58, 27, Math.max(1,rect.width-94));
        context.font = '400 12px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        this._notificationTextLines(String(overlays[1]?.text || ''),Math.max(1,rect.width-76),2)
            .forEach((line,index)=>context.fillText(line,58,47+index*16));
        context.fillStyle = "#7e8e9b";context.textAlign = "center";
        context.font = '400 18px "RiceWM Sans", system-ui, sans-serif';
        context.fillText('×',rect.width-20,20);
        context.restore();
    }

    _renderMatrixNotification(window) {
        const context = this.context;
        const overlays = this.shellTexts.get(window.id) || [];
        const title = String(overlays[0]?.text || "RICEWM STATUS").toUpperCase();
        const message = String(overlays[1]?.text || "");
        const kind = window.matrixNotificationKind || "info";
        const palette = riceMatrixNotificationPalette(kind);
        const motionEnabled = this.nativeCanvasWindow.snapshot?.shell?.motionEnabled !== false;
        const breathe = riceMatrixNotificationBreathe(this.now(), motionEnabled);
        const rgba = alpha => `rgba(${palette.rgb}, ${alpha})`;
        const rect = { x: 0, y: 0, width: window.width, height: window.height };

        context.save();
        context.shadowColor = rgba(breathe.glowAlpha);
        context.shadowBlur = breathe.glowBlur;
        context.fillStyle = "rgba(0, 0, 0, .94)";
        context.beginPath();
        context.roundRect(rect.x, rect.y, rect.width, rect.height, 10);
        context.fill();
        context.shadowBlur = 0;

        context.beginPath();
        context.roundRect(rect.x, rect.y, rect.width, rect.height, 10);
        context.clip();
        const wash = context.createLinearGradient?.(0, 0, rect.width * .62, rect.height);
        if (wash) {
            wash.addColorStop(0, rgba(kind === "error" ? .25 : .22));
            wash.addColorStop(1, rgba(0));
            context.fillStyle = wash;
        } else {
            context.fillStyle = rgba(.12);
        }
        context.fillRect(0, 0, rect.width, rect.height);

        context.strokeStyle = rgba(.055);
        context.lineWidth = 1;
        context.beginPath();
        for (let x = 0; x <= rect.width; x += 14) {
            context.moveTo(x + .5, 0);
            context.lineTo(x + .5, rect.height);
        }
        for (let y = 0; y <= rect.height; y += 14) {
            context.moveTo(0, y + .5);
            context.lineTo(rect.width, y + .5);
        }
        context.stroke();

        context.fillStyle = rgba(.92);
        context.shadowColor = rgba(.9);
        context.shadowBlur = 12;
        context.fillRect(0, 0, 3, rect.height);
        context.shadowBlur = 0;

        this.themeSkins ||= new RiceThemeSkinRenderer(()=>{this.damaged=true;this.schedule();});
        this.themeSkins.paint(this.context,{x:0,y:0,width:window.width,height:window.height},this.nativeCanvasWindow.snapshot?.chrome,'notification');
        context.fillStyle = this.themeSkins.style(this.nativeCanvasWindow.snapshot?.chrome,'notification','normal').foreground||palette.text;
        context.textAlign = "left";
        context.textBaseline = "middle";
        context.font = '700 10px "RiceWM Mono", "RiceWM CJK", monospace';
        context.fillText(title, 14, 20, Math.max(1, rect.width - 54));

        context.font = '500 12px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        const lines = this._notificationTextLines(
            message,
            Math.max(1, rect.width - 52),
            2,
        );
        lines.forEach((line, index) => {
            context.fillText(line, 14, 47 + index * 18);
        });

        context.globalAlpha = .68;
        context.font = '500 18px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        context.textAlign = "center";
        context.fillText("×", rect.width - 20, 20);
        context.globalAlpha = 1;

        context.strokeStyle = rgba(breathe.borderAlpha);
        context.lineWidth = 1;
        context.beginPath();
        context.roundRect(.5, .5, rect.width - 1, rect.height - 1, 10);
        context.stroke();
        context.restore();

        if (motionEnabled) {
            this._scheduleMatrixNotificationBreathe();
        }
    }

    _scheduleMatrixNotificationBreathe() {
        if (this.matrixNotificationBreatheTimer !== null) return;
        this.matrixNotificationBreatheTimer = this.setTimer(() => {
            this.matrixNotificationBreatheTimer = null;
            const current = this.windows.get(RICEWM_NOTIFICATION_ID);
            if (!current?.viewable) return;
            this._markNativeDamage([{
                x: current.x - 32,
                y: current.y - 32,
                width: current.width + 64,
                height: current.height + 64,
            }]);
            this.schedule();
        }, 50);
        this.matrixNotificationBreatheTimer.unref?.();
    }

    _renderLaunchAnimations() {
        this.nativeCanvasWindow.renderDesktopLaunchAnimations();
    }

    _renderTaskPreview(window) {
        const context = this.context;
        const width = window.width;
        const height = window.height;
        const taskbar = this.windows.get(RICEWM_TASKBAR_ID);
        const menu = this.windows.get(RICEWM_MENU_ID);
        const verticalDock = taskbar && taskbar.height > taskbar.width;
        const caretSide = verticalDock
            ? (window.x >= taskbar.x + taskbar.width ? "left" : "right")
            : menu?.viewable && window.x + width <= menu.x
            ? "right"
            : menu?.viewable && window.x >= menu.x + menu.width
                ? "left"
                : null;
        const caretAtTop = taskbar && window.y > taskbar.y;
        const caretHeight = 12;
        const card = caretSide === "left"
            ? {
                x: caretHeight,
                y: 8,
                width: Math.max(1, width - caretHeight - 8),
                height: Math.max(1, height - 16),
            }
            : caretSide === "right"
                ? {
                    x: 8,
                    y: 8,
                    width: Math.max(1, width - caretHeight - 8),
                    height: Math.max(1, height - 16),
                }
                : {
                    x: 8,
                    y: caretAtTop ? caretHeight : 0,
                    width: Math.max(1, width - 16),
                    height: Math.max(1, height - caretHeight),
                };
        const source = window.backgroundPixel >>> 0;
        let dark = false;
        try {
            const sample = this.nativeCanvasWindow.snapshot?.chrome?.body || window.surfaceContext?.getImageData?.(
                Math.min(width - 1, 18),
                Math.min(height - 1, card.y + 24),
                1,
                1,
            )?.data;
            dark = Boolean(sample && sample[0] * 299 + sample[1] * 587 +
                sample[2] * 114 < 128_000);
        } catch {
            // A retained shell surface can be unavailable for one frame.
        }
        const cardColor = dark
            ? "rgba(39, 39, 43, 0.97)"
            : "rgba(248, 248, 250, 0.97)";
        const textColor = dark ? "#f5f5f7" : "#202124";
        const native = (source & RICEWM_NATIVE_PREVIEW_PREFIX) !== 0
            ? this.nativeCanvasWindow.snapshots.find(candidate =>
                Number(candidate.taskbarId) === (source & 0xffff) ||
                Number(candidate.dialogTaskbarId) === (source & 0xffff))
            : null;
        const title = native?.title || (this.shellTexts.get(window.id) || [])
            .find(overlay => overlay.text)?.text || "Window preview";
        context.save();
        context.shadowColor = "rgba(0, 0, 0, 0.34)";
        context.shadowBlur = 24;
        context.shadowOffsetY = 10;
        context.fillStyle = cardColor;
        this._roundedRect(context, card, 18);
        context.fill();
        context.shadowColor = "transparent";
        context.strokeStyle = "rgba(40, 40, 48, 0.16)";
        context.lineWidth = 1;
        this._roundedRect(context, card, 18);
        context.stroke();
        context.beginPath();
        if (caretSide === "left") {
            const caretY = height / 2;
            context.moveTo(card.x + 1, caretY - 13);
            context.lineTo(0, caretY);
            context.lineTo(card.x + 1, caretY + 13);
        } else if (caretSide === "right") {
            const caretY = height / 2;
            const cardRight = card.x + card.width - 1;
            context.moveTo(cardRight, caretY - 13);
            context.lineTo(width, caretY);
            context.lineTo(cardRight, caretY + 13);
        } else if (caretAtTop) {
            const caretX = width / 2;
            context.moveTo(caretX - 13, caretHeight + 1);
            context.lineTo(caretX, 0);
            context.lineTo(caretX + 13, caretHeight + 1);
        } else {
            const caretX = width / 2;
            context.moveTo(caretX - 13, card.height - 1);
            context.lineTo(caretX, height);
            context.lineTo(caretX + 13, card.height - 1);
        }
        context.closePath();
        context.fillStyle = cardColor;
        context.fill();
        context.strokeStyle = "rgba(40, 40, 48, 0.12)";
        context.stroke();
        context.fillStyle = textColor;
        context.font = '600 14px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
        context.textAlign = "left";
        context.textBaseline = "middle";
        context.fillText(title, card.x + 16, card.y + 22, card.width - (this.dockStyle === "Compact dock" ? 60 : 32));
        if(this.dockStyle === "Compact dock") {
            const x=width-44,y=card.y+10;
            context.save();context.fillStyle="rgba(24,28,34,.88)";this._roundedRect(context,{x,y,width:24,height:24},12);context.fill();
            context.strokeStyle="#fff";context.lineWidth=1.5;context.beginPath();context.moveTo(x+8,y+8);context.lineTo(x+16,y+16);context.moveTo(x+16,y+8);context.lineTo(x+8,y+16);context.stroke();context.restore();
        }

        const preview = {
            x: card.x + 12,
            y: card.y + 42,
            width: card.width - 24,
            height: card.height - 54,
        };
        context.save();
        this._roundedRect(context, preview, 11);
        context.clip();
        context.fillStyle = dark ? "#18191c" : "#e8e9ed";
        context.fillRect(preview.x, preview.y, preview.width, preview.height);
        let rendered = false;
        if ((source & RICEWM_NATIVE_PREVIEW_PREFIX) !== 0) {
            const taskbarId = source & 0xffff;
            rendered = taskbarId === 0xffff
                ? Boolean(this.terminalPreview(context, preview))
                : this.nativeCanvasWindow.renderTaskPreview(
                    taskbarId,
                    context,
                    preview,
                );
        } else if (source !== 0) {
            rendered = this._renderX11TaskPreview(source, context, preview);
        }
        if (!rendered) {
            context.fillStyle = "#f1f3f5";
            context.fillRect(preview.x, preview.y, preview.width, preview.height);
            context.fillStyle = "#5f6368";
            context.font = '500 13px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
            context.textAlign = "center";
            context.fillText(
                t("Preview unavailable"),
                preview.x + preview.width / 2,
                preview.y + preview.height / 2,
            );
        }
        context.restore();
        context.restore();
    }

    /** Exact retained X11 producers; application names never establish identity.
     * Hidden frames retain ownership, but have no capture or input authority. */
    setSpatialX11Metadata(snapshot) {
        if(snapshot==null){this.spatialX11Metadata=null;return;}
        if(snapshot.schema!=='rice-x11-spatial/v1'||!Array.isArray(snapshot.windows)||snapshot.windows.length>1024)throw Error('Invalid X11 spatial metadata');
        const records=new Map(),clients=new Set();
        for(const record of snapshot.windows){
            if(!['client','frame','owner'].every(k=>Number.isInteger(record[k])&&record[k]>0&&record[k]<=0xffffffff)||!Number.isInteger(record.rootClient)||record.rootClient<0||record.rootClient>0xffffffff||!Number.isInteger(record.workspace)||record.workspace<0||record.workspace>=128||records.has(record.frame)||clients.has(record.client)||!['overrideRedirect','viewable','minimized','parked'].every(k=>typeof record[k]==='boolean'))throw Error('Invalid X11 spatial owner');
            const frame=this.windows.get(record.frame),client=this.windows.get(record.client);
            if(!frame||!client||!frame.spatialIdentity||!client.spatialIdentity||(record.overrideRedirect?(record.client!==record.frame||!frame.overrideRedirect):client.parent!==record.frame)){
                // Scene and metadata may cross a removal; absent owners have no authority.
                continue;
            }
            records.set(record.frame,{...record,frameToken:frame.spatialIdentity.token,clientToken:client.spatialIdentity.token});clients.add(record.client);
        }
        this.spatialX11Metadata=records;
    }
    spatialX11Windows() {
        const root=[...this.windows.values()].find(w=>w.parent===w.id);if(!root)return [];
        const children=new Map();for(const window of this.windows.values()){if(window.id===root.id)continue;const list=children.get(window.parent)||[];list.push(window);children.set(window.parent,list);}
        const result=[];
        for(const window of children.get(root.id)||[]){
            const managed=window.id>=RICEWM_FIRST_MANAGED_FRAME_ID&&window.id<RICEWM_MANAGED_FRAME_ID_LIMIT;
            if(window.inputOnly||window.desktopLayer||window.shellLayer||!window.spatialIdentity||!managed&&(window.overrideRedirect||window.id>=0xe0000000))continue;
            const width=window.width+2*window.borderWidth,height=window.height+2*window.borderWidth;
            if(!Number.isInteger(width)||!Number.isInteger(height)||width<1||height<1||width>16384||height>16384)continue;
            const seen=new Set(),revisions=[],pending=[window];
            while(pending.length){const item=pending.pop();if(seen.has(item.id)||seen.size>=4096)throw Error('Invalid retained X11 hierarchy');seen.add(item.id);revisions.push([item.id,item.spatialIdentity?.token,item.spatialRevision||0,item.viewable]);if(item===window||item.viewable)pending.push(...(children.get(item.id)||[]));}
            revisions.sort((a,b)=>a[0]-b[0]);
            const title=this.shellTexts.get(window.id)?.find(overlay=>overlay.role==='window-title')?.text||'Application';
            result.push({id:window.id,producerToken:window.spatialIdentity.token,revision:JSON.stringify(revisions),title:String(title).slice(0,512),
                frame:{x:window.x-window.borderWidth,y:window.y-window.borderWidth,width,height},available:window.viewable===true,
                active:!this.nativeCanvasWindow.foregroundActive&&window.activeWindow===true,dialog:window.ownedWindow===true});
        }
        const metadata=this.spatialX11Metadata;
        if(!metadata)return result;
        const current=new Map([...metadata].filter(([id,m])=>this.windows.get(id)?.spatialIdentity?.token===m.frameToken&&this.windows.get(m.client)?.spatialIdentity?.token===m.clientToken));
        const main=result.filter(w=>{const m=current.get(w.id);return !m||!m.rootClient||m.rootClient===m.client;});
        const groups=new Map(main.map(w=>[w.id,[w.id]]));
        for(const [id,m]of current){
            if(!m.viewable||m.minimized||m.parked||!this.windows.get(id)?.viewable)continue;
            let candidates;
            if(m.overrideRedirect)candidates=main.filter(w=>w.available&&current.get(w.id)?.owner===m.owner&&current.get(w.id)?.rootClient===current.get(w.id)?.client);
            else if(m.rootClient&&m.rootClient!==m.client)candidates=main.filter(w=>current.get(w.id)?.client===m.rootClient&&current.get(w.id)?.owner===m.owner);
            else continue;
            if(candidates.length===1)groups.get(candidates[0].id).push(id);
        }
        return main.map(w=>{
            const m=current.get(w.id),ids=groups.get(w.id),members=ids.map(id=>this.windows.get(id));
            const x=Math.min(...members.map(v=>v.x-v.borderWidth)),y=Math.min(...members.map(v=>v.y-v.borderWidth));
            const right=Math.max(...members.map(v=>v.x+v.width+v.borderWidth)),bottom=Math.max(...members.map(v=>v.y+v.height+v.borderWidth));
            if(right-x>16384||bottom-y>16384)return null;
            const revisions=[w.revision,ids.map(id=>{const v=this.windows.get(id);return[id,v.spatialIdentity.token,v.spatialRevision,v.viewable];}),m];
            // Include descendants of all composited transient/popup roots.
            const pending=members.slice(1),seen=new Set();while(pending.length){const v=pending.pop();if(seen.has(v.id)||seen.size>=4096)throw Error('Invalid X11 composition');seen.add(v.id);revisions.push([v.id,v.spatialRevision,v.viewable]);if(v.viewable)pending.push(...(children.get(v.id)||[]));}
            return {...w,client:m?.client??null,rootClient:m?.rootClient??null,ownerConnection:m?.owner??null,launchId:m?.launchId??null,nativeProcessId:m?.nativeProcessId??null,
                workspace:m?.workspace??null,minimized:m?.minimized??false,available:w.available&&(!m||m.viewable&&!m.minimized&&!m.parked),compositionIds:ids,
                frame:{x,y,width:right-x,height:bottom-y},revision:JSON.stringify(revisions),active:w.active||members.some(v=>v.activeWindow)};
        }).filter(Boolean);
    }
    ownsSpatialX11Window(owner,{presentable=false}={}) {
        return !!owner&&this.spatialX11Windows().some(w=>w.id===owner.id&&w.producerToken===owner.producerToken&&(!presentable||w.available));
    }
    paintSpatialX11Window(owner,context,rect) {
        const snapshot=this.spatialX11Windows().find(w=>w.id===owner?.id&&w.producerToken===owner?.producerToken&&w.available);
        if(!snapshot)return false;
        const window=this.windows.get(snapshot.id),children=new Map();for(const child of this.windows.values()){if(child.id===child.parent)continue;const list=children.get(child.parent)||[];list.push(child);children.set(child.parent,list);}
        for(const list of children.values())list.sort((a,b)=>a.stackIndex-b.stackIndex);
        const scale=Math.min(rect.width/snapshot.frame.width,rect.height/snapshot.frame.height);
        context.save();try{
            context.translate(rect.x+(rect.width-snapshot.frame.width*scale)/2,rect.y+(rect.height-snapshot.frame.height*scale)/2);context.scale(scale,scale);
            context.beginPath();context.rect(0,0,snapshot.frame.width,snapshot.frame.height);context.clip();
            const seen=new Set();const draw=(item,depth)=>{
                if(depth>64||seen.has(item.id)||seen.size>=4096)throw Error('Invalid retained X11 hierarchy');seen.add(item.id);
                context.save();try{context.beginPath();context.rect(0,0,item.width,item.height);context.clip();const surface=item.scaledUploadSurface||item.surface;
                    if(surface)context.drawImage(surface,0,0,item.width,item.height);else{context.fillStyle=pixelColor(item.backgroundPixel);context.fillRect(0,0,item.width,item.height);}
                    this._renderShellTextInto(context,item);
                    for(const child of children.get(item.id)||[]){if(!child.viewable||child.inputOnly||child.shellLayer)continue;context.save();try{context.translate(child.x,child.y);draw(child,depth+1);}finally{context.restore();}}
                }finally{context.restore();}
            };for(const id of (snapshot.compositionIds||[window.id]).slice().sort((a,b)=>this.windows.get(a).stackIndex-this.windows.get(b).stackIndex)){const item=this.windows.get(id);context.save();try{context.translate(item.x-snapshot.frame.x,item.y-snapshot.frame.y);draw(item,0);}finally{context.restore();}}return true;
        }finally{context.restore();}
    }
    _renderX11TaskPreview(targetId, targetContext, rect) {
        let target = this.windows.get(targetId);
        const root = [...this.windows.values()].find(window =>
            window.parent === window.id
        );
        if (!target || !root) return false;
        for (let depth = 0; depth < 64; depth += 1) {
            const parent = this.windows.get(target.parent);
            if (!parent || parent.id === root.id) break;
            target = parent;
        }
        if (!target.surface || target.width < 1 || target.height < 1) {
            return false;
        }
        const surface = this.canvasFactory(target.width, target.height);
        surface.width = target.width;
        surface.height = target.height;
        const context = surface.getContext("2d", { alpha: false });
        if (!context) return false;
        const children = new Map();
        for (const candidate of this.windows.values()) {
            const siblings = children.get(candidate.parent) || [];
            siblings.push(candidate);
            children.set(candidate.parent, siblings);
        }
        for (const siblings of children.values()) {
            siblings.sort((left, right) => left.stackIndex - right.stackIndex);
        }
        this._drawRetainedPreviewWindow(context, target, children);
        const scale = Math.min(
            rect.width / target.width,
            rect.height / target.height,
        );
        const drawWidth = target.width * scale;
        const drawHeight = target.height * scale;
        targetContext.drawImage(
            surface,
            0,
            0,
            target.width,
            target.height,
            rect.x + (rect.width - drawWidth) / 2,
            rect.y + (rect.height - drawHeight) / 2,
            drawWidth,
            drawHeight,
        );
        surface.close?.();
        return true;
    }

    _drawRetainedPreviewWindow(context, window, children) {
        const surface = window.scaledUploadSurface || window.surface;
        if (surface) {
            context.drawImage(surface, 0, 0, window.width, window.height);
        } else {
            context.fillStyle = pixelColor(window.backgroundPixel);
            context.fillRect(0, 0, window.width, window.height);
        }
        this._renderShellTextInto(context, window);
        for (const child of children.get(window.id) || []) {
            if (!child.viewable || child.inputOnly || child.shellLayer) continue;
            context.save();
            context.translate(child.x, child.y);
            this._drawRetainedPreviewWindow(context, child, children);
            context.restore();
        }
    }

    _renderShellTextInto(context, window) {
        const previous = this.context;
        this.context = context;
        try {
            const themedChrome=this.nativeCanvasWindow.snapshot?.chrome;
            this.themeSkins ||= new RiceThemeSkinRenderer(()=>{this.damaged=true;this.schedule();});
            const role=window.shellLayer&&![RICEWM_MENU_ID,RICEWM_NOTIFICATION_ID].includes(window.id)?'dock':null;
            if(role)this.themeSkins.paint(context,{x:0,y:0,width:window.width,height:window.height},themedChrome,role);
            this._renderShellText(window);
        } finally {
            this.context = previous;
        }
    }

    _roundedRect(context, rect, radius) {
        const r = Math.max(0, Math.min(radius, rect.width / 2, rect.height / 2));
        context.beginPath();
        context.moveTo(rect.x + r, rect.y);
        context.arcTo(rect.x + rect.width, rect.y, rect.x + rect.width, rect.y + r, r);
        context.arcTo(
            rect.x + rect.width,
            rect.y + rect.height,
            rect.x + rect.width - r,
            rect.y + rect.height,
            r,
        );
        context.arcTo(rect.x, rect.y + rect.height, rect.x, rect.y + rect.height - r, r);
        context.arcTo(rect.x, rect.y, rect.x + r, rect.y, r);
        context.closePath();
    }

    _renderNativeChrome(root, surfaceId) {
        const taskbar = this.windows.get(RICEWM_TASKBAR_ID);
        const dockMenu = this.windows.get(RICEWM_MENU_ID);
        if (surfaceId === RICEWM_MENU_ID && this.dockStyle === "Compact dock" && dockMenu?.viewable && (dockMenu.backgroundPixel >>> 31) && taskbar?.width > taskbar?.height) {
            const x = dockMenu.backgroundPixel & 0xffff;
            const above = dockMenu.y < taskbar.y;
            const y = above ? dockMenu.y+dockMenu.height : dockMenu.y;
            this.context.save(); this.context.fillStyle = pixelColor(this.nativeCanvasWindow.snapshot?.chrome?.menu?.[0] || this.nativeCanvasWindow.snapshot?.shell?.embeddedColor || 0x20232b);
            this.context.beginPath();this.context.moveTo(x-9,y);this.context.lineTo(x, y+(above?10:-10));this.context.lineTo(x+9,y);this.context.closePath();this.context.fill();this.context.restore();
        }
        if (surfaceId === RICEWM_TASKBAR_ID && taskbar?.viewable && taskbar.parent === root.id && this.dockStyle !== "Compact dock") {
            // Match the native shell's compact tray policy. These overlays are
            // painted independently of the native tray, including its CPU bay.
            if (root.width >= 700 && root.height >= 500) {
                this._renderTrayLauncherIcons(taskbar);
                this._renderWorkspaceStorageIndicator(taskbar);
                this._renderSystemControls(taskbar, "taskbar");
            }
            if (this.overviewDashVisible) this._renderQuickApplicationIcons(taskbar);
            const redmond98 = this.nativeCanvasWindow.snapshot?.chrome?.redmond98 === true;
            const signal = this.nativeCanvasWindow.snapshot?.shell?.surface?.frame === 'bracket';
            if ((redmond98 || signal) && this.nativeCanvasWindow.snapshot?.shell?.launcherVisible !== false && this.overviewDashVisible &&
                this.launcherAppearance !== "Text only") {
                this.nativeCanvasWindow.iconRenderer.renderBuiltInNamed({
                    icon: "ricewm-logo",
                    rect: riceCanvasTaskbarLogoRect(taskbar, redmond98),
                    color: signal ? [255,255,255,255] : [0, 0, 0, 255],
                });
            } else if (this.shellLogo && this.nativeCanvasWindow.snapshot?.shell?.launcherVisible !== false && this.overviewDashVisible &&
                this.launcherAppearance !== "Text only") {
                const rect = riceCanvasTaskbarLogoRect(taskbar);
                this.context.drawImage(
                    this.shellLogo,
                    rect.x,
                    rect.y,
                    rect.width,
                    rect.height,
                );
            }
        }
        const panel = this.windows.get(RICEWM_GNOME_PANEL_ID);
        if (surfaceId === RICEWM_GNOME_PANEL_ID && panel?.viewable && panel.parent === root.id) {
            this.topbarController?.refresh();
            if (!topbarRows(this).length) {
                this._renderMailIndicator(panel);
                this._renderSystemControls(panel, "top");
            }
            this.shellAccessoryPainter?.(this.context, {
                panel,
                trailingEdge: this._mailIndicatorRect(panel).x,
                chrome: this.nativeCanvasWindow.snapshot?.chrome || null,
            });
        }
    }

    _renderSystemControls(surface, placement, rect = null) {
        const controls = this.systemControlsLayout;
        const top = placement === "top";
        if (
            controls?.enabled !== true ||
            controls.placement !== placement ||
            (!top && surface.width <= surface.height)
        ) return;
        const width = controls.batteryPercentage === null ? 99 : 143;
        const x = rect?.x ?? surface.x + Math.max(0, surface.width - width - 7);
        const y = rect ? rect.y + (rect.height-24)/2 : surface.y + (top ? 3 : 4);
        const connected = controls.ethernet || controls.wifi;
        const foreground = [244, 244, 244, 255];
        const inactive = [128, 128, 128, 255];
        const icons = [
            { icon: "network", offset: 11, active: connected },
            {
                icon: controls.muted ? "audio-muted" : "audio-volume",
                offset: 43,
                active: controls.audio,
            },
            {
                icon: "logout",
                offset: controls.batteryPercentage === null ? 71 : 117,
                active: true,
            },
        ];
        for (const icon of icons) {
            this.nativeCanvasWindow.iconRenderer.renderNamed({
                icon: icon.icon,
                rect: { x: x + icon.offset, y: y + 3, width: 20, height: 18 },
                color: icon.active ? foreground : inactive,
            });
        }
        if (!connected) {
            this.context.save();
            this.context.strokeStyle = "rgba(244,244,244,.88)";
            this.context.lineWidth = 2;
            this.context.beginPath();
            this.context.moveTo(x + 13, y + 5);
            this.context.lineTo(x + 27, y + 19);
            this.context.stroke();
            this.context.restore();
        }
    }

    _mailIndicatorRect(panel) {
        if (!this.mailModel.configured) return {x: panel.x + panel.width, y: panel.y, width: 0, height: 0};
        const rows=topbarRows(this);
        if(rows.length){const item=rows.find(r=>r.key==="@mail");return item?.width>0?{...item,y:item.y+(item.height-24)/2,height:24}:{x:-100,y:-100,width:0,height:0};}
        const controls = this.systemControlsLayout;
        const nativeTopControls = controls?.enabled === true && controls.placement === "top";
        const controlsWidth = controls?.batteryPercentage === null ? 99 : 143;
        const reservedRight = nativeTopControls ? 7 + controlsWidth + 14 : 94;
        return {
            x: panel.x + Math.max(0, panel.width - reservedRight - 48),
            y: panel.y + Math.max(1, (panel.height - 25) / 2),
            width: 48,
            height: Math.min(25, Math.max(20, panel.height - 2)),
        };
    }

    _renderMailIndicator(panel, rect = this._mailIndicatorRect(panel), iconOverride = null, appearance = null) {
        const context = this.context;
        if (!this.mailModel.configured) return;
        const unread = this.mailModel.unread;
        context.save();
        try {
            if (!appearance || appearance.background) {
            this._roundedRect(context, rect, appearance?.radius ?? rect.height / 2);
            context.fillStyle = appearance?.background ? `rgba(${appearance.background.slice(0,3).join(",")},${appearance.background[3]/255})` : unread > 0
                ? "rgba(232,71,22,.94)"
                : "rgba(255,255,255,.10)";
            context.fill();
            }
            this.nativeCanvasWindow.iconRenderer.renderNamed({
                icon: iconOverride || (unread > 0 ? "mail-unread" : "mail-read"),
                rect: {
                    x: rect.x + 6,
                    y: rect.y + 5,
                    width: 16,
                    height: 15,
                },
                color: appearance?.foreground || [255, 255, 255, 255],
                hovered: appearance?.hovered === true,
                hoverBackground: appearance?.background,
            });
            context.fillStyle = appearance?.foreground ? `rgba(${appearance.foreground.slice(0,3).join(",")},${appearance.foreground[3]/255})` : "#ffffff";
            context.font = '700 11px "RiceWM Sans", "RiceWM CJK", system-ui, sans-serif';
            context.textAlign = "center";
            context.textBaseline = "middle";
            context.fillText(
                unread > 99 ? "99+" : String(unread),
                rect.x + 34,
                rect.y + rect.height / 2,
            );
        } finally {
            context.restore();
        }
    }

    _renderFocusOverlay(root) {
        const shell = this.nativeCanvasWindow.snapshot?.shell;
        if (shell?.focusOverlay !== true) return;
        const native = this.nativeCanvasWindow.snapshots.findLast(snapshot =>
            snapshot.role !== "desktop" &&
            snapshot.layerVisible !== false &&
            snapshot.active &&
            snapshot.frame
        );
        const x11 = native ? null : [...this.windows.values()].findLast(window =>
            window.viewable &&
            !window.inputOnly &&
            !window.desktopLayer &&
            !window.shellLayer &&
            window.activeWindow
        );
        const frame = native?.frame || (x11 ? {
            x: x11.x - x11.borderWidth,
            y: x11.y - x11.borderWidth,
            width: x11.width + x11.borderWidth * 2,
            height: x11.height + x11.borderWidth * 2,
        } : null);
        if (native?.frame) {
            const chrome = native.chrome || {};
            const titlebar = nativeTitlebarLayout(
                native.frame,
                native.title,
                native.titleTools,
                chrome.windowDecorations,
            );
            this.context.save();
            try {
                // A compact titlebar occupies only its short tab. Clip by the
                // actual silhouette so the empty band beside it is dimmed too;
                // excluding the frame's bounding rectangle left that band as
                // a bright, window-wide block.
                this.context.beginPath();
                this.context.rect(0, 0, root.width, root.height);
                appendWindowSilhouette(
                    this.context,
                    native.frame,
                    chrome,
                    titlebar,
                );
                this.context.clip("evenodd");
                this.context.fillStyle = "rgba(8, 10, 14, 0.46)";
                this.context.fillRect(0, 0, root.width, root.height);
            } finally {
                this.context.restore();
            }
            return;
        }
        const rectangles = riceFocusOverlayRects(root, frame);
        if (rectangles.length === 0) return;
        this.context.save();
        try {
            this.context.fillStyle = "rgba(8, 10, 14, 0.46)";
            for (const rectangle of rectangles) {
                this.context.fillRect(
                    rectangle.x,
                    rectangle.y,
                    rectangle.width,
                    rectangle.height,
                );
            }
        } finally {
            this.context.restore();
        }
    }

    _renderTrayLauncherIcons(taskbar) {
        if (taskbar.height > taskbar.width) {
            const items = this.overviewWorkspacesVisible
                ? this.trayItems
                : this.trayItems.filter(item => item?.type !== "workspaces");
            for (const { item, top } of riceCanvasVerticalTrayLayout(
                taskbar,
                items,
                this.workspaceNames.length,
            )) {
                if (
                    item?.type === "launcher" &&
                    typeof item.icon === "string" &&
                    /^#[0-9a-f]{6}$/i.test(item.color)
                ) {
                    const packed = Number.parseInt(item.color.slice(1), 16);
                    this.nativeCanvasWindow.iconRenderer.renderNamed({
                        icon: item.icon,
                        rect: {
                            x: taskbar.x + Math.max(0, Math.floor((taskbar.width - 19) / 2)),
                            y: taskbar.y + top + 4,
                            width: 19,
                            height: 18,
                        },
                        color: [
                            (packed >>> 16) & 0xff,
                            (packed >>> 8) & 0xff,
                            packed & 0xff,
                            255,
                        ],
                    });
                }
            }
            return;
        }
        let cursor = taskbar.width - 7;
        const widths = {
            clock: 70,
            workspaces: 96,
            performance: 66,
            sound: 25,
            launcher: 25,
            connectivity: 25,
        };
        for (let index = this.trayItems.length - 1; index >= 0; index -= 1) {
            const item = this.trayItems[index];
            const width = widths[item?.type] || 0;
            cursor = Math.max(0, cursor - width);
            if (
                item?.type === "launcher" &&
                typeof item.icon === "string" &&
                /^#[0-9a-f]{6}$/i.test(item.color)
            ) {
                const packed = Number.parseInt(item.color.slice(1), 16);
                this.nativeCanvasWindow.iconRenderer.renderNamed({
                    icon: item.icon,
                    rect: {
                        x: taskbar.x + cursor + 3,
                        y: taskbar.y + 7,
                        width: 19,
                        height: 18,
                    },
                    color: [
                        (packed >>> 16) & 0xff,
                        (packed >>> 8) & 0xff,
                        packed & 0xff,
                        255,
                    ],
                });
            }
            cursor = Math.max(0, cursor - 4);
        }
    }

    _renderWorkspaceStorageIndicator(taskbar) {
        const state = this.workspaceStorageState;
        if (!state) return;
        const vertical = taskbar.height > taskbar.width;
        const visibleTrayItems = this.overviewWorkspacesVisible
            ? this.trayItems
            : this.trayItems.filter(item => item?.type !== "workspaces");
        const verticalRect = vertical
            ? riceCanvasVerticalStorageIndicatorRect(
                taskbar,
                visibleTrayItems,
                this.workspaceNames.length,
            )
            : null;
        const redmond98 =
            this.nativeCanvasWindow.snapshot?.chrome?.redmond98 === true;
        const widths = {
            clock: 70,
            workspaces: 96,
            performance: 66,
            sound: 25,
            connectivity: 25,
            launcher: 25,
        };
        let cursor = taskbar.width - 7;
        for (let index = this.trayItems.length - 1; index >= 0; index -= 1) {
            cursor = Math.max(0, cursor - (widths[this.trayItems[index]?.type] || 0));
            cursor = Math.max(0, cursor - 4);
        }
        let x = verticalRect?.x ?? taskbar.x + Math.max(34, cursor - 23);
        if (redmond98 && !vertical) {
            // Share the first bay of the classic performance monitor instead
            // of painting a detached cylinder across its bevel.
            let performanceCursor = taskbar.width - 7;
            for (let index = this.trayItems.length - 1; index >= 0; index -= 1) {
                const item = this.trayItems[index];
                performanceCursor = Math.max(
                    0,
                    performanceCursor - (widths[item?.type] || 0),
                );
                if (item?.type === "performance") {
                    x = taskbar.x + performanceCursor + 2;
                    break;
                }
                performanceCursor = Math.max(0, performanceCursor - 4);
            }
        }
        const y = verticalRect?.y ??
            taskbar.y + Math.max(5, Math.floor((taskbar.height - 19) / 2));
        const attention = state.permission === "prompt" ||
            state.permission === "denied" ||
            /attention|delayed|failed|paused|recovery/i.test(
                String(state.cloudStatus || ""),
            );
        const active = /preparing|queued|restoring|syncing/i.test(
            String(state.cloudStatus || ""),
        );
        const local = state.backend === "local-directory-v2";
        const color = attention
            ? "#fbbf24"
            : local
                ? "#34d399"
                : "rgba(226, 232, 240, 0.76)";
        this.context.save();
        try {
            this.context.strokeStyle = color;
            this.context.lineWidth = 1.4;
            this.context.beginPath();
            this.context.ellipse(x + 8, y + 4, 6.5, 2.7, 0, 0, Math.PI * 2);
            this.context.moveTo(x + 1.5, y + 4);
            this.context.lineTo(x + 1.5, y + 13);
            this.context.bezierCurveTo(x + 2, y + 16, x + 14, y + 16, x + 14.5, y + 13);
            this.context.lineTo(x + 14.5, y + 4);
            this.context.moveTo(x + 1.5, y + 8.5);
            this.context.bezierCurveTo(x + 2, y + 11.5, x + 14, y + 11.5, x + 14.5, y + 8.5);
            this.context.stroke();

        } finally {
            this.context.restore();
        }
    }

    _renderQuickApplicationIcons(taskbar) {
        const redmond98 = this.nativeCanvasWindow.snapshot?.chrome?.redmond98 === true;
        const shell = this.nativeCanvasWindow.snapshot?.shell || {};
        // Match DisplayServer::wm_start_button_width, including its UTF-8
        // label measurement, so icons stay inside the native shortcut targets.
        const appearance = shell.launcherAppearance ?? this.launcherAppearance;
        const labelBytes = new TextEncoder().encode(shell.launcherText ?? "RiceWM").length;
        const startWidth = shell.launcherVisible === false ? 0
            : redmond98 ? 59
            : appearance === "Icon only" ? 36
            : appearance === "Text only" ? Math.max(44, Math.min(200, labelBytes * 7 + 20))
            : Math.max(112, Math.min(200, labelBytes * 7 + 44));
        const quickLaunchX = startWidth + 9;
        const vertical = taskbar.height > taskbar.width;
        for (const shortcut of this.quickApplicationIcons || []) {
            const renderer = this.nativeCanvasWindow.iconRenderer;
            const render = shortcut.iconName === "desktop" && !redmond98
                ? renderer.renderBuiltInNamed : renderer.renderNamed;
            render.call(renderer, {
                icon: redmond98 && shortcut.iconName === "desktop" ? "show-desktop" : shortcut.iconName,
                ...(shortcut.applicationId?{applicationId:shortcut.applicationId}:{}),
                rect: {
                    x: vertical
                        ? taskbar.x + Math.max(0, Math.floor((taskbar.width - 22) / 2))
                        : taskbar.x + quickLaunchX + shortcut.index * 28,
                    y: vertical
                        ? taskbar.y + 38 + shortcut.index * 28
                        : taskbar.y + 5,
                    width: 22,
                    height: 22,
                },
                // Use theme ink, just like the menu; white is an explicit override.
                color: [0, 0, 0, 255],
            });
        }
    }

    _allocateSurface(window, existing) {
        existing?.scaledUploadSurface?.close?.();
        window.scaledUploadSurface = null;
        window.scaledUploadContext = null;
        window.opaqueGpuFrame = false;
        const surface = this.canvasFactory(window.width, window.height);
        surface.width = window.width;
        surface.height = window.height;
        const context = surface.getContext("2d", {
            alpha: window.depth === 32,
        });
        if (!context) {
            throw new TypeError("RiceWM retained surface requires Canvas2D");
        }
        if (window.depth !== 32 || (window.backgroundPixel >>> 24) !== 0) {
            context.fillStyle = pixelColor(window.backgroundPixel);
            context.fillRect(0, 0, window.width, window.height);
        }
        // Hidden shell surfaces can have no pixels while a theme changes.
        // Canvas2D rejects a zero-sized source when they become visible again.
        if (existing?.surface?.width > 0 && existing.surface.height > 0) {
            context.drawImage(existing.surface, 0, 0);
        }
        window.surface = surface;
        window.surfaceContext = context;
        if (window.id === RICEWM_GNOME_PANEL_ID) {
            const data = new Uint8ClampedArray(window.width * window.height * 4);
            const color = [(window.backgroundPixel >>> 16) & 255,
                (window.backgroundPixel >>> 8) & 255, window.backgroundPixel & 255, 255];
            for (let offset = 0; offset < data.length; offset += 4) data.set(color, offset);
            const old = existing?.topbarPixels;
            if (old) {
                const width = Math.min(old.width, window.width);
                for (let y = 0; y < Math.min(old.height, window.height); y++) {
                    data.set(old.data.subarray(y * old.width * 4, (y * old.width + width) * 4), y * window.width * 4);
                }
            }
            window.topbarPixels = {data, width: window.width, height: window.height};
        }
    }

    _upload(upload) {
        const window = this.windows.get(upload.id);
        if (!window || window.inputOnly) { upload.bitmap?.close?.(); return; }
        if (upload.bitmap) {
            if (upload.x !== 0 || upload.y !== 0 || upload.targetWidth !== window.width ||
                upload.targetHeight !== window.height || upload.bitmap.width !== upload.width ||
                upload.bitmap.height !== upload.height) {
                upload.bitmap.close?.();
                throw new RangeError("invalid RiceWM GPU surface upload");
            }
            window.scaledUploadSurface?.close?.();
            window.scaledUploadSurface = upload.bitmap;
            window.scaledUploadContext = null;
            window.opaqueGpuFrame = upload.opaque === true;
            window.materialAlpha = false;
            window.externalFrameSequence = (window.externalFrameSequence || 0) + 1;
            window.spatialRevision=(window.spatialRevision||0)+1;
            const rect = this._sceneWindowRect(window, this.windows, null);
            if (rect) this._markNativeDamage([rect]);
            return;
        }
        if (!window.surfaceContext) {
            this._allocateSurface(window, null);
        }
        window.opaqueGpuFrame = false;
        const pixels = upload.pixels instanceof Uint8Array
            ? upload.pixels
            : new Uint8Array(upload.pixels);
        if (pixels.byteLength !== upload.width * upload.height * 4) {
            throw new RangeError("invalid RiceWM RGBA surface upload");
        }
        if (window.id === RICEWM_GNOME_PANEL_ID && window.topbarPixels) {
            // Retain partial clock/icon updates as well as complete theme paints.
            for (let y = 0; y < upload.height; y++) {
                window.topbarPixels.data.set(pixels.subarray(y * upload.width * 4,
                    (y + 1) * upload.width * 4), ((upload.y + y) * window.width + upload.x) * 4);
            }
        }
        if (upload.id === RICEWM_NOTIFICATION_ID) {
            window.matrixNotificationKind =
                riceMatrixNotificationKindFromPixels(
                    pixels,
                    window.matrixNotificationKind,
                );
        }
        let materialAlpha = false;
        // Depth-24 X11 drawables are opaque. Their uploaded alpha bytes are
        // synthesized as 255, so scanning the full frame cannot change the
        // material-alpha result.
        if (window.depth === 32) {
            for (let offset = 3; offset < pixels.byteLength; offset += 4) {
                if (pixels[offset] > 0 && pixels[offset] < 255) {
                    materialAlpha = true;
                    break;
                }
            }
        }
        const previousMaterialAlpha = window.materialAlpha;
        const scaledToSurface = upload.scaleToSurface === true &&
            upload.x === 0 && upload.y === 0 &&
            upload.targetWidth === window.width &&
            upload.targetHeight === window.height;
        const fullSurface = scaledToSurface || (
            upload.x === 0 && upload.y === 0 &&
            upload.width === window.width && upload.height === window.height
        );
        window.materialAlpha = fullSurface
            ? materialAlpha
            : window.materialAlpha || materialAlpha;
        const image = typeof globalThis.ImageData === "function"
            ? new globalThis.ImageData(
                new Uint8ClampedArray(
                    pixels.buffer,
                    pixels.byteOffset,
                    pixels.byteLength,
                ),
                upload.width,
                upload.height,
            )
            : window.surfaceContext.createImageData(
                upload.width,
                upload.height,
            );
        if (typeof globalThis.ImageData !== "function") {
            image.data.set(pixels);
        }
        if (scaledToSurface) {
            let source = window.scaledUploadSurface;
            if (!source || !window.scaledUploadContext || source.width !== upload.width ||
                source.height !== upload.height) {
                source?.close?.();
                source = this.canvasFactory(upload.width, upload.height);
                source.width = upload.width;
                source.height = upload.height;
                window.scaledUploadSurface = source;
                window.scaledUploadContext = source.getContext("2d", {
                    alpha: window.depth === 32,
                });
            }
            if (!window.scaledUploadContext) {
                throw new TypeError("scaled RiceWM upload requires Canvas2D");
            }
            window.scaledUploadContext.putImageData(image, 0, 0);
        } else {
            if (window.scaledUploadSurface) {
                window.surfaceContext.imageSmoothingEnabled = true;
                window.surfaceContext.imageSmoothingQuality = "low";
                window.surfaceContext.drawImage(
                    window.scaledUploadSurface,
                    0,
                    0,
                    window.width,
                    window.height,
                );
                window.scaledUploadSurface.close?.();
                window.scaledUploadSurface = null;
                window.scaledUploadContext = null;
            }
            window.surfaceContext.putImageData(image, upload.x, upload.y);
        }
        const damage = scaledToSurface
            ? { x: 0, y: 0, width: window.width, height: window.height }
            : upload;
        const rect = this._sceneWindowRect(
            window,
            this.windows,
            previousMaterialAlpha === window.materialAlpha ? damage : null,
        );
        if (rect) this._markNativeDamage([rect]);
        window.spatialRevision=(window.spatialRevision||0)+1;
    }
}

function nativeMediaRequests(snapshot) {
    const all = new Set();
    const hot = new Set();
    const windows = Array.isArray(snapshot?.windows)
        ? snapshot.windows
        : snapshot
            ? [snapshot]
            : [];
    const activeApp = String(snapshot?.activeApp ?? snapshot?.app ?? "");
    for (const window of windows) {
        const prioritized = window?.role === "desktop" ||
            String(window?.app ?? "") === activeApp;
        if (
            window?.role === "desktop" &&
            window.desktopBackground?.type === "Video" &&
            String(window.desktopBackground.videoSession || "0") !== "0"
        ) {
            const request = String(window.desktopBackground.videoSession);
            all.add(request);
            hot.add(request);
        }
        if(window?.desktopBackground?.scene?.request){const request=String(window.desktopBackground.scene.request);all.add(request);hot.add(request);}
        for (const command of window?.commands || []) {
            if (command?.type !== "media" || command.request === undefined) {
                continue;
            }
            const request = String(command.request);
            all.add(request);
            if (prioritized) hot.add(request);
        }
    }
    return { all, hot };
}

function equalSets(left, right) {
    if (left.size !== right.size) return false;
    for (const value of left) {
        if (!right.has(value)) return false;
    }
    return true;
}
