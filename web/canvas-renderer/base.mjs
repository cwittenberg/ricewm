import {t, riceLocale} from '../rice-i18n.mjs';
import {RiceEffectRenderer} from '../rice-effect-renderer.mjs';
import * as CanvasRendererShared from "./shared.mjs";
const {MAX_NATIVE_MEDIA_REQUEST_TRACKING,MAX_NATIVE_MEDIA_SURFACES,RICEWM_FIRST_MANAGED_FRAME_ID,RICEWM_GNOME_BACKGROUND_ID,RICEWM_GNOME_PANEL_ID,RICEWM_MANAGED_FRAME_ID_LIMIT,RICEWM_MENU_ID,RICEWM_NATIVE_PREVIEW_PREFIX,RICEWM_NOTIFICATION_ID,RICEWM_TASKBAR_ID,RICEWM_TASK_PREVIEW_ID,RICE_BUILTIN_MENU_ICONS,RICE_NATIVE_MEDIA_CACHE_MAX_BYTES,RICE_NATIVE_MENU_ICONS,RICE_SCENE_COMMAND_BYTES,RICE_SCENE_DAMAGE_WINDOW,RICE_SCENE_OWNED_WINDOW_STACK_FLAG,RICE_SCENE_REMOVE_WINDOW,RICE_SCENE_RESET,RICE_SCENE_RESIZABLE_WINDOW_STACK_FLAG,RICE_SCENE_STACK_INDEX_MASK,RICE_SCENE_UPSERT_WINDOW,ROOT_PARENT_SENTINEL,appendWedgedSurfacePath,defaultCancelFrame,defaultCanvasFactory,defaultRequestFrame,overviewEase,pixelColor,riceFocusOverlayRects,riceNativeMenuIconName,riceNativeTaskIconName,riceOverviewLayout,riceWindowTransitionOrigin,riceWmBarBorderPixels,sameSceneWindow,appendWindowSilhouette,RiceNativeCanvasWindow,FrostedIceMaterial,RiceGnomeEffectRenderer,riceIconName,NATIVE_ENTRYPOINTS,nativeEntrypointIconName,nativeEntrypointTitle,nativeTitlebarLayout} = CanvasRendererShared;
let spatialX11RendererEpoch = 0;
export class RiceCanvasRendererBase {
    get damaged() {
        return this._damaged;
    }

    set damaged(value) {
        this.terminalImageReplay = null;
        this.terminalImageOnlyDamage = false;
        this._damaged = Boolean(value);
        this.damageRects = value ? null : [];
    }

    constructor(
        canvas,
        {
            requestFrame = defaultRequestFrame,
            cancelFrame = defaultCancelFrame,
            onFrame = () => {},
            onNativeFault = () => {},
            onFault = () => {},
            onNativeMediaMissing = () => {},
            onNativeMediaRequestsChanged = () => {},
            canvasFactory = defaultCanvasFactory,
            shellLogo = null,
            trayItems = [],
            workspaceStorageState = null,
            terminalPreview = () => false,
            setTimer = globalThis.setTimeout.bind(globalThis),
            clearTimer = globalThis.clearTimeout.bind(globalThis),
            now = () => globalThis.performance?.now?.() || Date.now(),
            onDesktopIconAnimation = null,
            onNotificationAction = null,
            onGnomeEffectStatus = () => {},
            onVisualEffectStatus = () => {},
            gnomeEffectsHardwareOnly = true,
            doubleBuffer = true,
        } = {},
    ) {
        // Terminal portals are real DOM xterm surfaces below this canvas.
        // Native terminal draw commands punch transparent holes through the
        // canvas so xterm remains visible while shell menus/chrome painted
        // later in this renderer stay above it. An opaque backing context
        // turns clearRect() into black pixels and completely hides xterm.
        const context = canvas?.getContext?.("2d", { alpha: true });
        if (!context) {
            throw new TypeError("RiceWM requires a Canvas2D target");
        }
        this.canvas = canvas;
        this.context = context;
        this.compositionSurface = null;
        this.compositionContext = null;
        this.requestFrame = requestFrame;
        this.frameSchedulerIsAnimationFrame = requestFrame === defaultRequestFrame &&
            typeof globalThis.requestAnimationFrame === "function";
        this.cancelFrame = cancelFrame;
        this.onFrame = onFrame;
        this.onNativeFault = onNativeFault;
        this.onFault = onFault;
        this.onNativeMediaMissing = onNativeMediaMissing;
        this.onNativeMediaRequestsChanged = onNativeMediaRequestsChanged;
        this.canvasFactory = canvasFactory;
        this.doubleBuffer = doubleBuffer !== false;
        this.windows = new Map();
        this.shellTexts = new Map();
        this.shellTextsSignature = "";
        this.menuApplicationIcons = new Map();
        this.menuSourceIcons = new Map();
        this.menuColorArtworkLabels = new Set();
        this.menuBuiltinIcons = new Map();
        this.applicationCatalog = [];
        this.applicationIconsSignature = "";
        this.pointerPosition = null;
        this.frameHandle = null;
        this._damaged = false;
        this.damageRects = [];
        this.shellLogo = shellLogo;
        this.trayItems = Array.isArray(trayItems) ? [...trayItems] : [];
        this.workspaceStorageState = workspaceStorageState &&
            typeof workspaceStorageState === "object"
            ? { ...workspaceStorageState }
            : null;
        this.terminalPreview = terminalPreview;
        this.setTimer = setTimer;
        this.clearTimer = clearTimer;
        this.now = now;
        this.onNotificationAction = typeof onNotificationAction === "function"
            ? onNotificationAction
            : null;
        this.nativeMediaSurfaces = new Map();
        this.nativeMediaWallpaperRequests = new Set();
        this.nativeMediaSurfaceBytes = new Map();
        this.nativeMediaBytes = 0;
        this.nativeMediaLiveRequests = new Set();
        this.nativeMediaHotRequests = new Set();
        this.nativeMediaRetiredRequests = new Set();
        this.nativeMediaRefreshGeneration = 0;
        this.nativeMediaRefreshAttempts = new Map();
        this.nativeMediaRefreshQueue = new Set();
        this.nativeMediaRefreshScheduled = false;
        this.nativeMediaStates = new Map();
        this.nativeMediaBlinkVisible = true;
        this.nativeMediaBlinkTimer = null;
        this.shellGlassBackdrop = null;
        this.frostedBackdropGeneration = 0;
        this.shellFrostedIceMaterial = new FrostedIceMaterial({
            canvasFactory,
            onStatus: onVisualEffectStatus,
            surface: "shell",
        });
        this.gnomeEffectRenderer = new RiceGnomeEffectRenderer({
            canvasFactory,
            hardwareOnly: gnomeEffectsHardwareOnly,
            onStatus: onGnomeEffectStatus,
        });
        this.customEffects = new RiceEffectRenderer({canvasFactory,now,setTimer,clearTimer,
            onStatus:onVisualEffectStatus,changed:()=>{this.damaged=true;this.schedule();}});
        this.windowTransitions = new Map();
        this.closingSceneWindows = new Map();
        this.overviewProgress = 0;
        this.overviewTarget = 0;
        this.overviewStartedAt = 0;
        this.overviewStartedProgress = 0;
        this.overviewItems = null;
        this.overviewLayout = [];
        this.overviewHover = null;
        this.overviewPointerActive = false;
        this.overviewCloseHover = null;
        this.overviewCloseRects = new Map();
        this.overviewSearchQuery = "";
        this.overviewSearchRect = null;
        this.overviewExternalResults = [];
        this.gnomeShellState = null;
        this.overviewSearchVisible = true;
        this.overviewDashVisible = true;
        this.overviewWorkspacesVisible = true;
        this.overviewWorkspaceScale = 0.1;
        this.calendarActive = false;
        this.calendarFromTopBar = false;
        this.calendarMonth = new Date();
        this.calendarMonth.setDate(1);
        this.calendarLayout = null;
        this.calendarModel = Object.freeze({ events: [] });
        this.mailModel = Object.freeze({ configured: false, unread: 0 });
        this.systemControlsLayout = Object.freeze({
            enabled: false,
            placement: "top",
            batteryPercentage: null,
            ethernet: false,
            wifi: false,
            bluetooth: false,
            audio: false,
            muted: false,
        });
        this.calendarSelectedDate = new Date();
        this.notificationHistory = [];
        this.notificationHistoryScroll = 0;
        this.notificationExpandedId = null;
        this.matrixNotificationBreatheTimer = null;
        this.workspaceNames = ["1", "2", "3", "4"];
        this.launcherAppearance = "Icon and text";
        this.workspaceTransition = null;
        this.workspaceTransitionDuration = 340;
        this.workspaceTransitionStyle = "slide";
        this.nativeCanvasWindow = new RiceNativeCanvasWindow(
            context,
            (damage, imageOnly) => {
                this._markNativeDamage(damage, imageOnly);
                this.schedule();
            },
            request => this._nativeMediaSurface(request),
            request => {
                const status = this.nativeMediaStates.get(String(request));
                return status
                    ? { ...status, blinkVisible: this.nativeMediaBlinkVisible }
                    : null;
            },
            fault => this.onNativeFault(fault),
            now,
            onDesktopIconAnimation,
            { canvasFactory, onVisualEffectStatus },
        );
        this.nativeCanvasWindow.paintCustomFrame=(context,native,root)=>{
            if(native.entrypoint==='rice-settings'||native.role==='desktop')return;
            const f=native.frame;
            this.customEffects.render(context,{width:root.width,height:root.height,target:'window-frame',
                rects:[{x:0,y:0,width:f.width,height:f.titleHeight}],reducedMotion:native.shell?.motionEnabled===false});
        };
        this.effectVisibilityListener=()=>{this.customEffects.pause(globalThis.document?.hidden===true);this.damaged=true;this.schedule();};
        globalThis.document?.addEventListener('visibilitychange',this.effectVisibilityListener);
        if (!this.shellLogo && typeof globalThis.Image === "function") {
            const logo = new globalThis.Image();
            logo.addEventListener("load", () => {
                this.shellLogo = logo;
                this.damaged = true;
                this.schedule();
            }, { once: true });
            const logoUrl = new URL(
                "../assets/img/ricewm-logo.svg",
                import.meta.url,
            );
            const assetVersion = new URL(
                import.meta.url,
            ).searchParams.get("asset_version");
            if (assetVersion) {
                logoUrl.searchParams.set(
                    "asset_version",
                    assetVersion,
                );
            }
            logo.src = logoUrl.href;
        }
    }

    apply(commands, uploads = [], shellTexts = null) {
        this.spatialX11Epoch ||= ++spatialX11RendererEpoch;
        const bytes = commands instanceof Uint8Array
            ? commands
            : new Uint8Array(commands);
        if (bytes.byteLength % RICE_SCENE_COMMAND_BYTES !== 0) {
            throw new RangeError("unaligned RiceWM scene command stream");
        }
        const view = new DataView(
            bytes.buffer,
            bytes.byteOffset,
            bytes.byteLength,
        );
        let resetWindows = null;
        const damage = [];
        const changedWindows = new Set();
        const damagedSurfaces = [];
        for (
            let offset = 0;
            offset < bytes.byteLength;
            offset += RICE_SCENE_COMMAND_BYTES
        ) {
            const opcode = view.getUint8(offset);
            if (opcode === RICE_SCENE_RESET) {
                // A reset rebuilds stacking and parentage; it does not imply
                // that every X11 drawable lost its pixels.  Retain matching
                // canvases so a restack cannot flash blank or force a full
                // multi-megabyte desktop upload.
                resetWindows = this.windows;
                this.windows = new Map();
            } else if (opcode === RICE_SCENE_UPSERT_WINDOW) {
                const flags = view.getUint8(offset + 1);
                const id = view.getUint32(offset + 4, true);
                const existing = this.windows.get(id) || resetWindows?.get(id);
                const window = {
                    id,
                    spatialIdentity: existing?.spatialIdentity || Object.freeze({token:`x11-${this.spatialX11Epoch}-${this.spatialX11Generation=(this.spatialX11Generation||0)+1}`}),
                    spatialRevision: existing?.spatialRevision || 0,
                    depth: view.getUint8(offset + 2),
                    parent: view.getUint32(offset + 8, true),
                    x: view.getInt32(offset + 12, true),
                    y: view.getInt32(offset + 16, true),
                    width: view.getUint16(offset + 20, true),
                    height: view.getUint16(offset + 22, true),
                    borderWidth: view.getUint16(offset + 24, true),
                    stackIndex: view.getUint16(offset + 26, true) & RICE_SCENE_STACK_INDEX_MASK,
                    ownedWindow: (view.getUint16(offset + 26, true) &
                        RICE_SCENE_OWNED_WINDOW_STACK_FLAG) !== 0,
                    resizableWindow: (view.getUint16(offset + 26, true) &
                        RICE_SCENE_RESIZABLE_WINDOW_STACK_FLAG) !== 0,
                    backgroundPixel: view.getUint32(offset + 28, true),
                    viewable: (flags & 1) !== 0,
                    overrideRedirect: (flags & 2) !== 0,
                    inputOnly: (flags & 4) !== 0,
                    desktopLayer: (flags & 8) !== 0,
                    shellLayer: (flags & 16) !== 0,
                    backdropMaterial: (flags & 32) !== 0,
                    activeWindow: (flags & 64) !== 0,
                    maximizedWindow: (flags & 128) !== 0,
                    materialAlpha: existing?.materialAlpha || false,
                    surface: existing?.surface || null,
                    surfaceContext: existing?.surfaceContext || null,
                    topbarPixels: existing?.topbarPixels || null,
                    scaledUploadSurface: existing?.scaledUploadSurface || null,
                    scaledUploadContext: existing?.scaledUploadContext || null,
                    opaqueGpuFrame: existing?.opaqueGpuFrame === true,
                    externalFrameSequence: existing?.externalFrameSequence || 0,
                    matrixNotificationKind:
                        existing?.matrixNotificationKind || "info",
                };
                const shell=this.nativeCanvasWindow.snapshot?.shell;
                if(shell?.tileDragScrolling&&this.windowTransitions.get(id)?.direction==='layout')this.windowTransitions.delete(id);
                if(existing?.viewable&&window.viewable&&shell?.scrollingTiling&&shell.motionEnabled&&!shell.tileDragScrolling&&
                    !window.overrideRedirect&&!window.desktopLayer&&!window.shellLayer&&
                    (this.windows.get(window.parent)||resetWindows?.get(window.parent))?.parent===window.parent&&
                    (existing.x!==window.x||existing.y!==window.y)) {
                    const previous=this.windowTransitions.get(id),now=this.now();
                    const duration=Math.max(1,shell.motion?.definitions?.['window.layout']?.duration??320);
                    const t=previous?.direction==='layout'?Math.min(1,Math.max(0,(now-previous.startedAt)/duration)):1;
                    const remaining=(1-t)**4;
                    const from={x:existing.x+(previous?.direction==='layout'?(previous.from.x-previous.to.x)*remaining:0),
                        y:existing.y+(previous?.direction==='layout'?(previous.from.y-previous.to.y)*remaining:0)};
                    this.windowTransitions.set(id,{direction:'layout',startedAt:now,from,to:{x:window.x,y:window.y}});
                }
                if (!sameSceneWindow(existing, window)) {
                    const previousRect = this._sceneWindowRect(existing, resetWindows || this.windows);
                    if (previousRect) damage.push(previousRect);
                    changedWindows.add(id);
                }
                if (
                    !window.inputOnly &&
                    (
                        !window.surface ||
                        window.surface.width !== window.width ||
                        window.surface.height !== window.height ||
                        existing?.depth !== window.depth
                    )
                ) {
                    this._allocateSurface(window, existing);
                }
                if (
                    window.viewable &&
                    !existing?.viewable &&
                    !window.desktopLayer &&
                    window.id !== RICEWM_TASKBAR_ID &&
                    window.parent !== window.id &&
                    (this.windows.get(window.parent) || resetWindows?.get(window.parent))
                        ?.parent === window.parent
                ) {
                    this.closingSceneWindows.delete(id);
                    this.windowTransitions.set(id, {
                        startedAt: this.now(),
                        direction: "open",
                    });
                }
                if (!window.viewable && existing?.viewable && id === RICEWM_MENU_ID &&
                    this.nativeCanvasWindow.snapshot?.shell?.shellStyle === "Embedded" &&
                    this.nativeCanvasWindow.snapshot?.shell?.motionEnabled === true) {
                    this.closingSceneWindows.set(id, existing);
                    this.windowTransitions.set(id, {startedAt: this.now(), direction: "close"});
                }
                this.windows.set(id, window);
            } else if (opcode === RICE_SCENE_REMOVE_WINDOW) {
                const id = view.getUint32(offset + 4, true);
                const existing = this.windows.get(id) || resetWindows?.get(id);
                existing?.scaledUploadSurface?.close?.();
                const previousRect = this._sceneWindowRect(
                    existing,
                    this.windows.has(id) ? this.windows : resetWindows,
                );
                if (previousRect) damage.push(previousRect);
                this.windows.delete(id);
                if (
                    existing?.viewable && !existing.inputOnly &&
                    !existing.desktopLayer && (!existing.shellLayer || (existing.id === RICEWM_MENU_ID && this.nativeCanvasWindow.snapshot?.shell?.shellStyle === "Embedded")) &&
                    existing.parent !== existing.id &&
                    this.nativeCanvasWindow.snapshot?.shell?.motionEnabled === true &&
                    (this.nativeCanvasWindow.snapshot?.shell?.motion?.window === "Foldout" ||
                        (id === RICEWM_MENU_ID && this.nativeCanvasWindow.snapshot?.shell?.shellStyle === "Embedded"))
                ) {
                    this.closingSceneWindows.set(id, existing);
                    this.windowTransitions.set(id, {
                        startedAt: this.now(),
                        direction: "close",
                    });
                } else {
                    this.windowTransitions.delete(id);
                }
                resetWindows?.delete(id);
            } else if (opcode === RICE_SCENE_DAMAGE_WINDOW) {
                damagedSurfaces.push({
                    id: view.getUint32(offset + 4, true),
                    x: view.getInt32(offset + 12, true),
                    y: view.getInt32(offset + 16, true),
                    width: view.getUint16(offset + 20, true),
                    height: view.getUint16(offset + 22, true),
                });
            } else if (opcode !== RICE_SCENE_DAMAGE_WINDOW) {
                throw new RangeError(
                    `unknown RiceWM scene opcode ${opcode}`,
                );
            }
        }
        if (resetWindows) {
            for (const [id, previous] of resetWindows) {
                if (this.windows.has(id)) continue;
                const previousRect = this._sceneWindowRect(previous, resetWindows);
                if (previousRect) damage.push(previousRect);
                if (
                    previous.viewable && !previous.inputOnly &&
                    !previous.desktopLayer && (!previous.shellLayer || (previous.id === RICEWM_MENU_ID && this.nativeCanvasWindow.snapshot?.shell?.shellStyle === "Embedded")) &&
                    previous.parent !== previous.id &&
                    this.nativeCanvasWindow.snapshot?.shell?.motionEnabled === true &&
                    (this.nativeCanvasWindow.snapshot?.shell?.motion?.window === "Foldout" ||
                        (id === RICEWM_MENU_ID && this.nativeCanvasWindow.snapshot?.shell?.shellStyle === "Embedded"))
                ) {
                    this.closingSceneWindows.set(id, previous);
                    this.windowTransitions.set(id, {
                        startedAt: this.now(),
                        direction: "close",
                    });
                } else {
                    this.windowTransitions.delete(id);
                }
            }
        }
        for (const id of changedWindows) {
            const nextRect = this._sceneWindowRect(this.windows.get(id), this.windows);
            if (nextRect) damage.push(nextRect);
        }
        for (const item of damagedSurfaces) {
            changedWindows.add(item.id);
            const window = this.windows.get(item.id);
            const rect = this._sceneWindowRect(window, this.windows, item);
            if (rect) damage.push(rect);
        }
        for (const upload of uploads) {
            this._upload(upload);
        }
        if (Array.isArray(shellTexts)) {
            const signature = JSON.stringify(shellTexts);
            if (signature !== this.shellTextsSignature) {
                const previousTexts=this.shellTexts;
                const affected = new Set([
                    ...this.shellTexts.keys(),
                    ...shellTexts.map(overlay => overlay.id),
                ]);
                this.shellTexts = new Map();
                for (const overlay of shellTexts) {
                    const overlays = this.shellTexts.get(overlay.id) || [];
                    overlays.push(overlay);
                    this.shellTexts.set(overlay.id, overlays);
                }
                this.shellTextsSignature = signature;
                for (const id of affected) {
                    if(JSON.stringify(previousTexts.get(id))!==JSON.stringify(this.shellTexts.get(id)))changedWindows.add(id);
                    const rect = this._sceneWindowRect(this.windows.get(id), this.windows);
                    if (rect) damage.push(rect);
                }
            }
        }
        for(const id of changedWindows){const window=this.windows.get(id);if(window)window.spatialRevision=(window.spatialRevision||0)+1;}
        if (damage.length || uploads.length) {
            this.staticBackdropRevision = (this.staticBackdropRevision || 0) + 1;
            this.sceneBackdropRevision = (this.sceneBackdropRevision || 0) + 1;
        }
        this._markNativeDamage(damage);
        if (this.overviewActive && this._damaged) this.overviewItems = null;
        this.schedule();
    }

    setApplicationIcons(catalog) {
        if (!catalog || typeof catalog !== "object") return;
        // Every retained scene carries the current icon catalog so a newly
        // attached renderer is self-contained. Pointer motion can therefore
        // deliver the same catalog many times per second. Rebuilding the
        // maps and marking the whole canvas damaged for identical metadata
        // turns cursor-only scenes into visible full-desktop redraws.
        const signature = riceLocale() + JSON.stringify(catalog);
        if (signature === this.applicationIconsSignature) return;
        this.applicationIconsSignature = signature;
        this.nativeCanvasWindow.iconRenderer.setApplicationSelectorAliases(catalog.menu||[]);
        this.menuApplicationIds=new Map((catalog.menu||[]).filter(entry=>entry.id||entry.applicationId).flatMap(entry=>[[String(entry.label||''),entry.id||entry.applicationId],...(entry.labelMessage?[[t(entry.labelMessage),entry.id||entry.applicationId]]:[])]));
        this.menuApplicationIcons = new Map(
            (Array.isArray(catalog.menu) ? catalog.menu : []).flatMap(entry => {
                const label = String(entry?.label || "");
                const icon = riceIconName(entry?.iconName);
                return label && icon ? [[label, icon], ...(entry.labelMessage ? [[t(entry.labelMessage), icon]] : [])] : [];
            }),
        );
        this.menuColorArtworkLabels = new Set(
            (Array.isArray(catalog.menu) ? catalog.menu : []).flatMap(entry =>
                entry?.colorArtwork === true
                    ? [String(entry.label || ""), ...(entry.labelMessage ? [t(entry.labelMessage)] : [])]
                    : []),
        );
        this.menuSourceIcons = new Map(
            (Array.isArray(catalog.menu) ? catalog.menu : []).flatMap(entry => {
                const label = String(entry?.label || "");
                const icon = riceIconName(entry?.sourceIconName);
                return label && icon ? [[label, icon], ...(entry.labelMessage ? [[t(entry.labelMessage), icon]] : [])] : [];
            }),
        );
        this.menuProvisionableApplications = new Set(
            (Array.isArray(catalog.menu) ? catalog.menu : []).flatMap(entry =>
                entry?.provisionable === true && entry?.label
                    ? [String(entry.label)]
                    : []
            ),
        );
        this.menuBuiltinIcons = new Map(
            (Array.isArray(catalog.builtins) ? catalog.builtins : []).flatMap(entry => {
                const label = String(entry?.label || "");
                const icon = riceIconName(entry?.iconName);
                return label && icon ? [[label, icon]] : [];
            }),
        );
        this.applicationCatalog = (Array.isArray(catalog.menu) ? catalog.menu : [])
            .map(entry => ({
                kind: "application",
                id: Number(entry?.action) >>> 0,
                action: Number(entry?.action) >>> 0,
                title: entry?.labelMessage ? t(entry.labelMessage) : String(entry?.label || ""),
                subtitle: t("Application"),
                iconName: riceIconName(entry?.iconName),
            }))
            .filter(entry => entry.id && entry.title);
        this.quickApplicationIcons = (Array.isArray(catalog.quick) ? catalog.quick : [])
            .flatMap(entry => {
                const index = Number(entry?.index);
                const iconName = riceIconName(entry?.iconName);
                return Number.isInteger(index) && index >= 0 && index < 8 && iconName
                    ? [{ index, iconName, applicationId:String(entry.applicationId||'') }]
                    : [];
            });
        this.nativeCanvasWindow.setDesktopApplicationIcons(catalog.desktop);
        this.nativeCanvasWindow.setFileAssociationIcons(catalog.associations);
        this.damaged = true;
        this.schedule();
    }

    setApplicationIconAssets(assets) {
        this.nativeCanvasWindow.setApplicationIconAssets(assets);
        this.damaged = true;
        this.schedule();
    }

    prepareNativeLocale(snapshot) {
        return this.nativeCanvasWindow.prepareLocale(snapshot);
    }

    applyNative(snapshot) {
        this.nativeCanvasWindow.apply(snapshot);
        // Native snapshots own the current dock layout. Do not paint standard
        // taskbar overlays above a compact dock while a preference message is
        // still in flight (or when this compositor is hosted directly).
        if (snapshot?.shell?.dockStyle) this.dockStyle = snapshot.shell.dockStyle;
        this._reconcileNativeMedia(snapshot);
        if (this.overviewActive) this.overviewItems = null;
    }

    animateLaunchAt(x, y, source = "menu") {
        const shell = this.nativeCanvasWindow.snapshot?.shell;
        if (shell?.motionEnabled !== true) return false;
        if (source === "desktop") {
            // The page reports every primary press as a potential desktop
            // launch before the display worker resolves its target. Suppress
            // native desktop feedback when a retained application frame owns
            // the point; otherwise a click inside X11/USER32 content can
            // animate the desktop shortcut geometrically underneath it.
            const pointX = Number(x);
            const pointY = Number(y);
            const root = [...this.windows.values()].find(window =>
                window.parent === window.id);
            if (root && [...this.windows.values()].some(window => {
                if (
                    !window.viewable || window.inputOnly ||
                    window.desktopLayer || window.shellLayer ||
                    window.parent !== root.id
                ) return false;
                const border = Number(window.borderWidth) || 0;
                return pointX >= window.x - border &&
                    pointY >= window.y - border &&
                    pointX < window.x + window.width + border &&
                    pointY < window.y + window.height + border;
            })) return false;
            return this.nativeCanvasWindow.animateDesktopLaunchAt(x, y);
        }
        return false;
    }

    finishDesktopLaunchAnimation(key) {
        return this.nativeCanvasWindow.finishDesktopLaunchAnimation(key);
    }

    pointerHoverAt(x, y) {
        const next = Number.isFinite(x) && Number.isFinite(y)
            ? { x: Number(x), y: Number(y) }
            : null;
        this.pointerPosition = next;
        this.customEffects.setInputs({pointer:next?[next.x/Math.max(1,this.canvas.width),next.y/Math.max(1,this.canvas.height)]:[0,0]});
        this.nativeCanvasWindow.pointerHoverAt(next?.x, next?.y);
        if (this.dockStyle === "Compact dock" && this.windows.get(RICEWM_MENU_ID)?.viewable && (this.windows.get(RICEWM_MENU_ID).backgroundPixel >>> 31)) {this.damaged=true;this.schedule();}
    }

    nativeTerminalBackdrop(app, session) {
        return this.nativeCanvasWindow.terminalBackdrop(app, session);
    }

    get shellOverlayActive() {
        return this.overviewActive || this.calendarActive ||
            this.windows.get(RICEWM_MENU_ID)?.viewable === true ||
            (this.nativeCanvasWindow.snapshot?.windows || []).some(window =>
                window?.role === "desktop" && window.layerVisible !== false &&
                (window.popup || window.dialog)
            );
    }

    interactiveShellAt(x, y) {
        const pointX = Number(x);
        const pointY = Number(y);
        if (!Number.isFinite(pointX) || !Number.isFinite(pointY)) return false;
        return [
            RICEWM_GNOME_PANEL_ID,
            RICEWM_TASKBAR_ID,
            RICEWM_MENU_ID,
            RICEWM_NOTIFICATION_ID,
        ]
            .some(id => {
                const window = this.windows.get(id);
                return Boolean(window?.viewable) &&
                    pointX >= window.x && pointY >= window.y &&
                    pointX < window.x + window.width &&
                    pointY < window.y + window.height;
            });
    }

    managedFrameCursorAt(x, y) {
        const pointX = Number(x);
        const pointY = Number(y);
        if (!Number.isFinite(pointX) || !Number.isFinite(pointY)) return null;
        const root = [...this.windows.values()].find(window =>
            window.parent === window.id
        );
        if (!root) return null;
        const frame = [...this.windows.values()]
            .filter(window =>
                window.id >= RICEWM_FIRST_MANAGED_FRAME_ID &&
                window.id < RICEWM_MANAGED_FRAME_ID_LIMIT &&
                window.parent === root.id &&
                window.viewable && !window.inputOnly &&
                window.resizableWindow && !window.maximizedWindow &&
                pointX >= window.x && pointY >= window.y &&
                pointX < window.x + window.width &&
                pointY < window.y + window.height
            )
            .sort((left, right) =>
                left.stackIndex - right.stackIndex || left.id - right.id
            )
            .at(-1);
        if (!frame) return null;
        const localX = pointX - frame.x;
        const localY = pointY - frame.y;
        // Match the compositor's invisible resize target. The visible frame
        // remains one pixel; a borderless native host still gets a usable
        // directional cursor several pixels inside its client surface.
        const edge = 7;
        const left = localX < edge;
        const right = localX >= frame.width - edge;
        const top = localY < edge;
        const bottom = localY >= frame.height - edge;
        if ((left && top) || (right && bottom)) return "nwse-resize";
        if ((right && top) || (left && bottom)) return "nesw-resize";
        if (left || right) return "ew-resize";
        if (top || bottom) return "ns-resize";
        return null;
    }

    applyNativeMediaFrame(frame) {
        const width = Number(frame?.width);
        const height = Number(frame?.height);
        const pixels = frame?.pixels instanceof Uint8Array
            ? frame.pixels
            : new Uint8Array(frame?.pixels || 0);
        if (
            !frame?.session || !Number.isInteger(width) || width < 1 || width > 1280 ||
            !Number.isInteger(height) || height < 1 || height > 720 ||
            pixels.byteLength !== width * height * 4
        ) throw new RangeError("invalid Rice Player software frame");
        const key = String(frame.session);
        if (
            this.nativeMediaRetiredRequests.has(key) ||
            this.nativeMediaLiveRequests.has(key) &&
                !this.nativeMediaHotRequests.has(key)
        ) return false;
        const surface = this.canvasFactory(width, height);
        surface.width = width;
        surface.height = height;
        const context = surface.getContext("2d");
        const image = context.createImageData(width, height);
        image.data.set(pixels);
        context.putImageData(image, 0, 0);
        if(frame.role==='wallpaper')this.nativeMediaWallpaperRequests.add(key);
        this._retainNativeMediaSurface(key, surface, width * height * 4);
        this.damaged = true;
        this.schedule();
        return true;
    }

    applyNativeMediaSurface(frame) {
        const width = Number(frame?.width);
        const height = Number(frame?.height);
        const surface = frame?.surface;
        const surfaceWidth = surface?.displayWidth ?? surface?.videoWidth ??
            surface?.naturalWidth ?? surface?.width;
        const surfaceHeight = surface?.displayHeight ?? surface?.videoHeight ??
            surface?.naturalHeight ?? surface?.height;
        if (
            !frame?.request || !surface ||
            !Number.isInteger(width) || width < 1 || width > 8192 ||
            !Number.isInteger(height) || height < 1 || height > 8192 ||
            width * height > 37_748_736 ||
            surfaceWidth !== width || surfaceHeight !== height
        ) throw new RangeError("invalid native preview surface");
        const key = String(frame.request);
        if (
            this.nativeMediaRetiredRequests.has(key) ||
            this.nativeMediaLiveRequests.has(key) &&
                !this.nativeMediaHotRequests.has(key)
        ) {
            surface.close?.();
            return false;
        }
        if(frame.role==='wallpaper')this.nativeMediaWallpaperRequests.add(key);
        this._retainNativeMediaSurface(key, surface, width * height * 4);
        this.nativeMediaRefreshQueue.delete(key);
        this.damaged = true;
        this.schedule();
        return true;
    }

    _retainNativeMediaSurface(key, surface, bytes) {
        this.staticBackdropRevision = (this.staticBackdropRevision || 0) + 1;
        this.sceneBackdropRevision = (this.sceneBackdropRevision || 0) + 1;
        const previous = this.nativeMediaSurfaces.get(key);
        const previousBytes = this.nativeMediaSurfaceBytes.get(key) || 0;
        if (previous !== surface) previous?.close?.();
        // Setting an existing key intentionally preserves insertion order:
        // reads and replacement frames never promote an entry in this FIFO.
        this.nativeMediaSurfaces.set(key, surface);
        this.nativeMediaSurfaceBytes.set(key, bytes);
        this.nativeMediaBytes = Math.max(
            0,
            this.nativeMediaBytes - previousBytes + bytes,
        );
        while (
            this.nativeMediaBytes > RICE_NATIVE_MEDIA_CACHE_MAX_BYTES ||
            this.nativeMediaSurfaces.size > MAX_NATIVE_MEDIA_SURFACES
        ) {
            // Preserve the active panorama before evicting ordinary previews.
            const keys=[...this.nativeMediaSurfaces.keys()];
            const oldest=keys.find(candidate=>!this.nativeMediaWallpaperRequests.has(candidate)) ?? keys[0];
            if (oldest === undefined) break;
            this._evictNativeMediaSurface(oldest);
        }
        // A successful refresh must allow recovery from a later eviction in
        // the same scene. Failed refreshes remain bounded by their generation.
        if (this.nativeMediaSurfaces.has(key)) this.nativeMediaRefreshAttempts.delete(key);
    }

    nativeMediaCacheUsage() {
        return Object.freeze({
            bytes: this.nativeMediaBytes,
            entries: this.nativeMediaSurfaces.size,
            maximumBytes: RICE_NATIVE_MEDIA_CACHE_MAX_BYTES,
        });
    }

    applyNativeMediaState(event) {
        const key = String(event?.session || "");
        if (!key || key === "0") {
            throw new RangeError("invalid Rice Player media state session");
        }
        if (event.state === "seeking") {
            this.nativeMediaStates.set(key, {
                state: "seeking",
                bytesParsed: Math.max(0, Number(event.bytesParsed) || 0),
                sourceBytes: Math.max(0, Number(event.sourceBytes) || 0),
                targetMillis: Math.max(0, Number(event.targetMillis) || 0),
            });
            this._scheduleMediaBlink();
        } else {
            this.nativeMediaStates.delete(key);
            if (this.nativeMediaStates.size === 0) {
                this.nativeMediaBlinkVisible = true;
                if (this.nativeMediaBlinkTimer !== null) {
                    this.clearTimer(this.nativeMediaBlinkTimer);
                    this.nativeMediaBlinkTimer = null;
                }
            }
        }
        this.damaged = true;
        this.schedule();
    }

    get nativeWindow() {
        return this.nativeCanvasWindow.snapshot;
    }

    get overviewActive() {
        return this.overviewTarget > 0 || this.overviewProgress > 0;
    }

    setShellPreferences({ workspaceNames, launcherAppearance, dockStyle } = {}) {
        if (dockStyle) this.dockStyle = dockStyle;
        if (Array.isArray(workspaceNames) && workspaceNames.length === 4) {
            this.workspaceNames = workspaceNames.map((name, index) =>
                String(name || index + 1).trim().slice(0, 16)
            );
        }
        if (["Icon only", "Icon and text", "Text only"].includes(launcherAppearance)) {
            this.launcherAppearance = launcherAppearance;
        }
        this.damaged = true;
        this.schedule();
    }

    setGnomeShellState(state = null) {
        this.gnomeShellState = state && typeof state === "object" ? state : null;
        const overview = this.gnomeShellState?.overview;
        const styleClasses = Array.isArray(this.gnomeShellState?.styleClasses)
            ? this.gnomeShellState.styleClasses : [];
        this.overviewSearchVisible = overview?.searchVisible !== false &&
            !styleClasses.includes("just-perfection-api-no-search");
        this.overviewDashVisible = overview?.dashVisible !== false;
        this.overviewWorkspacesVisible =
            !styleClasses.includes("just-perfection-api-no-workspace");
        const scale = Number(overview?.workspaceScale);
        this.overviewWorkspaceScale = Number.isFinite(scale) && scale > 0
            ? Math.max(0.05, Math.min(0.35, scale))
            : 0.1;
        this.workspaceTransitionStyle =
            this.gnomeShellState?.surfaces?.desktopCube?.enabled === true
                ? "cube" : "slide";
        this.overviewItems = null;
        this.damaged = true;
        this.schedule();
    }

    setGnomeEffects(payload = null) {
        this.gnomeEffectRenderer.update(payload || {});
        this.damaged = true;
        this.schedule();
    }

    setGnomeEffectAcceleration(enabled) {
        this.gnomeEffectRenderer.setEnabled(Boolean(enabled));
        this.damaged = true;
        this.schedule();
    }

    setGnomeEffectAccelerationMode(mode) {
        this.gnomeEffectRenderer.setMode(mode);
        this.damaged = true;
        this.schedule();
    }

    setVisualEffectAccelerationMode(mode) {
        this.customEffects.setMode(mode);
        this.gnomeEffectRenderer.setMode(mode);
        this.shellFrostedIceMaterial.setAccelerationMode(mode);
        this.nativeCanvasWindow.setVisualEffectAccelerationMode(mode);
        this.damaged = true;
        this.schedule();
    }

    shellTargetAt(x, y) {
        const panel = this.windows.get(RICEWM_GNOME_PANEL_ID);
        if (panel?.viewable && y >= panel.y && y < panel.y + panel.height) {
            const mail = this._mailIndicatorRect(panel);
            if (x >= mail.x && x < mail.x + mail.width) return "mail";
            const clockLeft = panel.x + Math.max(0, (panel.width - 84) / 2);
            if (x >= clockLeft && x < clockLeft + 84) return "clock";
        }
        const taskbar = this.windows.get(RICEWM_TASKBAR_ID);
        if (this.dockStyle === "Compact dock") return null;
        if (!taskbar?.viewable || y < taskbar.y || y >= taskbar.y + taskbar.height) {
            return null;
        }
        if (
            this.overviewDashVisible &&
            this.nativeCanvasWindow.snapshot?.shell?.launcherVisible !== false &&
            this.launcherAppearance !== "Text only"
        ) {
            const redmond98 =
                this.nativeCanvasWindow.snapshot?.chrome?.redmond98 === true;
            const vertical = taskbar.height > taskbar.width;
            const size = redmond98 ? 16 : 22;
            const logo = {
                x: taskbar.x + (vertical
                    ? Math.max(0, Math.floor((taskbar.width - size) / 2))
                    : 6),
                y: taskbar.y + (redmond98 ? 8 : 5),
                width: size,
                height: size,
            };
            if (
                x >= logo.x && x < logo.x + logo.width &&
                y >= logo.y && y < logo.y + logo.height
            ) return "logo";
        }
        let cursor = taskbar.x + taskbar.width - 7;
        const widths = item => item?.type === "clock" ? 70
            : item?.type === "workspaces"
                ? this.workspaceNames.reduce((total, name) =>
                    total + Math.max(28, Math.min(100, [...name].length * 7 + 16)), 0)
                : item?.type === "performance" ? 66 : 25;
        for (let index = this.trayItems.length - 1; index >= 0; index -= 1) {
            const item = this.trayItems[index];
            const width = widths(item);
            cursor -= width;
            if (x >= cursor && x < cursor + width) return item?.type || null;
            cursor -= 4;
        }
        return null;
    }

    beginWorkspaceTransition(direction) {
        const normalizedDirection = Number(direction) < 0 ? -1 : 1;
        if (this.workspaceTransition) return true;
        if (
            this.nativeCanvasWindow.snapshot?.shell?.motionEnabled === false ||
            this.canvas.width < 1 || this.canvas.height < 1
        ) {
            this.workspaceTransition = null;
            return false;
        }
        // Capture the fully presented old workspace before the Worker swaps
        // its retained scene. The new scene can then arrive normally while
        // both complete desktops remain available to the compositor.
        this.flush();
        const source = this.doubleBuffer && this.compositionSurface
            ? this.compositionSurface
            : this.canvas;
        const from = this._workspaceTransitionSnapshot(source);
        if (!from) return false;
        this.workspaceTransition = {
            direction: normalizedDirection,
            from,
            to: null,
            startedAt: null,
            style: this.workspaceTransitionStyle,
        };
        return true;
    }

    beginWorkspaceSwipe(bounds,to,direction,initial=null) {
        if(this.nativeCanvasWindow.snapshot?.shell?.motionEnabled===false||!to||![bounds?.x,bounds?.y,bounds?.width,bounds?.height].every(Number.isFinite)||bounds.width<=0||bounds.height<=0||bounds.x<0||bounds.y<0||bounds.x+bounds.width>this.canvas.width||bounds.y+bounds.height>this.canvas.height)return false;
        this.flush();
        const source=this.presentationSource||this.compositionSurface||this.canvas;
        const from=this.canvasFactory(bounds.width,bounds.height);
        from.width=bounds.width;from.height=bounds.height;
        const context=from.getContext('2d',{alpha:false});if(!context)return false;
        if(initial)context.drawImage(initial,0,0,bounds.width,bounds.height,0,0,bounds.width,bounds.height);
        else context.drawImage(source,bounds.x,bounds.y,bounds.width,bounds.height,0,0,bounds.width,bounds.height);
        this.workspaceTransition={interactive:true,bounds:{...bounds},from,to,direction:direction<0?-1:1,progress:0,goal:null};
        return true;
    }
    updateWorkspaceSwipe(progress) {
        const t=this.workspaceTransition;if(!t?.interactive||t.goal!==null||!Number.isFinite(progress))return false;
        t.progress=Math.max(0,Math.min(1,progress));this.damaged=true;this.schedule();return true;
    }
    finishWorkspaceSwipe(commit) {
        const t=this.workspaceTransition;if(!t?.interactive)return false;
        t.goal=commit?1:0;t.fromProgress=t.progress;t.startedAt=this.now();
        this.damaged=true;this.schedule();return true;
    }
    _presentWorkspaceSwipe(context,target,t) {
        if(t.goal!==null) {
            const duration=Math.max(1,220*Math.abs(t.goal-t.fromProgress));
            const amount=Math.min(1,Math.max(0,this.now()-t.startedAt)/duration);
            t.progress=t.fromProgress+(t.goal-t.fromProgress)*(1-(1-amount)**3);
            if(amount===1){this.workspaceTransition=null;return false;}
        }
        const b=t.bounds,oldY=-t.direction*Math.round(t.progress*b.height),nextY=oldY+t.direction*b.height;
        context.save();context.setTransform(1,0,0,1,0,0);
        // Neighboring monitors keep their live scene during this output's swipe.
        if(target!==this.canvas)context.drawImage(target,0,0);
        context.beginPath();context.rect(b.x,b.y,b.width,b.height);context.clip();
        context.fillStyle='rgb(16,21,28)';context.fillRect(b.x,b.y,b.width,b.height);
        context.drawImage(t.from,b.x,b.y+oldY);context.drawImage(t.to,b.x,b.y+nextY);context.restore();return true;
    }
    _workspaceTransitionSnapshot(source) {
        const width = Number(this.canvas.width) || 0;
        const height = Number(this.canvas.height) || 0;
        if (width < 1 || height < 1 || !source) return null;
        const surface = this.canvasFactory(width, height);
        surface.width = width;
        surface.height = height;
        const context = surface.getContext?.("2d", { alpha: true });
        if (!context) return null;
        const root = [...this.windows.values()].find(window =>
            window.parent === window.id
        );
        // Native terminal portals intentionally punch transparent holes in
        // the live canvas. Give transition snapshots an opaque desktop plane
        // so those holes can never expose a black wrapper while sliding.
        context.fillStyle = pixelColor(root?.backgroundPixel || 0x10151c);
        context.fillRect(0, 0, width, height);
        if (root?.surface) context.drawImage(root.surface, 0, 0);
        context.drawImage(source, 0, 0);
        return surface;
    }

    _presentWorkspaceTransition(context, target) {
        const transition = this.workspaceTransition;
        if (!transition) return false;
        if(transition.interactive)return this._presentWorkspaceSwipe(context,target,transition);
        if (!transition.to) {
            transition.to = this._workspaceTransitionSnapshot(target);
            transition.startedAt = this.now();
        }
        if (!transition.to) {
            this.workspaceTransition = null;
            return false;
        }
        const elapsed = Math.max(0, this.now() - transition.startedAt);
        const duration = transition.style === "cube"
            ? Math.max(420, this.workspaceTransitionDuration)
            : this.workspaceTransitionDuration;
        const linear = Math.min(1, elapsed / duration);
        if (linear >= 1) {
            this.workspaceTransition = null;
            return false;
        }
        const progress = linear < 0.5
            ? 4 * linear * linear * linear
            : 1 - Math.pow(-2 * linear + 2, 3) / 2;
        const width = this.canvas.width;
        const height = this.canvas.height;
        if (transition.style === "cube") {
            this._presentWorkspaceCube(
                context,
                transition.from,
                transition.to,
                transition.direction,
                progress,
                width,
                height,
            );
            return true;
        }
        const oldY = transition.direction > 0
            ? -Math.round(progress * height)
            : Math.round(progress * height);
        const newY = transition.direction > 0
            ? oldY + height
            : oldY - height;
        context.save();
        context.setTransform(1, 0, 0, 1, 0, 0);
        context.globalCompositeOperation = "copy";
        context.fillStyle = "rgb(16, 21, 28)";
        context.fillRect(0, 0, width, height);
        context.globalCompositeOperation = "source-over";
        // The positions share one exact edge, including after rounding, so
        // there is never a border or unpainted seam between workspaces.
        context.drawImage(transition.from, 0, oldY);
        context.drawImage(transition.to, 0, newY);
        context.restore();
        return true;
    }

    _presentWorkspaceCube(context, from, to, direction, progress, width, height) {
        const outgoing = Math.cos(progress * Math.PI / 2);
        const incoming = Math.sin(progress * Math.PI / 2);
        const total = Math.max(0.0001, outgoing + incoming);
        const oldWidth = Math.max(0, Math.round(width * outgoing / total));
        const newWidth = Math.max(0, width - oldWidth);
        const edge = direction > 0 ? oldWidth : newWidth;
        const slices = Math.max(24, Math.min(160, Math.ceil(width / 8)));
        const drawFace = (source, left, faceWidth, sharedOnRight, darkness) => {
            if (faceWidth <= 0) return;
            for (let slice = 0; slice < slices; slice += 1) {
                const sourceX = Math.floor(slice * width / slices);
                const sourceRight = Math.ceil((slice + 1) * width / slices);
                const ratio = (slice + 0.5) / slices;
                const near = sharedOnRight ? ratio : 1 - ratio;
                const depthScale = 0.72 + 0.28 * near;
                const targetX = left + Math.floor(slice * faceWidth / slices);
                const targetRight = left + Math.ceil((slice + 1) * faceWidth / slices);
                const targetHeight = Math.max(1, Math.round(height * depthScale));
                const targetY = Math.round((height - targetHeight) / 2);
                context.drawImage(
                    source,
                    sourceX,
                    0,
                    Math.max(1, sourceRight - sourceX),
                    height,
                    targetX,
                    targetY,
                    Math.max(1, targetRight - targetX),
                    targetHeight,
                );
            }
            context.fillStyle = `rgba(0, 0, 0, ${Math.max(0, Math.min(0.42, darkness))})`;
            context.fillRect(left, 0, faceWidth, height);
        };
        context.save();
        context.setTransform(1, 0, 0, 1, 0, 0);
        context.globalCompositeOperation = "copy";
        context.fillStyle = "rgb(8, 11, 16)";
        context.fillRect(0, 0, width, height);
        context.globalCompositeOperation = "source-over";
        if (direction > 0) {
            drawFace(from, 0, oldWidth, true, progress * 0.32);
            drawFace(to, edge, newWidth, false, (1 - progress) * 0.32);
        } else {
            drawFace(to, 0, newWidth, true, (1 - progress) * 0.32);
            drawFace(from, edge, oldWidth, false, progress * 0.32);
        }
        context.fillStyle = "rgba(255, 255, 255, 0.16)";
        context.fillRect(Math.max(0, edge - 1), 0, 2, height);
        context.restore();
    }

    setMailModel(model) {
        const unread = Number(model?.unread);
        this.mailModel = Object.freeze({
            configured: model?.configured === true,
            unread: Number.isSafeInteger(unread) && unread > 0
                ? Math.min(unread, 999)
                : 0,
        });
        this.damaged = true;
        this.schedule();
    }

    setSystemControlsLayout({
        enabled = false,
        placement = "top",
        batteryPercentage = null,
        ethernet = false,
        wifi = false,
        bluetooth = false,
        audio = false,
        muted = false,
    } = {}) {
        this.systemControlsLayout = Object.freeze({
            enabled: Boolean(enabled),
            placement: placement === "taskbar" ? "taskbar" : "top",
            batteryPercentage: batteryPercentage !== null && batteryPercentage !== undefined &&
                Number.isInteger(Number(batteryPercentage)) &&
                Number(batteryPercentage) >= 0 && Number(batteryPercentage) <= 100
                ? Number(batteryPercentage) : null,
            ethernet: Boolean(ethernet),
            wifi: Boolean(wifi),
            bluetooth: Boolean(bluetooth),
            audio: Boolean(audio),
            muted: Boolean(muted),
        });
        this.damaged = true;
        this.schedule();
    }

    toggleCalendar(force = null, anchor = null) {
        this.calendarActive = force === null ? !this.calendarActive : Boolean(force);
        if (this.calendarActive) {
            this.closeOverview();
            this.calendarMonth = new Date();
            this.calendarMonth.setDate(1);
            this.calendarSelectedDate = new Date();
            const panel = this.windows.get(RICEWM_GNOME_PANEL_ID);
            this.calendarFromTopBar = Boolean(
                panel?.viewable && anchor &&
                anchor.y >= panel.y && anchor.y < panel.y + panel.height,
            );
        }
        this.calendarLayout = null;
        this.damaged = true;
        this.schedule();
        return this.calendarActive;
    }

    setNotificationHistory(entries) {
        const previousNewestFirst = this.notificationHistory.slice().reverse();
        const previousTopId = previousNewestFirst[this.notificationHistoryScroll]?.id || null;
        this.notificationHistory = (Array.isArray(entries) ? entries : [])
            .slice(-50)
            .map(entry => Object.freeze({
                id: String(entry?.id || ""),
                generation: entry?.generation,
                kind: String(entry?.kind || "info").slice(0, 24),
                title: String(entry?.title || "Notification").slice(0, 160),
                message: String(entry?.message || "").slice(0, 512),
                timestamp: Number(entry?.timestamp) || 0,
                actionLabel: String(entry?.actionLabel || '').slice(0,80),
                actions: (entry?.actions || []).slice(0,3).map(({id,label})=>({id,label:String(label).slice(0,80)})),
                replyAllowed: entry?.chat===true && entry?.replyAllowed===true,
            }));
        const newestFirst = this.notificationHistory.slice().reverse();
        this.notificationHistoryScroll = this.notificationHistoryScroll > 0 && previousTopId
            ? Math.max(0, newestFirst.findIndex(entry => entry.id === previousTopId))
            : 0;
        if (!newestFirst.some(entry => entry.id === this.notificationExpandedId)) {
            this.notificationExpandedId = null;
        }
        this.damaged = true;
        this.schedule();
    }

    _notificationAction(action, id = "", actionId) {
        const generation=this.notificationHistory.find(entry=>entry.id===id)?.generation;
        // The worker publishes acknowledged removals. Optimistically erasing
        // a history card would hide a draft when retirement is refused.
        this.onNotificationAction?.({ action, id, ...(generation?{generation}:{}), ...(actionId===undefined?{}:{actionId}) });
    }

    setCalendarModel(model) {
        const events = (Array.isArray(model?.events) ? model.events : [])
            .slice(0, 2_000)
            .map(event => Object.freeze({
                id: String(event?.id || "").slice(0, 96),
                title: String(event?.title || "Untitled").slice(0, 160),
                startDate: String(event?.startDate || ""),
                startTime: String(event?.startTime || ""),
                endDate: String(event?.endDate || ""),
                endTime: String(event?.endTime || ""),
                source: String(event?.source || "").slice(0, 128),
            }));
        this.calendarModel = Object.freeze({
            interface: String(model?.interface || ""),
            defaultSource: String(model?.defaultSource || ""),
            events,
        });
        this.damaged = true;
        this.schedule();
    }

    calendarPointer(x, y, button = 1) {
        if (!this.calendarActive || !this.calendarLayout) return false;
        const { panel, previous, next, notifications, notificationPane,
            visibleNotifications, notificationCards, notificationDismissRects, notificationActionRects,
            notificationClear, notificationScrollbar, calendarEventRects,
            dateCells, openDay } =
            this.calendarLayout;
        const inside = rect => Boolean(rect) && x >= rect.x && y >= rect.y &&
            x < rect.x + rect.width && y < rect.y + rect.height;
        if ((button === 4 || button === 5) && inside(notificationPane || notifications)) {
            const maximum = Math.max(
                0,
                this.notificationHistory.length - visibleNotifications,
            );
            this.notificationHistoryScroll = Math.max(0, Math.min(
                maximum,
                this.notificationHistoryScroll + (button === 5 ? 1 : -1),
            ));
        } else if (button !== 1) {
            return true;
        } else
        if (notificationClear && inside(notificationClear)) {
            this._notificationAction("clear");
        } else if (notificationActionRects?.some(entry => {
            if (!inside(entry.rect) || !inside(notifications)) return false;
            this._notificationAction('invoke',entry.id,entry.actionId);
            return true;
        })) {
            // Button clicks retain the card until the worker acknowledges the action.
        } else if (notificationDismissRects?.some(entry => {
            if (!inside(entry.rect)) return false;
            this._notificationAction("dismiss", entry.id);
            return true;
        })) {
            // The matched notification was dismissed above.
        } else if (notificationScrollbar && inside(notificationScrollbar.track)) {
            const maximum = Math.max(
                0,
                this.notificationHistory.length - visibleNotifications,
            );
            const position = Math.max(0, Math.min(
                1,
                (y - notificationScrollbar.track.y) /
                    Math.max(1, notificationScrollbar.track.height),
            ));
            this.notificationHistoryScroll = Math.round(maximum * position);
        } else if (notificationCards?.some(entry => {
            if (!inside(entry.rect)) return false;
            if(this.notificationHistory.find(item=>item.id===entry.id)?.replyAllowed) {
                this.calendarActive=false;
                this._notificationAction('show-reply',entry.id);
                return true;
            }
            this.notificationExpandedId = this.notificationExpandedId === entry.id
                ? null
                : entry.id;
            return true;
        })) {
            // Notification cards expand in place like GNOME message banners.
        } else if (calendarEventRects?.some(entry => {
            if (!inside(entry.rect)) return false;
            this.calendarActive = false;
            return true;
        })) {
            const event = calendarEventRects.find(entry => inside(entry.rect));
            return { action: "open-event", id: event.id, date: event.date };
        } else if (inside(previous)) this.calendarMonth.setMonth(this.calendarMonth.getMonth() - 1);
        else if (inside(next)) this.calendarMonth.setMonth(this.calendarMonth.getMonth() + 1);
        else if (inside(openDay)) {
            this.calendarActive = false;
            return { action: "open-day", date: this._calendarDateKey(this.calendarSelectedDate) };
        } else if (dateCells.some(entry => {
            if (!inside(entry.rect)) return false;
            this.calendarSelectedDate = new Date(entry.date);
            return true;
        })) {
            // The selected day's agenda is rendered below the month grid.
        }
        else if (!inside(panel)) this.calendarActive = false;
        this.damaged = true;
        this.schedule();
        return { action: "handled" };
    }

    _calendarDateKey(date) {
        return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-` +
            String(date.getDate()).padStart(2, "0");
    }

    _calendarEventOccursOn(event, dateKey) {
        return event.startDate <= dateKey &&
            (event.endDate > dateKey ||
                (event.endDate === dateKey && event.endTime !== "00:00"));
    }

    _calendarEventTimeLabel(event, dateKey) {
        if (event.startDate === event.endDate) return event.startTime;
        if (dateKey === event.startDate) return `${event.startTime} →`;
        if (dateKey === event.endDate) return `→ ${event.endTime}`;
        return "All day";
    }

    toggleOverview(force = null) {
        const next = force === null
            ? (this.overviewTarget > 0 ? 0 : 1)
            : Number(Boolean(force));
        this._advanceOverview();
        if (next === this.overviewTarget) return Boolean(next);
        this.overviewStartedProgress = this.overviewProgress;
        this.overviewStartedAt = this.now();
        this.overviewTarget = next;
        if (!next) this.overviewInteraction?.cancel();
        if (next > 0) {
            this.overviewItems = null;
            this.calendarActive = false;
        } else {
            this.overviewSearchQuery = "";
            this.overviewExternalResults = [];
        }
        this.overviewHover = null;
        this.overviewPointerActive = false;
        this.overviewCloseHover = null;
        this.damaged = true;
        this.schedule();
        return Boolean(next);
    }

    closeOverview() {
        return this.toggleOverview(false);
    }

    setOverviewSearchQuery(query) {
        this.overviewSearchQuery = Array.from(String(query || "")).slice(0, 96).join("");
        this.overviewItems = null;
        this.overviewHover = null;
        this.overviewPointerActive = false;
        this.overviewCloseHover = null;
        this.damaged = true;
        this.schedule();
        return this.overviewSearchQuery;
    }

    setOverviewSearchResults(results) {
        this.overviewExternalResults = (Array.isArray(results) ? results : [])
            .slice(0, 24)
            .map(entry => ({
                kind: entry?.kind === "folder" ? "folder" : "file",
                id: String(entry?.path || ""),
                path: String(entry?.path || ""),
                title: String(entry?.title || entry?.path || ""),
                subtitle: entry?.kind === "folder" ? "Folder" : "File",
            }))
            .filter(entry => entry.path.startsWith("/") && entry.title);
        this.overviewItems = null;
        this.damaged = true;
        this.schedule();
    }

    selectFirstOverviewResult() {
        const selected = this.overviewLayout.find(({ item }) =>
            this.overviewHover === `${item.kind}:${item.id}`
        )?.item;
        const first = selected || this.overviewLayout[0]?.item;
        if (!first) return null;
        this.closeOverview();
        return Object.freeze({
            kind: first.kind,
            id: first.id,
            title: first.title,
            ...(first.action ? { action: first.action } : {}),
            ...(first.path ? { path: first.path } : {}),
        });
    }

    moveOverviewSelection(direction, layout = this.overviewLayout) {
        if (!this.overviewActive || layout.length === 0) return null;
        const axis = {
            left: [-1, 0],
            right: [1, 0],
            up: [0, -1],
            down: [0, 1],
        }[direction];
        if (!axis) return null;
        const currentIndex = layout.findIndex(({ item }) =>
            this.overviewHover === `${item.kind}:${item.id}`
        );
        let next;
        if (currentIndex < 0) {
            next = layout[axis[0] < 0 || axis[1] < 0
                ? layout.length - 1 : 0];
        } else {
            const current = layout[currentIndex].hit;
            const centerX = current.x + current.width / 2;
            const centerY = current.y + current.height / 2;
            next = layout
                .filter((_, index) => index !== currentIndex)
                .map(entry => {
                    const x = entry.hit.x + entry.hit.width / 2 - centerX;
                    const y = entry.hit.y + entry.hit.height / 2 - centerY;
                    const primary = x * axis[0] + y * axis[1];
                    const perpendicular = Math.abs(x * axis[1] - y * axis[0]);
                    return { entry, primary, score: primary + perpendicular * 2.5 };
                })
                .filter(candidate => candidate.primary > 1)
                .sort((left, right) => left.score - right.score)[0]?.entry;
            next ||= layout[currentIndex];
        }
        this.overviewHover = `${next.item.kind}:${next.item.id}`;
        this.overviewPointerActive = false;
        this.damaged = true;
        this.schedule();
        return next.item;
    }

    overviewPointer(x, y) {
        if (!this.overviewActive) return null;
        this.overviewPointerActive = true;
        const closeMatch = this.overviewLayout.find(({ item }) => {
            const close = this.overviewCloseRects.get(`${item.kind}:${item.id}`);
            return (item.kind === "native" || item.kind === "x11") &&
                close && x >= close.x && y >= close.y &&
                x < close.x + close.width && y < close.y + close.height;
        });
        const match = this.overviewLayout.find(({ hit }) =>
            x >= hit.x && y >= hit.y &&
            x < hit.x + hit.width && y < hit.y + hit.height
        );
        const hover = match
            ? `${match.item.kind}:${match.item.id}`
            : null;
        const closeHover = closeMatch
            ? `${closeMatch.item.kind}:${closeMatch.item.id}`
            : null;
        if (hover !== this.overviewHover || closeHover !== this.overviewCloseHover) {
            this.overviewHover = hover;
            this.overviewCloseHover = closeHover;
            this.damaged = true;
            this.schedule();
        }
        return match?.item || null;
    }

    closeOverviewWindowAt(x, y) {
        if (!this.overviewActive) return null;
        const match = this.overviewLayout.find(({ item }) => {
            const close = this.overviewCloseRects.get(`${item.kind}:${item.id}`);
            return (item.kind === "native" || item.kind === "x11") &&
                close && x >= close.x && y >= close.y &&
                x < close.x + close.width && y < close.y + close.height;
        });
        if (!match) return null;
        return Object.freeze({
            kind: match.item.kind,
            id: match.item.id,
            title: match.item.title,
        });
    }

    selectOverviewWindow(x, y) {
        const selected = this.overviewPointer(x, y);
        if (!selected) return null;
        this.closeOverview();
        return Object.freeze({
            kind: selected.kind,
            id: selected.id,
            title: selected.title,
            ...(selected.action ? { action: selected.action } : {}),
            ...(selected.path ? { path: selected.path } : {}),
        });
    }

    topX11WindowRect() {
        const root = [...this.windows.values()].find(window =>
            window.parent === window.id
        );
        if (!root) return null;
        const window = [...this.windows.values()]
            .filter(candidate =>
                candidate.parent === root.id &&
                candidate.viewable &&
                !candidate.inputOnly &&
                !candidate.desktopLayer &&
                !candidate.shellLayer
            )
            .sort((left, right) =>
                left.stackIndex - right.stackIndex || left.id - right.id
            )
            .at(-1);
        if (!window) return null;
        return Object.freeze({
            x: window.x - window.borderWidth,
            y: window.y - window.borderWidth,
            width: window.width + window.borderWidth * 2,
            height: window.height + window.borderWidth * 2,
        });
    }

    shellWindowRects() {
        const root = [...this.windows.values()].find(window =>
            window.parent === window.id
        );
        if (!root) return Object.freeze([]);
        return Object.freeze([...this.windows.values()]
            .filter(window =>
                window.id !== root.id &&
                window.parent === root.id &&
                window.viewable &&
                !window.inputOnly &&
                window.shellLayer
            )
            .sort((left, right) =>
                left.stackIndex - right.stackIndex || left.id - right.id
            )
            .map(window => Object.freeze({
                x: window.x - window.borderWidth,
                y: window.y - window.borderWidth,
                width: window.width + window.borderWidth * 2,
                height: window.height + window.borderWidth * 2,
            })));
    }

    setTrayItems(items) {
        this.trayItems = Array.isArray(items) ? [...items] : [];
        this.damaged = true;
        this.schedule();
    }

    setWorkspaceStorageState(state) {
        this.workspaceStorageState = state && typeof state === "object"
            ? { ...state }
            : null;
        this.damaged = true;
        this.schedule();
    }

    schedule() {
        if (!this.damaged || this.frameHandle !== null) return;
        this.frameHandle = this.requestFrame(() => {
            this.frameHandle = null;
            const glass = this.nativeCanvasWindow.frostedIceMaterial;
            const softwareGlass = glass.gpuUnavailable && !glass.webgpu?.pipeline &&
                this.nativeCanvasWindow.snapshots.some(native => native.layerVisible !== false &&
                    native.chrome?.frostedIce?.model === "hyprglass");
            const frameTime = this.now();
            // Software optical surfaces have a 30 fps ceiling during activity;
            // hardware stays at display cadence. Idle never schedules a frame.
            if (softwareGlass && frameTime - (this.lastSoftwareGlassFrameAt ?? -Infinity) < 1000 / 30) {
                this.schedule();
                return;
            }
            this.lastSoftwareGlassFrameAt = frameTime;
            const previousBoundary = this.animationFrameBoundary;
            this.animationFrameBoundary = this.frameSchedulerIsAnimationFrame;
            try {
                this.render();
            } catch (error) {
                this.onFault(error);
            } finally {
                this.animationFrameBoundary = previousBoundary;
            }
        });
    }

    flush() {
        if (this.frameHandle !== null) {
            this.cancelFrame(this.frameHandle);
            this.frameHandle = null;
        }
        if (this.damaged) {
            try {
                this.render();
            } catch (error) {
                this.onFault(error);
            }
        }
    }

}
