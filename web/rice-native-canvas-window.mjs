import {RiceThemeSkinRenderer} from './rice-theme-skin-renderer.mjs';
import {desktopThemeFont} from './rice-theme-customization.mjs';
import {themeColor} from "./rice-theme-customization.mjs";
import {paintPaddyDropOverlay,paddyDropOverlayRect} from "./rice-paddy-drop-overlay.mjs";
import {classicUiFont, ensureClassicUiFonts, paintClassicCaptionGlyph} from "./rice-classic-ui.mjs";
import {RiceNativeSceneLocale} from './rice-native-scene-locale.mjs';
import {riceLocale} from './rice-i18n.mjs';
import { cyberpunkPath, paintCyberpunkFrame } from './rice-cyberpunk-style.mjs';
import { paintMatrixCurtain, paintMatrixFrame, createMatrixMorph, paintMatrixMorph } from './rice-matrix-style.mjs';
import { sampleThemeMotion, paintSignalCurtain, signalFontReady } from "./rice-theme-bundle.mjs";
import { RiceNativeWallpaper } from "./rice-native-wallpaper.mjs";
import {captureTerminalPreviewReplay, finishTerminalImageReplay, terminalGridHasForeground} from './rice-terminal-image-replay.mjs';
import { renderRiceNativeDock } from "./rice-native-dock.mjs";
import {
    rgbaColor,
    readableInk,
    roundedPath,
    resizeCanvasBackingStore,
} from "./rice-native-canvas-primitives.mjs";
import {
    renderRiceWebScene,
    releaseRiceWebSceneTextures,
} from "./rice-native-web-scene-renderer.mjs";
import {
    paintStructuralGlass,
    RiceNativeCommandRenderer,
} from "./rice-native-command-renderer.mjs";
import {
    RiceNativeEditorRenderer,
} from "./rice-native-editor-renderer.mjs";
import {
    RiceNativeIconRenderer,
    RICE_ICON_BOUNCE_DURATION_MS,
    riceIconBounceOffset,
    riceIconName,
    riceNativeIconName,
} from "./rice-native-icon-renderer.mjs";
import { nativeEntrypointIconName } from "./rice-native-launch-policy.mjs";
import {
    nativeTitleIconX,
    nativeTitleControlLayout,
    nativeTitlebarLayout,
    nativeTitleToolLayout,
} from "./rice-native-window-stack.mjs";
import {
    FrostedIceMaterial,
    hyprglassSurfaceCommand,
    setFrostedIcePointerLight,
} from "./rice-frosted-ice.mjs";
import {
    adaptiveStructuralInk,
    expandedRect,
    frameRect,
    globalCommandRect,
    popupSurfaceRects,
    riceNativeCommandIntersectsDamage,
    riceNativeBackdropIntersectsDamage,
    riceNativeOpaqueInterior,
    riceNativeSnapshotDamage,
    sameRect,
} from "./rice-native-canvas-window-damage.mjs";
export {
    adaptiveStructuralInk,
    riceNativeCommandIntersectsDamage,
    riceNativeSnapshotDamage,
};
const MAX_NATIVE_COMMANDS = 65_536;
const DECORATION_ASSET_PATTERN = /^[a-z0-9]+(?:-[a-z0-9]+)*\.png$/;
const DECORATION_ASSET_BASE = new URL("./assets/themes/decorations/", import.meta.url);
export function riceWindowTransitionDuration({
    speed = 55,
    kind = "window",
    direction = "open",
    foldout = false,
} = {}) {
    const numericSpeed = Number(speed);
    const boundedSpeed = Number.isFinite(numericSpeed)
        ? Math.max(0, Math.min(100, numericSpeed))
        : 55;
    const closing = direction === "close";
    if (kind === "menu") {
        return Math.round(Math.max(
            closing ? 75 : 95,
            (closing ? 115 : 165) - boundedSpeed * (closing ? .4 : .7),
        ));
    }
    if (kind === "dialog") {
        return Math.round(Math.max(
            closing ? 100 : 130,
            (closing ? 170 : 250) - boundedSpeed * (closing ? .7 : 1.5),
        ));
    }
    const minimum = closing ? 90 : 135;
    const base = foldout
        ? closing ? 165 : 365
        : closing ? 175 : 285;
    const slope = foldout
        ? closing ? .7 : 2.1
        : closing ? .7 : 1.5;
    return Math.round(Math.max(minimum, base - boundedSpeed * slope));
}
function contrastingControlInk(color) {
    const [red = 0, green = 0, blue = 0] = color || [];
    const linear = channel => {
        channel /= 255;
        return channel <= .04045
            ? channel / 12.92
            : ((channel + .055) / 1.055) ** 2.4;
    };
    const luminance = linear(red) * .2126 + linear(green) * .7152 +
        linear(blue) * .0722;
    return luminance >= .179129
        ? [0, 0, 0, 255]
        : [255, 255, 255, 255];
}
const FILE_ICON_IDS = new Set([
    2, 71,
    89, 90, 91, 92, 93, 94, 97, 98, 99, 100, 101, 102,
    103, 104, 105, 106, 107, 108, 109, 110,
]);
const FILE_MIME_BY_EXTENSION = new Map(Object.entries({
    txt: "text/plain", log: "text/plain", nfo: "text/plain",
    md: "text/markdown", markdown: "text/markdown",
    mdown: "text/markdown", mkd: "text/markdown",
    csv: "text/csv", tsv: "text/tab-separated-values",
    rs: "text/rust", c: "text/x-c", h: "text/x-c",
    cc: "text/x-c++", cpp: "text/x-c++", cxx: "text/x-c++",
    hpp: "text/x-c++",
    js: "application/javascript", mjs: "application/javascript",
    cjs: "application/javascript", jsx: "application/javascript",
    ts: "application/typescript", tsx: "application/typescript",
    json: "application/json", toml: "application/toml",
    yaml: "application/yaml", yml: "application/yaml",
    xml: "application/xml", webmanifest: "application/manifest+json",
    manifest: "application/manifest+json", env: "text/x-env",
    html: "text/html", htm: "text/html", xhtml: "application/xhtml+xml",
    css: "text/css", scss: "text/x-scss", sass: "text/x-sass",
    less: "text/x-less",
    svg: "image/svg+xml", png: "image/png", jpg: "image/jpeg",
    jpeg: "image/jpeg", gif: "image/gif", webp: "image/webp",
    bmp: "image/bmp", avif: "image/avif",
    mp4: "video/mp4", m4v: "video/mp4", webm: "video/webm",
    mkv: "video/x-matroska", ogv: "video/ogg", ogg: "video/ogg",
    mov: "video/quicktime", move: "video/quicktime",
    wmv: "video/x-ms-wmv", asf: "video/x-ms-asf",
    avi: "video/x-msvideo", mpeg: "video/mpeg", mpg: "video/mpeg",
    m2ts: "video/mp2t",
    mp3: "audio/mpeg", wav: "audio/wav", flac: "audio/flac",
    m4a: "audio/mp4", aac: "audio/aac", oga: "audio/ogg",
    opus: "audio/ogg",
    pdf: "application/pdf", zip: "application/zip",
    tar: "application/x-tar", gz: "application/gzip",
    tgz: "application/gzip", bz2: "application/x-bzip2",
    xz: "application/x-xz", zst: "application/zstd",
    "7z": "application/x-7z-compressed", rar: "application/x-rar-compressed",
    doc: "application/msword",
    docx: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    odt: "application/vnd.oasis.opendocument.text", rtf: "application/rtf",
    ppt: "application/vnd.ms-powerpoint",
    pptx: "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    potx: "application/vnd.openxmlformats-officedocument.presentationml.template",
    ppsx: "application/vnd.openxmlformats-officedocument.presentationml.slideshow",
    odp: "application/vnd.oasis.opendocument.presentation",
    otp: "application/vnd.oasis.opendocument.presentation-template",
    xls: "application/vnd.ms-excel",
    xlsx: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    ods: "application/vnd.oasis.opendocument.spreadsheet",
    pem: "application/x-pem-file", key: "application/x-pem-file",
    p12: "application/pkcs12", pfx: "application/pkcs12",
    gpg: "application/pgp-keys", asc: "application/pgp-keys",
    wasm: "application/wasm",
    sh: "text/x-shellscript", bash: "text/x-shellscript",
    zsh: "text/x-shellscript", fish: "text/x-shellscript",
    exe: "application/x-msdownload",
    bat: "application/x-rice-wine-batch", cmd: "application/x-rice-wine-batch",
    appimage: "application/x-appimage",
    deb: "application/vnd.debian.binary-package", rpm: "application/x-rpm",
    so: "application/x-sharedlib", dll: "application/x-sharedlib",
    dylib: "application/x-sharedlib", bin: "application/octet-stream",
    o: "application/x-object", obj: "application/x-object",
    class: "application/java-vm",
}));
const FILE_ICON_BY_EXTENSION = new Map(Object.entries({
    mp3: "audio", wav: "audio", flac: "audio", m4a: "audio",
    aac: "audio", oga: "audio", opus: "audio",
    bin: "binary", o: "binary", obj: "binary", so: "binary",
    dll: "binary", dylib: "binary", class: "binary", wasm: "binary",
    css: "css", scss: "css", sass: "css", less: "css",
    csv: "csv", tsv: "csv",
    doc: "doc", docx: "doc", odt: "doc", rtf: "doc",
    zip: "file-zip", tar: "file-zip", gz: "file-zip",
    bz2: "file-zip", xz: "file-zip", zst: "file-zip",
    "7z": "file-zip", rar: "file-zip", tgz: "file-zip",
    html: "html", htm: "html", xhtml: "html",
    js: "js", mjs: "js", cjs: "js", jsx: "js", ts: "js", tsx: "js",
    json: "manifest", webmanifest: "manifest", manifest: "manifest",
    md: "markdown", markdown: "markdown", mdown: "markdown", mkd: "markdown",
    pdf: "pdf", ppt: "ppt", pptx: "ppt", potx: "ppt", ppsx: "ppt", odp: "ppt", otp: "ppt",
    sh: "program", bash: "program", zsh: "program", fish: "program",
    exe: "program", bat: "program", cmd: "program",
    appimage: "program", deb: "program", rpm: "program",
    pem: "secret", key: "secret", p12: "secret", pfx: "secret",
    gpg: "secret", asc: "secret",
    txt: "text", log: "text", nfo: "text", toml: "text", xml: "text",
    rs: "text", c: "text", h: "text", cc: "text", cpp: "text",
    cxx: "text", hpp: "text",
    mp4: "video", m4v: "video", webm: "video", mkv: "video",
    ogv: "video", ogg: "video", mov: "video", move: "video",
    wmv: "video", asf: "video", avi: "video", mpeg: "video",
    mpg: "video", m2ts: "video",
    xls: "xls", xlsx: "xls", ods: "xls", yaml: "yaml", yml: "yaml",
    svg: "image", png: "image", jpg: "image", jpeg: "image",
    gif: "image", webp: "image", bmp: "image", avif: "image",
}));
const TEXT_FILE_BASENAMES = new Set([
    "license", "licence", "copying", "notice", "authors", "changelog",
    ".gitignore", ".gitattributes", ".dockerignore", ".npmignore",
    ".editorconfig",
]);
export function riceFileMimeType(label) {
    const displayName = String(label || "").split(/\s{4,}/u, 1)[0];
    const basename = displayName.split(/[\\/]/u).at(-1).toLowerCase();
    if (basename === "dockerfile" || basename === "containerfile") {
        return "text/x-dockerfile";
    }
    if (basename === "manifest.json") return "application/manifest+json";
    if (basename === ".env" || basename.startsWith(".env.")) return "text/x-env";
    const dot = basename.lastIndexOf(".");
    return dot >= 0
        ? FILE_MIME_BY_EXTENSION.get(basename.slice(dot + 1)) ||
            "application/octet-stream"
        : "application/octet-stream";
}
export function riceFileIconName(label) {
    const displayName = String(label || "").split(/\s{4,}/u, 1)[0];
    const basename = displayName.split(/[\\/]/u).at(-1).toLowerCase();
    if (basename === "dockerfile" || basename === "containerfile") return "docker";
    if (basename === ".env" || basename.startsWith(".env.")) return "env";
    if (basename === "manifest.json") return "manifest";
    if (TEXT_FILE_BASENAMES.has(basename)) return "text";
    const dot = basename.lastIndexOf(".");
    return dot >= 0
        ? FILE_ICON_BY_EXTENSION.get(basename.slice(dot + 1)) || "file"
        : "file";
}
function hasTerminalPortal(native) {
    return native.commands?.some(command => command.type === "terminal-portal") === true;
}
function derivedChrome(snapshot) {
    const fills = snapshot.commands.filter(command => command.type === "fill");
    const root = fills.reduce((largest, command) =>
        !largest ||
        command.rect.width * command.rect.height >
            largest.rect.width * largest.rect.height
            ? command
            : largest
    , null);
    const border = snapshot.commands.find(
        command => command.type === "border",
    );
    const text = snapshot.commands.find(command => command.type === "text");
    if (!root) {
        throw new TypeError("native UI snapshot has no themed root surface");
    }
    return {
        active: border?.color || root.color,
        inactive: border?.color || root.color,
        border: border?.color || root.color,
        body: root.color,
        text: text?.color || border?.color || root.color,
        control: root.color,
        close: border?.color || root.color,
        pressed: border?.color || root.color,
        radius: root.radius || 0,
    };
}
function titlebarPath(
    context,
    frame,
    layout,
    originX = 0,
    originY = 0,
    begin = true,
) {
    const width = layout.width;
    const height = frame.titleHeight;
    if (begin) context.beginPath();
    if (!layout.compact || width >= frame.width || layout.end === "Straight") {
        context.rect(originX, originY, width, height);
        return;
    }
    context.moveTo(originX, originY);
    if (layout.end === "Inclined") {
        context.lineTo(originX + width - layout.endWidth, originY);
        context.lineTo(originX + width, originY + height);
    } else {
        context.lineTo(originX + width - layout.endWidth, originY);
        context.quadraticCurveTo(
            originX + width,
            originY,
            originX + width,
            originY + height,
        );
    }
    context.lineTo(originX, originY + height);
    context.closePath();
}
function compactTitlebarIsolated(chrome, titlebar) {
    return titlebar?.compact === true &&
        chrome?.windowDecorations
            ?.compactTitlebarFullWidthBackdrop !== true;
}
function paintRedmondBevel(context, rect, pressed = false) {
    const colors = pressed
        ? ["#000000", "#808080", "#dfdfdf", "#ffffff"]
        : ["#ffffff", "#dfdfdf", "#808080", "#000000"];
    context.save();
    try {
        context.lineWidth = 1;
        for (let inset = 0; inset < 2; inset += 1) {
            const x = Math.round(rect.x) + inset + .5;
            const y = Math.round(rect.y) + inset + .5;
            const right = Math.round(rect.x + rect.width) - inset - .5;
            const bottom = Math.round(rect.y + rect.height) - inset - .5;
            context.strokeStyle = colors[inset];
            context.beginPath();
            context.moveTo(x, bottom);
            context.lineTo(x, y);
            context.lineTo(right, y);
            context.stroke();
            context.strokeStyle = colors[3 - inset];
            context.beginPath();
            context.moveTo(right, y);
            context.lineTo(right, bottom);
            context.lineTo(x, bottom);
            context.stroke();
        }
    } finally {
        context.restore();
    }
}
function wedgedWindowPath(context, frame) {
    const cut = Math.max(8, Math.min(
        18,
        Math.floor(Math.min(frame.width, frame.height) * .045),
    ));
    context.moveTo(frame.x + cut, frame.y);
    context.lineTo(frame.x + frame.width, frame.y);
    context.lineTo(frame.x + frame.width, frame.y + frame.height - cut);
    context.lineTo(frame.x + frame.width - cut, frame.y + frame.height);
    context.lineTo(frame.x, frame.y + frame.height);
    context.lineTo(frame.x, frame.y + cut);
    context.closePath();
}
export function appendWindowSilhouette(context, frame, chrome, titlebar) {
    if (chrome?.windowShape === "Wedged") {
        wedgedWindowPath(context, frame);
        return;
    }
    if (!compactTitlebarIsolated(chrome, titlebar)) {
        if (Number(chrome?.radius) <= 0) {
            context.rect(frame.x, frame.y, frame.width, frame.height);
        } else if (typeof context.roundRect === "function") {
            context.roundRect(
                frame.x,
                frame.y,
                frame.width,
                frame.height,
                chrome.radius,
            );
        } else {
            context.rect(frame.x, frame.y, frame.width, frame.height);
        }
        return;
    }
    titlebarPath(context, frame, titlebar, frame.x, frame.y, false);
    context.rect(
        frame.x,
        frame.y + frame.titleHeight,
        frame.width,
        Math.max(0, frame.height - frame.titleHeight),
    );
}
export class RiceNativeCanvasWindow {
    constructor(
        context,
        onDamage = () => {},
        mediaSurface = () => null,
        mediaStatus = () => null,
        onFault = () => {},
        now = () => globalThis.performance?.now?.() || Date.now(),
        onDesktopIconAnimation = null,
        {
            canvasFactory,
            onVisualEffectStatus = () => {},
            decorationImageFactory,
            fonts = globalThis.document?.fonts,
            hostSurface = () => null,
        } = {},
    ) {
        this.context = context;
        this.sceneLocale = new RiceNativeSceneLocale(context.canvas?.ownerDocument, onFault);
        this.onDamage = onDamage;
        void this.sceneLocale.prepare({locale:riceLocale()}).then(onDamage).catch(onFault);
        void signalFontReady.then(()=>this.onDamage());
        // Signal/industrial title labels use Mono. A canvas paint can request
        // it lazily; repaint once loaded instead of retaining fallback glyphs.
        this.chromeFontsDisposed=false;
        this.chromeFontReady=fonts?.load
            ? Promise.resolve().then(()=>fonts.load('11px "RiceWM Mono"')).then(()=>{
                if(!this.chromeFontsDisposed)this.onDamage();
            },()=>{}) : Promise.resolve();
        this.snapshots = [];
        this.activeApp = null;
        this.foregroundActive = false;
        this.dragVisual = null;
        this.snapPreview = null;
        this.layoutPreview = null;
        this.mediaSurface = mediaSurface;
        this.hostSurface = typeof hostSurface === "function" ? hostSurface : () => null;
        this.wallpaper = new RiceNativeWallpaper(context,()=>this.onDamage());
        this.iconRenderer = new RiceNativeIconRenderer(context, onDamage, { monochrome: true });
        this.editorRenderer = new RiceNativeEditorRenderer(context, onDamage, onFault);
        this.frostedIceMaterial = new FrostedIceMaterial({
            ...(typeof canvasFactory === "function" ? { canvasFactory } : {}),
            onStatus: onVisualEffectStatus,
            surface: "native",
        });
        this.terminalBackdrops = new Map();
        this.terminalPreviewReplays = new Map();
        this.liveTerminalBackdrops = null;
        this.onFault = onFault;
        this.now = now;
        this.onDesktopIconAnimation = typeof onDesktopIconAnimation === "function"
            ? onDesktopIconAnimation
            : null;
        this.transitions = new Map();
        this.closingWindows = new Map();
        this.closingPopups = new Map();
        this.desktopApplicationIcons = new Map();
        this.fileAssociationIcons = new Map();
        this.desktopLaunchAnimations = new Map();
        this.desktopHoveredIcon = null;
        this.pointerLightPosition = null;
        this.paintDamageRects = null;
        this.windowContentAlpha = 1;
        this.windowBodyAlpha = 1;
        this.cpuWorkByApp = new Map();
        this.structuralBackdrop = null;
        this.decorationImages = new Map();
        this.decorationImageFactory = typeof decorationImageFactory === "function"
            ? decorationImageFactory
            : source => {
                if (typeof globalThis.Image !== "function") return null;
                const image = new globalThis.Image();
                image.decoding = "async";
                image.src = source;
                return image;
            };
        this.commandRenderer = new RiceNativeCommandRenderer(context, {
            editor: this.editorRenderer,
            icons: this.iconRenderer,
            renderWebScene: scene => renderRiceWebScene(this.context, scene, mediaSurface),
            mediaSurface,
            hostSurface: identity => this.hostSurface(identity),
            mediaStatus,
            onDamage,
            glassMaterial: this.frostedIceMaterial,
        });
    }
    setVisualEffectAccelerationMode(mode) {
        this.frostedIceMaterial.setAccelerationMode(mode);
        this.onDamage();
    }
    setPaintDamage(rects) {
        this.paintDamageRects = Array.isArray(rects)
            ? rects.filter(rect => rect?.width > 0 && rect?.height > 0)
            : null;
    }
    setBackdropOcclusions(rects) {
        this.backdropOcclusions = rects;
    }
    setRenderContext(context) {
        if (!context) return;
        this.context = context;
        this.wallpaper.context = context;
        this.iconRenderer.context = context;
        this.editorRenderer.context = context;
        this.commandRenderer.context = context;
    }
    #decorationImage(asset, customSource = "") {
        const custom = asset.startsWith("/") && customSource.length <= 5_592_432 &&
            /^data:image\/png;base64,[A-Za-z0-9+/]+=*$/.test(customSource);
        if (!custom && !DECORATION_ASSET_PATTERN.test(asset)) return null;
        const source = custom ? customSource : new URL(asset, DECORATION_ASSET_BASE).href;
        if (this.decorationImages.has(source)) return this.decorationImages.get(source);
        if (this.decorationImages.size >= 8) this.decorationImages.delete(this.decorationImages.keys().next().value);
        const image = this.decorationImageFactory(source);
        if (!image) {
            this.decorationImages.set(source, null);
            return null;
        }
        this.decorationImages.set(source, image);
        const redraw = () => this.onDamage();
        if (typeof image.addEventListener === "function") {
            image.addEventListener("load", redraw, { once: true });
        } else if ("onload" in image) {
            image.onload = redraw;
        }
        return image;
    }
    #renderCommandDeckTitleRail(chrome, frame, titlebar, title) {
        const context = this.context;
        const titleHeight = frame.titleHeight;
        const naturalTitleRight = Math.max(
            142,
            titlebar.titleX + String(title || "RiceWM").length * 7.5 + 28,
        );
        const titleRight = Math.max(
            54,
            Math.min(Math.max(54, frame.width - 92), naturalTitleRight),
        );
        const railRight = Math.max(titleRight, frame.width - 8);
        const topHeight = Math.max(3, Math.min(5, Math.floor(titleHeight / 6)));
        const gap = 4;
        context.save();
        try {
            // A few broad color fields read as an LCARS information rail.
            // The former many tiny atlas fragments looked like a dashed
            // border and never connected visually to the title capsule.
            const firstWidth = Math.min(48, Math.max(20, titleRight * .34));
            context.fillStyle = rgbaColor(chrome.control);
            context.fillRect(0, 0, firstWidth, topHeight);
            context.fillStyle = rgbaColor(chrome.border);
            context.fillRect(
                firstWidth + gap,
                0,
                Math.max(0, titleRight - firstWidth - gap),
                topHeight,
            );
            let railX = titleRight + 7;
            const available = Math.max(0, railRight - railX);
            const segmentWidths = [
                Math.floor(available * .2),
                Math.floor(available * .46),
                Math.max(0, available - Math.floor(available * .66) - gap * 2),
            ];
            const segmentColors = [chrome.control, chrome.border, chrome.pressed];
            for (let index = 0; index < segmentWidths.length; index += 1) {
                const width = Math.max(0, segmentWidths[index]);
                if (width > 0) {
                    context.fillStyle = rgbaColor(segmentColors[index]);
                    context.fillRect(railX, 0, width, topHeight);
                }
                railX += width + gap;
            }
            // Keep the application title typographic. The segmented upper
            // rail provides enough LCARS identity without enclosing every
            // title in another capsule or border.
        } finally {
            context.restore();
        }
    }
    #renderDecorationAtlas(chrome, frame, titlebar, layer) {
        const decoration = chrome.decoration;
        if (
            decoration?.layout !== "hud-frame" ||
            decoration.opacity <= 0 ||
            !decoration.asset
        ) return;
        const image = this.#decorationImage(decoration.asset, decoration.source);
        if (!image || image.complete === false || !(image.naturalWidth || image.width)) return;
        const context = this.context;
        context.save();
        try {
            // Decoration atlases use a black field. Screen blending makes it
            // neutral while retaining the authored luminous/colored pixels.
            context.globalCompositeOperation = "screen";
            if (chrome.chromeProfile === "command-deck") {
                if (layer === "title") {
                    // Preserve the authored artwork as a restrained luminous
                    // texture inside the vector rail. It no longer dictates
                    // geometry or turns the complete top edge into dashes.
                    context.globalAlpha = decoration.opacity * .24;
                    context.drawImage(image, 28, 54, 1695, 66,
                        0, 1, frame.width, 3);
                }
                // Command Deck artwork belongs to the title rail only. Body
                // and bottom decals created a broken left edge and competed
                // with application status bars instead of reading as LCARS.
                return;
            }
            if (layer === "title") {
                // Both corner pieces are composited after content so their
                // complete authored legs remain visible across the title/body
                // boundary instead of being clipped to titlebar height.
                return;
            }
            const lowerCornerWidth = Math.min(68, frame.width / 6);
            const lowerCornerHeight = Math.min(78, frame.height / 4);
            context.globalAlpha = decoration.opacity * .72;
            // The source corner points right/down, so its vertical leg sits
            // naturally flush with the upper-right window edge.
            context.drawImage(image, 1015, 240, 205, 245,
                Math.max(0, frame.width - lowerCornerWidth), 0,
                lowerCornerWidth, lowerCornerHeight);
            // Rotate the same authored corner for the lower-left edge.
            context.save();
            context.translate(lowerCornerWidth, frame.height);
            context.rotate(Math.PI);
            context.drawImage(image, 1015, 240, 205, 245,
                0, 0, lowerCornerWidth, lowerCornerHeight);
            context.restore();
            // No bottom-right reticle or floating rail: that corner remains
            // available for application resize/status affordances.
        } finally {
            context.restore();
        }
    }
    beginBackdropEpoch(epoch) {
        this.frostedIceMaterial.beginBackdropEpoch?.(epoch);
    }
    invalidateBackdrop() {
        this.frostedIceMaterial.invalidateBackdrop?.();
    }
    setDesktopApplicationIcons(entries) {
        this.desktopApplicationIcons = new Map(
            (Array.isArray(entries) ? entries : []).flatMap(entry => {
                const label = String(entry?.label || "");
                const icon = riceIconName(entry?.iconName);
                return label && icon ? [[label, icon]] : [];
            }),
        );
        this.onDamage();
    }
    setApplicationIconAssets(assets) {
        this.iconRenderer.setApplicationIconAssets(assets);
    }
    setFileAssociationIcons(entries) {
        this.fileAssociationIcons = new Map(
            (Array.isArray(entries) ? entries : []).flatMap(entry => {
                const mimeType = String(entry?.mimeType || "").toLowerCase();
                const icon = riceIconName(entry?.iconName);
                return mimeType.includes("/") && icon
                    ? [[mimeType, icon]]
                    : [];
            }),
        );
        this.onDamage();
    }
    animateDesktopLaunchAt(x, y) {
        const topmost = this.snapshots.findLast(snapshot => {
            if (snapshot.layerVisible === false) return false;
            const frame = snapshot.frame;
            return frame &&
                Number(x) >= frame.x && Number(y) >= frame.y &&
                Number(x) < frame.x + frame.width &&
                Number(y) < frame.y + frame.height;
        });
        // Desktop feedback belongs only to clicks whose resolved topmost
        // surface is the desktop. Never animate an icon through an app window.
        if (topmost?.role !== "desktop") return false;
        const native = this.snapshots.findLast(snapshot =>
            snapshot.role === "desktop" && snapshot.layerVisible !== false
        );
        if (!native || native.shell?.motionEnabled !== true) return false;
        const commands = this.#desktopCommandsWithApplicationIcons(
            native.commands,
        );
        const pointX = Number(x) - native.frame.x;
        const pointY = Number(y) - native.frame.y;
        for (const command of commands) {
            if (command.type !== "icon" || !command.namedIcon) continue;
            const rect = command.rect;
            const hit = {
                x: rect.x - 16,
                y: rect.y - 8,
                width: rect.width + 32,
                height: rect.height + 42,
            };
            if (
                pointX < hit.x || pointY < hit.y ||
                pointX >= hit.x + hit.width || pointY >= hit.y + hit.height
            ) continue;
            const key = this.#desktopLaunchKey(command);
            const now = this.now();
            const active = this.desktopLaunchAnimations.get(key);
            if (
                active &&
                now - active.startedAt < RICE_ICON_BOUNCE_DURATION_MS
            ) return true;
            const glyph = this.iconRenderer.glyph(command.namedIcon);
            const animation = {
                key,
                startedAt: now,
                duration: RICE_ICON_BOUNCE_DURATION_MS,
                external: Boolean(this.onDesktopIconAnimation && glyph),
                icon: command.namedIcon,
                glyph,
                rect: {
                    x: native.frame.x + rect.x,
                    y: native.frame.y + rect.y,
                    width: rect.width,
                    height: rect.height,
                },
                color: native.chrome?.icon || command.color,
            };
            this.desktopLaunchAnimations.clear();
            this.desktopLaunchAnimations.set(key, animation);
            this.onDamage([expandedRect(animation.rect, 16)]);
            this.onDesktopIconAnimation?.(Object.freeze({
                ...animation,
                rect: Object.freeze({ ...animation.rect }),
                color: Object.freeze([...(animation.color || [])]),
            }));
            return true;
        }
        return false;
    }
    finishDesktopLaunchAnimation(key) {
        const animation = this.desktopLaunchAnimations.get(String(key || ""));
        if (!animation?.external) return false;
        this.desktopLaunchAnimations.delete(animation.key);
        this.onDamage([expandedRect(animation.rect, 16)]);
        return true;
    }
    pointerHoverAt(x, y) {
        const nextLight = Number.isFinite(x) && Number.isFinite(y)
            ? { x: Number(x), y: Number(y) }
            : null;
        const lightChanged = setFrostedIcePointerLight(x, y);
        const pointerLightEnabled = this.snapshots.some(snapshot =>
            snapshot.shell?.frostedIce?.enabled === true &&
            snapshot.shell?.frostedIce?.pointerLight === true
        );
        this.pointerLightPosition = nextLight;
        const repaintCompleteComposition = lightChanged && pointerLightEnabled;
        const native = this.snapshots.findLast(snapshot =>
            snapshot.role === "desktop" && snapshot.layerVisible !== false
        );
        let hovered = null;
        if (native && Number.isFinite(x) && Number.isFinite(y)) {
            const commands = this.#desktopCommandsWithApplicationIcons(native.commands);
            const pointX = Number(x) - native.frame.x;
            const pointY = Number(y) - native.frame.y;
            for (const command of commands) {
                if (command.type !== "icon" || !command.namedIcon) continue;
                const rect = command.rect;
                if (
                    pointX >= rect.x - 16 && pointY >= rect.y - 8 &&
                    pointX < rect.x + rect.width + 16 &&
                    pointY < rect.y + rect.height + 42
                ) {
                    hovered = this.#desktopLaunchKey(command);
                    break;
                }
            }
        }
        if (hovered === this.desktopHoveredIcon) {
            if (repaintCompleteComposition) this.onDamage();
            return repaintCompleteComposition;
        }
        const previousHover = this.desktopHoveredIcon;
        this.desktopHoveredIcon = hovered;
        const damage = [];
        for (const key of [previousHover, hovered]) {
            if (!key) continue;
            const command = this.#desktopCommandsWithApplicationIcons(
                native?.commands || [],
            ).find(candidate =>
                candidate.type === "icon" &&
                candidate.namedIcon && this.#desktopLaunchKey(candidate) === key
            );
            const rect = globalCommandRect(native, command);
            if (rect) damage.push({
                x: rect.x - 16,
                y: rect.y - 8,
                width: rect.width + 32,
                height: rect.height + 50,
            });
        }
        if (repaintCompleteComposition) {
            this.onDamage();
        } else {
            this.onDamage(damage);
        }
        return true;
    }
    renderDesktopLaunchAnimations() {
        const shell = this.snapshot?.shell;
        if (shell?.motionEnabled !== true) {
            this.desktopLaunchAnimations.clear();
            return;
        }
        for (const [key, animation] of this.desktopLaunchAnimations) {
            if (animation.external) continue;
            const elapsed = this.now() - animation.startedAt;
            if (elapsed >= RICE_ICON_BOUNCE_DURATION_MS) {
                this.desktopLaunchAnimations.delete(key);
                continue;
            }
            this.iconRenderer.renderNamed({
                icon: animation.icon,
                color: animation.color,
                hovered: this.desktopHoveredIcon === key,
                rect: {
                    ...animation.rect,
                    y: animation.rect.y + riceIconBounceOffset(
                        elapsed,
                        animation.rect.height,
                    ),
                },
            });
            this.onDamage([expandedRect(animation.rect, 16)]);
        }
    }
    get snapshot() {
        const visible = this.snapshots.filter(snapshot =>
            snapshot.layerVisible !== false
        );
        return visible.find(snapshot =>
            snapshot.app === this.activeApp
        ) || visible.at(-1) || null;
    }
    prepareLocale(snapshot) {
        return this.sceneLocale.prepare(snapshot);
    }

    apply(snapshot) {
        this.commandRenderer.terminalTheme = snapshot?.shell?.terminalTheme || null;
        if (snapshot === null) {
            const damage = this.snapshots.map(frameRect).filter(Boolean);
            if (this.identifyingDisplays) damage.push(...(this.outputRects || []));
            this.identifyingDisplays = false;
            this.snapshots = [];
            this.liveApps = null;
            this.activeApp = null;
            this.foregroundActive = false;
            this.dragVisual = null;
            this.snapPreview = null;
            this.transitions.clear();
            this.closingWindows.clear();
            this.closingPopups.clear();
            this.iconRenderer.setThemeColor(null);
            this.iconRenderer.setThemeHoverColor(null);
            this.iconRenderer.setThemeHoverBackground(null);
            this.iconRenderer.setThemeIcons("monochrome");
            this.onDamage(damage);
            return;
        }
        const previous = new Map(this.snapshots.map(native => [native.app, native]));
        const previousLiveApps = this.liveApps;
        this.liveApps = Array.isArray(snapshot.liveApps) ? new Set(snapshot.liveApps.map(String)) : null;
        const snapshots = Array.isArray(snapshot?.windows)
            ? snapshot.windows
            : [snapshot];
        if (snapshots.length < 1 || snapshots.length > 128) {
            throw new RangeError("native UI window stack is out of bounds");
        }
        for (const native of snapshots) this.#validate(native);
        this.commandRenderer.terminalTheme = snapshots.at(-1)?.shell?.terminalTheme || null;
        this.iconRenderer.setThemeColor(
            snapshots.findLast(native => Array.isArray(native.chrome?.icon))
                ?.chrome.icon || null,
        );
        this.iconRenderer.setThemeHoverColor(
            snapshots.findLast(native => Array.isArray(native.chrome?.iconHover))
                ?.chrome.iconHover || null,
        );
        this.iconRenderer.setThemeHoverBackground(
            snapshots.findLast(native => Array.isArray(native.chrome?.selection))
                ?.chrome.selection || null,
        );
        const themedIcons = snapshots.findLast(native => native.chrome?.iconVariant);
        this.iconRenderer.setThemeIcons(
            themedIcons?.chrome.iconVariant,
            themedIcons?.chrome.iconOverrides,
            themedIcons?.chrome.iconThemeName,
            themedIcons?.chrome.themeIcons,
        );
        const startedAt = this.now();
        const nextApps = new Set(snapshots.map(native => String(native.app)));
        for (const before of this.snapshots) {
            const app = String(before.app);
            if (nextApps.has(app)) continue;
            for (const key of this.closingPopups.keys()) if (key.startsWith(`popup:${app}:`)) this.closingPopups.delete(key);
            // Retire child animations with their owner. Only the window's
            // explicit closing transition may survive removal from the stack.
            for (const key of this.transitions.keys()) {
                if (key === `window:${app}` || key === `layout:${app}` || key === `editor-preview:${app}` ||
                    key.startsWith(`popup:${app}:`) || key.startsWith(`dialog:${app}:`)) {
                    this.transitions.delete(key);
                }
            }
            // Workspace visibility is not application lifetime. Moving away
            // must not leave a closing shutter on the current desktop.
            if (this.liveApps?.has(app) || before.layerVisible === false ||
                before.role === "desktop" || before.shell?.motionEnabled !== true) {
                this.closingWindows.delete(app);
                continue;
            }
            if (before.shell?.motion?.close === false) {
                this.transitions.delete(`window:${app}`);
                this.closingWindows.delete(app);
                continue;
            }
            this.closingWindows.set(app, before);
            this.transitions.set(`window:${app}`, {
                startedAt,
                direction: "close",
            });
        }
        for (const native of snapshots) {
            if (native.shell?.motionEnabled !== true) {
                this.transitions.delete(`layout:${native.app}`);
                continue;
            }
            const before = previous.get(native.app);
            const layoutKey = `layout:${native.app}`;
            const layout = native.layoutTransition;
            if (before && layout && layout.revision !== before.layoutTransition?.revision) {
                const from = this.#layoutFrame(before);
                this.transitions.set(layoutKey, {startedAt, from, to: native.frame});
            } else if (!layout || (before && ['x','y','width','height'].some(
                field => before.frame[field] !== native.frame[field]))) {
                this.transitions.delete(layoutKey);
            }
            this.closingWindows.delete(String(native.app));
            if (!before && !previousLiveApps?.has(String(native.app)) && native.layerVisible !== false && native.role !== "desktop") this.transitions.set(`window:${native.app}`, {
                startedAt,
                direction: "open",
            });
            const popup = popupIdentity(native);
            const previousPopup = popupIdentity(before);
            if (previousPopup && previousPopup !== popup) {
                const key = `popup:${native.app}:${previousPopup}`;
                const definition = before.shell?.motion?.definitions?.['menu.close'];
                if (definition && definition.preset !== 'Instant' && definition.duration > 0) {
                    this.closingPopups.set(key, before);
                    this.transitions.set(key, { startedAt, direction: 'close' });
                } else this.transitions.delete(key);
            }
            if (popup && popup !== previousPopup) {
                const key = `popup:${native.app}:${popup}`;
                this.closingPopups.delete(key);
                this.transitions.set(key, { startedAt });
            }
            const dialog = native.dialog?.widget ? String(native.dialog.widget) : "";
            const previousDialog = before?.dialog?.widget
                ? String(before.dialog.widget)
                : "";
            if (previousDialog && previousDialog !== dialog) {
                this.transitions.delete(`dialog:${native.app}:${previousDialog}`);
            }
            if (dialog && dialog !== previousDialog) {
                this.transitions.set(`dialog:${native.app}:${dialog}`, { startedAt });
            }
            const editorPreview = editorPreviewRect(native);
            const previousEditorPreview = editorPreviewRect(before);
            if (editorPreview && !previousEditorPreview) {
                this.transitions.set(`editor-preview:${native.app}`, {
                    startedAt,
                    direction: "open",
                    rect: editorPreview,
                });
            } else if (!editorPreview && previousEditorPreview) {
                this.transitions.set(`editor-preview:${native.app}`, {
                    startedAt,
                    direction: "close",
                    rect: previousEditorPreview,
                    commands: before.commands,
                });
            }
        }
        const damage = riceNativeSnapshotDamage(this.snapshots, snapshots);
        const previousDrop=paddyDropOverlayRect(this.snapshots,this.dragVisual);
        const nextDrag=validDragVisual(snapshot.dragVisual);
        const nextDrop=paddyDropOverlayRect(snapshots,nextDrag);
        if(!sameRect(previousDrop,nextDrop)) {
            if(previousDrop)damage.push(previousDrop);
            if(nextDrop)damage.push(nextDrop);
        }
        const snapPreview = validSnapPreview(snapshot.snapPreview);
        if (!sameRect(this.snapPreview, snapPreview)) {
            if (this.snapPreview) damage.push(this.snapPreview);
            if (snapPreview) damage.push(snapPreview);
        }
        const identifying = snapshots.some(native => native.entrypoint === "rice-settings" &&
            native.layerVisible !== false && !native.minimized &&
            native.commands.some(command => command.type === "text" && command.text === "Apply arrangement"));
        if (identifying !== Boolean(this.identifyingDisplays)) damage.push(...(this.outputRects || []));
        this.identifyingDisplays = identifying;
        this.snapshots = snapshots.slice();
        const liveApps = new Set(snapshots.map(native => String(native.app)));
        for (const app of this.cpuWorkByApp.keys()) if (!liveApps.has(app)) this.cpuWorkByApp.delete(app);
        this.activeApp = snapshot.activeApp === undefined
            ? snapshots.at(-1).app
            : String(snapshot.activeApp);
        this.foregroundActive = snapshot.foregroundActive !== false;
        this.dragVisual = nextDrag;
        this.snapPreview = snapPreview;
        this.onDamage(damage);
    }
    render(root) {
        this.renderDesktop(root);
        this.renderWindows(root);
        renderRiceNativeDock(
            this.context,
            this.snapshots.at(-1)?.shell,
            this.snapshots,
            root,
            this.iconRenderer,
            this.frostedIceMaterial,
        );
        this.renderOverlays(root);
        this.renderPopupOverlays(root);
        this.renderPromotedDesktopDialogs(root);
        this.finishFrame();
    }
    finishFrame() {
        const motionDamage = this.#activeMotionDamage();
        if (motionDamage.length > 0) {
            // Transforms and shadows extend beyond the untransformed frame.
            // Reconstruct all planes until the final settled frame is painted.
            this.onDamage(this.transitions.size > 0 ? null : motionDamage);
        }
    }
    #activeMotionDamage() {
        const damage = [];
        for (const native of this.snapshots) {
            if (
                native.layerVisible === false ||
                native.shell?.motionEnabled === false
            ) continue;
            if ([...this.transitions.keys()].some(key =>
                key.includes(`:${native.app}`)
            )) {
                const rect = frameRect(native);
                if (rect) damage.push(rect);
                continue;
            }
            const frame=native.frame,viewport=this.context.canvas;
            const clip=native.tileClip;
            if(clip&&frame&&(frame.x+frame.width<=clip.x||frame.x>=clip.x+clip.width||frame.y+frame.height<=clip.y||frame.y>=clip.y+clip.height))continue;
            if(native.role!=='desktop'&&frame&&viewport&&
                (frame.x+frame.width < -32 || frame.x > viewport.width+32 ||
                 frame.y+frame.height < -32 || frame.y > viewport.height+32))continue;
            for (const command of [
                ...(native.commands || []),
                ...(native.popupCommands || []),
            ]) {
                if (
                    command.type !== "spinner" &&
                    command.type !== "activity" &&
                    !(command.type === "icon" && command.animated === true)
                ) continue;
                const rect = globalCommandRect(native, command);
                if (rect) damage.push(expandedRect(rect, 4));
            }
        }
        for (const animation of this.desktopLaunchAnimations.values()) {
            if (!animation.external) damage.push(expandedRect(animation.rect, 16));
        }
        for (const [app, native] of this.closingWindows) {
            if (!this.transitions.has(`window:${app}`)) continue;
            const rect = frameRect(native);
            if (rect) damage.push(rect);
        }
        for (const native of this.closingPopups.values()) {
            const popup = native.popup?.rect;
            if (popup) damage.push(expandedRect({x:native.frame.x+popup.x, y:native.frame.y+(native.role==='desktop'?0:native.frame.titleHeight)+popup.y, width:popup.width, height:popup.height},16));
        }
        return damage.filter(Boolean);
    }
    renderDesktop(root, { extendWallpaperToTop = false } = {}) {
        const desktopRoot = extendWallpaperToTop ? { ...root, wallpaperTop: 0 } : root;
        for (const native of this.snapshots.filter(
            snapshot => snapshot.role === "desktop",
        )) {
            this.invalidateBackdrop();
            this.#renderWindowContained(native, desktopRoot);
        }
    }
    renderWindows(root) {
        this.liveTerminalBackdrops = new Set();
        const windows = this.snapshots.filter(snapshot => snapshot.role !== "desktop" && snapshot.layerVisible !== false);
        const cpuTiles = this.frostedIceMaterial.gpuUnavailable && !this.frostedIceMaterial.webgpu?.pipeline &&
            windows.length > 0 && windows.every((native,index)=>native.chrome?.frostedIce?.model === 'hyprglass' &&
                windows.slice(0,index).every(other=>native.frame.x >= other.frame.x+other.frame.width ||
                    other.frame.x >= native.frame.x+native.frame.width || native.frame.y >= other.frame.y+other.frame.height ||
                    other.frame.y >= native.frame.y+native.frame.height));
        this.cpuTileBackdrop = null;
        if (cpuTiles) {
            // Disjoint tiles all see the same desktop underlay. Snapshot that
            // underlay once at the CPU optical resolution, before painting text.
            const width=Math.ceil(root.width/4),height=Math.ceil(root.height/4);
            this.tileBackdrop ||= this.frostedIceMaterial.canvasFactory(width,height);
            if (resizeCanvasBackingStore(this.tileBackdrop,width,height)) this.tileBackdropKey=null;
            const backdropKey=this.desktopBackdropKey == null ? null : JSON.stringify([
                this.desktopBackdropKey,this.snapshots.filter(native=>native.role==='desktop')],
                (_key,value)=>typeof value==='bigint'?String(value):value);
            if (backdropKey == null || this.tileBackdropKey !== backdropKey) {
                const target=this.tileBackdrop.getContext('2d',{willReadFrequently:true});
                target.globalCompositeOperation='copy';
                target.drawImage(this.context.canvas,0,0,root.width/4,root.height/4);
                this.tileBackdropKey=backdropKey;
            }
            this.cpuTileBackdrop=this.tileBackdrop;
        } else {
            this.tileBackdrop = null;
            this.tileBackdropKey = null;
        }
        const prefixLength = windows.length - 1;
        // Only retained layers must be static. The top window is repainted on
        // every pass, including video, portals, controls, and their effects.
        const retainedWindows = [
            ...this.snapshots.filter(native => native.role === "desktop"),
            ...windows.slice(0, prefixLength),
        ];
        const staticCommands = retainedWindows.every(native => !native.popup && !native.dialog &&
            (native.commands || []).every(command =>
                !["image", "media", "inline-image", "web-scene", "host-portal", "terminal-portal", "spinner", "activity"].includes(command.type) && !command.animated && !command.row?.image));
        const reusable = prefixLength > 0 && root.width * root.height <= 4_194_304 &&
            !this.paintOrigin && !Array.isArray(this.paintDamageRects) &&
            this.staticBackdropKey != null && staticCommands &&
            !this.closingWindows.size && windows.every(native => native.chrome?.frostedIce?.model === "hyprglass");
        const key = reusable ? JSON.stringify([this.staticBackdropKey,
            this.snapshots.filter(native => native.role === "desktop"), windows.slice(0, prefixLength)],
            (_key, value) => typeof value === "bigint" ? String(value) : value) : null;
        let first = 0;
        if (key && this.staticPrefix?.key === key) {
            this.context.drawImage(this.staticPrefix.canvas, 0, 0);
            first = prefixLength;
            this.staticPrefixReuses = (this.staticPrefixReuses || 0) + 1;
        } else if (!key) this.staticPrefix = null;
        for (let index = first; index < windows.length; index++) {
            const native = windows[index];
            this.invalidateBackdrop();
            this.#renderWindowContained(native, root);
            if (key && index === prefixLength - 1) {
                let canvas = this.staticPrefix?.canvas;
                canvas ||= this.frostedIceMaterial.canvasFactory(root.width, root.height);
                resizeCanvasBackingStore(canvas, root.width, root.height);
                const target = canvas?.getContext?.("2d");
                if (target) {
                    target.globalCompositeOperation = "copy";
                    target.drawImage(this.context.canvas, 0, 0);
                    this.staticPrefix = {key, canvas};
                }
            }
        }
        this.cpuTileBackdrop = null;
        for (const [app, native] of this.closingWindows) {
            const key = `window:${app}`;
            if (!this.transitions.has(key)) {
                this.closingWindows.delete(app);
                continue;
            }
            this.invalidateBackdrop();
            this.#renderWindowContained(native, root);
            if (!this.transitions.has(key)) this.closingWindows.delete(app);
        }
        for (const key of this.terminalBackdrops.keys()) {
            if (!this.liveTerminalBackdrops.has(key)) {
                this.terminalBackdrops.delete(key);
            }
        }
        this.liveTerminalBackdrops = null;
    }
    terminalBackdrop(app, session) {
        return this.terminalBackdrops.get(`${String(app)}:${String(session)}`) || null;
    }
    renderTaskPreview(taskbarId, targetContext, rect) {
        const native = this.snapshots.find(snapshot =>
            Number(snapshot.taskbarId) === Number(taskbarId) ||
            Number(snapshot.dialogTaskbarId) === Number(taskbarId)
        );
        return this.renderSnapshotPreview(native,targetContext,rect);
    }
    renderSnapshotPreview(native,targetContext,rect,options = {}) {
        return this.#renderApplication(native,targetContext,rect,options);
    }
    // A seated application replays its real display commands in CSS pixels.
    // The caller supplies a DPR backing store; no preview image is captured,
    // sampled, or enlarged. Hosted clients supply their original DOM separately.
    renderDirectApplication(native,targetContext,{onDamage,paper} = {}) {
        if (!native?.frame) return false;
        return this.#renderApplication(native,targetContext,
            {x:0,y:0,width:native.frame.width,height:native.frame.height},
            {onDamage,paper,direct:true});
    }
    #renderApplication(native,targetContext,rect,{onDamage,imageOnly = false,paper,direct = false} = {}) {
        if (!native || !targetContext || !rect) return false;
        const hostPortal = native.commands?.find(command => command.type === "host-portal");
        const hostSurface = hostPortal && !direct ? this.hostPreview?.(native) : null;
        if (hostPortal && !hostSurface && !direct) return false;
        const frame = native.frame;
        const scale = Math.min(
            rect.width / Math.max(1, frame.width),
            rect.height / Math.max(1, frame.height),
        );
        const drawWidth = frame.width * scale;
        const drawHeight = frame.height * scale;
        const offsetX = rect.x + (rect.width - drawWidth) / 2;
        const offsetY = rect.y + (rect.height - drawHeight) / 2;
        const renderers = [
            this,
            this.wallpaper,
            this.iconRenderer,
            this.editorRenderer,
            this.commandRenderer,
        ];
        const contexts = renderers.map(renderer => renderer.context);
        for (const renderer of renderers) renderer.context = targetContext;
        // This is a complete app-local replay, not a desktop damage tile.
        // Never inherit the screen clip/origin or freeze a desktop transition
        // into a retained world texture. Construct owns its materialization.
        const previousDamage = this.paintDamageRects;
        const previousOrigin = this.paintOrigin;
        const previousMotionScale = this.commandRenderer.motionRasterScale;
        const previousCommandOrigin = this.commandRenderer.paintOrigin;
        const previousCommandDamage = this.commandRenderer.onDamage;
        const previousImageCapture = this.commandRenderer.terminalImageCapture;
        const replayKey = JSON.stringify([rect,targetContext.canvas?.width,targetContext.canvas?.height,
            targetContext.getTransform?.(),this.commandRenderer.terminalTheme]);
        const priorReplay = this.terminalPreviewReplays.get(targetContext);
        const imageReplay = !direct && imageOnly && priorReplay?.native === native && priorReplay.key === replayKey &&
            priorReplay.onDamage === onDamage ? priorReplay.image : null;
        let capturedImage = null, imageCaptures = 0;
        this.commandRenderer.terminalImageCapture = direct || imageReplay ? null : (context, image) => {
            imageCaptures++;
            capturedImage = imageCaptures === 1 ? captureTerminalPreviewReplay(this, native, context, image) : null;
        };
        this.paintDamageRects = null;
        this.paintOrigin = null;
        this.commandRenderer.motionRasterScale = null;
        this.commandRenderer.paintOrigin = null;
        if (onDamage) this.commandRenderer.onDamage = onDamage;
        targetContext.save();
        try {
            targetContext.beginPath();
            targetContext.rect(rect.x, rect.y, rect.width, rect.height);
            targetContext.clip();
            if (imageReplay) {
                targetContext.setTransform(1,0,0,1,0,0);
                const damage = imageReplay.damage;
                targetContext.clearRect(damage.x,damage.y,damage.width,damage.height);
                targetContext.drawImage(imageReplay.background,damage.x,damage.y);
                imageReplay.paint(targetContext, null);
                this.terminalPreviewReplayFrames = (this.terminalPreviewReplayFrames || 0) + 1;
                return true;
            }
            // A real app face is opaque. Desktop glass/transparency must not
            // let the code-covered back of its 3D casing show through content.
            targetContext.fillStyle = native.commands?.some(c => c.type === 'terminal-grid')
                ? `#${(this.commandRenderer.terminalTheme?.background ?? 0x10141c).toString(16).padStart(6,'0')}`
                : rgbaColor((native.chrome || derivedChrome(native)).surface || [8,16,12,255]);
            if(!paper)targetContext.fillRect(offsetX, offsetY, drawWidth, drawHeight);
            targetContext.translate(offsetX, offsetY);
            targetContext.scale(scale, scale);
            targetContext.translate(-frame.x, -frame.y);
            this.#renderWindowContained(native, {
                preview: true,
                paper,
                width: Math.max(frame.x + frame.width, 1),
                height: Math.max(frame.y + frame.height, 1),
            });
            if (hostSurface) {
                // Replay the same full window as every native preview, then
                // fill its portal with the renderer-owned content pixels.
                targetContext.drawImage(hostSurface,
                    frame.x + hostPortal.rect.x,
                    frame.y + frame.titleHeight + hostPortal.rect.y,
                    hostPortal.rect.width, hostPortal.rect.height);
            }
            finishTerminalImageReplay(capturedImage, targetContext, null);
            this.terminalPreviewReplays.delete(targetContext);
            if (capturedImage) {
                this.terminalPreviewReplays.set(targetContext,{native,key:replayKey,onDamage,image:capturedImage});
                // Four bounded underlays, at most 16 MiB, independent of the
                // registry's borrowed app surfaces and their disposal.
                if (this.terminalPreviewReplays.size > 4) this.terminalPreviewReplays.delete(this.terminalPreviewReplays.keys().next().value);
            }
        } finally {
            targetContext.restore();
            this.paintDamageRects = previousDamage;
            this.paintOrigin = previousOrigin;
            this.commandRenderer.motionRasterScale = previousMotionScale;
            this.commandRenderer.paintOrigin = previousCommandOrigin;
            this.commandRenderer.onDamage = previousCommandDamage;
            this.commandRenderer.terminalImageCapture = previousImageCapture;
            renderers.forEach((renderer, index) => {
                renderer.context = contexts[index];
            });
        }
        return true;
    }
    dispose() {
        this.terminalPreviewReplays.clear();
        releaseRiceWebSceneTextures(this.context);
        this.commandRenderer.releaseInlineImages?.();
        this.chromeFontsDisposed=true;
        this.cpuTileBackdrop = this.tileBackdrop = null;
        this.snapshots = [];
        this.activeApp = null;
        this.foregroundActive = false;
        this.dragVisual = null;
        this.snapPreview = null;
        this.iconRenderer.dispose();
        this.staticPrefix = null;
        this.frostedIceMaterial.dispose();
        this.terminalBackdrops.clear();
        this.liveTerminalBackdrops = null;
        this.transitions.clear();
        this.closingWindows.clear();
        this.closingPopups.clear();
        this.desktopApplicationIcons.clear();
        this.fileAssociationIcons.clear();
        this.desktopLaunchAnimations.clear();
        this.desktopHoveredIcon = null;
        this.structuralBackdrop = null;
    }
    renderOverlays(root) {
        this.#renderDisplayNumbers();
        this.#renderSnapPreview(root);
        this.#renderDragVisual(root);
        const native=this.snapshots.findLast(window=>window.layerVisible!==false&&window.titleControlTooltip);
        if(native){
            const context=this.context,chrome=native.chrome,frame=native.frame;
            const control=nativeTitleControlLayout(frame,native.title,native.titleTools,chrome.windowDecorations).find(item=>item.action===native.hoveredControl);
            if(control){
                context.save();context.font='12px "RiceWM Sans", "RiceWM CJK", sans-serif';
                const text=native.titleControlTooltip,width=Math.min(root.width-16,context.measureText(text).width+20),height=30;
                const x=Math.max(8,Math.min(root.width-width-8,frame.x+control.cell.x+control.cell.width-width));
                const y=Math.max(8,Math.min(root.height-height-8,frame.y+frame.titleHeight+5));
                const rect={x,y,width,height};
                context.fillStyle=rgbaColor(chrome.surface||chrome.control);context.strokeStyle=rgbaColor(chrome.selection||chrome.border);context.lineWidth=1;
                roundedPath(context,rect,['Wedged','Chamfered'].includes(chrome.buttonStyle)?0:4);context.fill();context.stroke();
                context.fillStyle=rgbaColor(chrome.text);context.textBaseline='middle';context.textAlign='left';context.fillText(text,x+10,y+height/2,width-20);context.restore();
            }
        }
    }
    setLayoutPreview(value) {
        const next=validSnapPreview(value);
        if(JSON.stringify(next)===JSON.stringify(this.layoutPreview))return;
        this.layoutPreview=next;this.onDamage();
    }
    renderLayoutPreview(root) {this.#renderSnapPreview(root,this.layoutPreview);}
    renderPopupOverlays(root) {
        const visible = this.snapshots.filter(snapshot => snapshot.layerVisible !== false);
        const popups = visible.filter(snapshot => snapshot.popupCommands?.length > 0);
        for (const [key, native] of this.closingPopups) {
            if (!this.transitions.has(key)) { this.closingPopups.delete(key); continue; }
            if (visible.some(owner => String(owner.app) === String(native.app))) popups.push(native);
        }
        for (const native of popups) {
            this.invalidateBackdrop();
            const frame = native.frame;
            const popup = native.popup?.rect;
            const identity = popupIdentity(native);
            const key = identity ? `popup:${native.app}:${identity}` : '';
            const closing = this.closingPopups.has(key);
            const elapsed = this.#transitionProgress(native, key);
            if (closing && elapsed >= 1) { this.closingPopups.delete(key); continue; }
            const progress = closing ? 1 - elapsed : elapsed;
            const motion = menuMotion(native, progress, closing);
            const offsetY = native.role === "desktop" ? 0 : frame.titleHeight;
            const globalPopup = popup ? {
                x: frame.x + popup.x,
                y: frame.y + offsetY + popup.y,
                width: popup.width,
                height: popup.height,
            } : null;
            let matrixPopup = null;
            if (popup && progress < 1 && native.shell?.surface?.frame === 'matrix') {
                const transition=this.transitions.get(key);
                if (transition) {
                    matrixPopup=transition.matrixPopup;
                    if (!matrixPopup || matrixPopup.source.width!==popup.width+4 || matrixPopup.source.height!==popup.height+4) {
                        matrixPopup=transition.matrixPopup={source:this.frostedIceMaterial.canvasFactory(popup.width+4,popup.height+4),backdrop:this.frostedIceMaterial.canvasFactory(popup.width+4,popup.height+4)};
                        matrixPopup.backdrop.getContext('2d').drawImage(this.context.canvas,globalPopup.x-2,globalPopup.y-2,popup.width+4,popup.height+4,0,0,popup.width+4,popup.height+4);
                    }
                    this.context.drawImage(matrixPopup.backdrop,globalPopup.x-2,globalPopup.y-2);
                }
            }
            const wedgedPopup = popup && native.chrome?.windowShape === "Wedged";
            // Cascades occupy separate panes, not their rectangular union.
            for (const pane of popup ? popupSurfaceRects(native.popupCommands, popup) : []) {
                const popupHasOwnGlass = native.popupCommands.some(command =>
                    command.type === "glass" &&
                    command.control !== true &&
                    command.structural !== true &&
                    sameRect(command.rect, pane)
                );
                if (!native.shell?.frostedIce?.enabled ||
                    !native.shell?.frostedIce?.menus || popupHasOwnGlass) continue;
                const globalPane = {
                    x: frame.x + pane.x,
                    y: frame.y + offsetY + pane.y,
                    width: pane.width,
                    height: pane.height,
                };
                this.context.save();
                this.context.beginPath();
                if (wedgedPopup) {
                    appendWindowSilhouette(this.context, globalPane, {
                        ...native.chrome,
                        windowShape: "Wedged",
                    });
                } else {
                    this.context.rect(
                        globalPane.x,
                        globalPane.y,
                        globalPane.width,
                        globalPane.height,
                    );
                }
                this.context.clip();
                if (progress < 1) {
                    const originX = globalPopup.x + globalPopup.width / 2;
                    const originY = globalPopup.y;
                    this.context.translate(originX, originY);
                    this.context.translate(motion.x, motion.y);
                    this.context.scale(native.shell?.surface?.frame === 'matrix' ? 1 : motion.scaleX, motion.scaleY);
                    this.context.translate(-originX, -originY);
                    this.context.globalAlpha *= motion.alpha;
                }
                this.frostedIceMaterial.draw(
                    this.context,
                    this.context.canvas,
                    globalPane,
                    10,
                    { ...native.shell.frostedIce, rim: false, rimWidth: 0, sheet: true },
                    this.now(),
                );
                this.context.restore();
            }
            // A popup with its own Glass command is authoritative; painting
            // the shell material underneath it double-blurs the menu into an
            // opaque gray slab. Other popups still receive the shell coat.
            this.invalidateBackdrop();
            this.context.save();
            try {
                this.context.beginPath();
                this.context.rect(0, 0, root.width, root.height);
                this.context.clip();
                this.context.translate(frame.x, frame.y + offsetY);
                if (popup && progress < 1) {
                    const originX = popup.x + popup.width / 2;
                    const originY = popup.y;
                    this.context.beginPath();
                    if (wedgedPopup) {
                        appendWindowSilhouette(this.context, popup, {
                            ...native.chrome,
                            windowShape: "Wedged",
                        });
                    } else {
                        this.context.rect(popup.x, popup.y, popup.width, popup.height);
                    }
                    this.context.clip();
                    this.context.translate(originX, originY);
                    this.context.translate(motion.x, motion.y);
                    this.context.scale(native.shell?.surface?.frame === 'matrix' ? 1 : motion.scaleX, motion.scaleY);
                    this.context.translate(-originX, -originY);
                    this.context.globalAlpha *= motion.alpha;
                }
                if (popup && native.shell?.menuShadow === true && native.shell?.surface?.frame !== 'matrix') {
                    this.#renderPopupShadow(
                        native,
                        popup,
                        native.popupCommands,
                        root,
                        frame,
                        offsetY,
                    );
                }
                const popupCommands = wedgedPopup
                    ? native.popupCommands.map(command =>
                        ["fill", "border"].includes(command.type) &&
                        command.rect?.width === popup.width &&
                        command.rect?.height === popup.height
                            ? { ...command, shape: "wedged" }
                            : command
                    )
                    : native.popupCommands;
                this.#renderSurfaceCommands(native, popupCommands, null, popup);
                if (matrixPopup) {
                    const capture=matrixPopup.source.getContext('2d');
                    capture.clearRect(0,0,popup.width+4,popup.height+4);
                    capture.drawImage(this.context.canvas,globalPopup.x-2,globalPopup.y-2,popup.width+4,popup.height+4,0,0,popup.width+4,popup.height+4);
                    matrixPopup.model ||= createMatrixMorph(matrixPopup.source);
                    paintMatrixMorph(this.context,{x:popup.x-2,y:popup.y-2,width:popup.width+4,height:popup.height+4},progress,matrixPopup.model,{background:matrixPopup.backdrop});
                } else if (popup && motion.reveal !== undefined && motion.reveal < 1) {
                    const accent = native.chrome?.selection || native.chrome?.close || [204,21,21,255];
                    this.context.fillStyle = `rgba(${accent[0]},${accent[1]},${accent[2]},${(accent[3] ?? 255)/255})`;
                    if (native.shell?.surface?.frame === 'matrix') paintMatrixCurtain(this.context,popup,motion.reveal,native.shell.surface.edge,progress);
                    else if(native.shell?.surface?.variant==='cyberpunk')paintSignalCurtain(this.context,popup,motion.reveal,this.context.fillStyle,native.shell.surface);
                    else this.context.fillRect(popup.x, popup.y, popup.width * (1-motion.reveal), popup.height);
                }
            } catch (error) {
                this.onFault(Object.freeze({
                    app: String(native.app || ""),
                    entrypoint: String(native.entrypoint || "rice-desktop"),
                    error: error?.message || String(error),
                }));
            } finally {
                this.context.restore();
            }
        }
    }
    renderPromotedDesktopPopups(root) {
        const snapshots = this.snapshots;
        this.snapshots = snapshots.filter(snapshot => snapshot.role === "desktop");
        try {
            this.renderPopupOverlays(root);
        } finally {
            this.snapshots = snapshots;
        }
    }
    renderPromotedDesktopDialogs(root) {
        for (const native of this.snapshots.filter(snapshot =>
            snapshot.role === "desktop" &&
            snapshot.layerVisible !== false &&
            snapshot.dialog?.rect
        )) {
            const rect = native.dialog.rect;
            const commands = this.#desktopCommandsWithApplicationIcons(
                native.commands,
                native,
            );
            const dialogStart = commands.findIndex(command =>
                command.type === "fill" &&
                command.rect?.x === rect.x &&
                command.rect?.y === rect.y &&
                command.rect?.width === rect.width &&
                command.rect?.height === rect.height
            );
            if (dialogStart < 0) continue;
            this.invalidateBackdrop();
            this.context.save();
            try {
                this.context.beginPath();
                this.context.rect(0, 0, root.width, root.height);
                this.context.clip();
                this.context.translate(native.frame.x, native.frame.y);
                this.#renderDialogShadow(native, native.chrome || derivedChrome(native));
                this.#renderSurfaceCommands(native, commands.slice(dialogStart), native.chrome || derivedChrome(native), rect, true);
            } catch (error) {
                this.onFault(Object.freeze({
                    app: String(native.app || ""),
                    entrypoint: String(native.entrypoint || "rice-desktop"),
                    error: error?.message || String(error),
                }));
            } finally {
                this.context.restore();
            }
        }
    }
    #renderPopupShadow(native, popup, popupCommands, root, frame, offsetY) {
        const context = this.context;
        const panes = popupSurfaceRects(popupCommands, popup);
        context.save();
        try {
            for (const pane of panes) {
                // Intersecting one outside-only clip per pane produces the
                // complement of their union even where cascading panes
                // overlap. A single even-odd path would expose that overlap.
                context.beginPath();
                context.rect(
                    -frame.x,
                    -frame.y - offsetY,
                    root.width,
                    root.height,
                );
                if (native.chrome?.windowShape === "Wedged") {
                    appendWindowSilhouette(context, pane, {
                        ...native.chrome,
                        windowShape: "Wedged",
                    });
                } else if (typeof context.roundRect === "function") {
                    context.roundRect(
                        pane.x,
                        pane.y,
                        pane.width,
                        pane.height,
                        7,
                    );
                } else {
                    context.rect(pane.x, pane.y, pane.width, pane.height);
                }
                context.clip("evenodd");
            }
            context.shadowColor = "rgba(0, 0, 0, 0.42)";
            context.shadowBlur = 18;
            context.shadowOffsetX = 0;
            context.shadowOffsetY = 6;
            context.fillStyle = "#000000";
            context.beginPath();
            for (const pane of panes) {
                if (native.chrome?.windowShape === "Wedged") {
                    appendWindowSilhouette(context, pane, {
                        ...native.chrome,
                        windowShape: "Wedged",
                    });
                } else {
                    roundedPath(context, pane, 7);
                }
            }
            context.fill();
        } finally {
            context.restore();
        }
    }
    #renderDisplayNumbers() {
        if (!this.identifyingDisplays) return;
        const context = this.context;
        context.save();
        try {
            for (const [index, output] of (this.outputRects || []).entries()) {
                const width = Math.min(112, output.width), height = Math.min(96, output.height);
                const x = output.x + Math.round((output.width - width) / 2);
                const y = output.y + Math.min(48, Math.max(0, output.height - height));
                roundedPath(context, {x, y, width, height}, 12);
                context.fillStyle = "rgba(15, 23, 42, 0.92)";
                context.fill();
                context.fillStyle = "#ffffff";
                context.font = "bold 56px sans-serif";
                context.textAlign = "center";
                context.textBaseline = "middle";
                context.fillText(String(output.displayNumber || index + 1), x + width / 2, y + height / 2);
            }
        } finally { context.restore(); }
    }
    #renderSnapPreview(root,preview=this.snapPreview) {
        if (!preview) return;
        const context = this.context;
        const inset = 4;
        const rect = {
            x: Math.max(0, preview.x + inset),
            y: Math.max(0, preview.y + inset),
            width: Math.max(1, Math.min(
                root.width - preview.x - inset * 2,
                preview.width - inset * 2,
            )),
            height: Math.max(1, Math.min(
                root.height - preview.y - inset * 2,
                preview.height - inset * 2,
            )),
        };
        context.save();
        try {
            roundedPath(context, rect, 8);
            context.fillStyle = "rgba(59, 130, 246, 0.24)";
            context.fill();
            context.strokeStyle = "rgba(147, 197, 253, 0.92)";
            context.lineWidth = 2;
            context.stroke();
        } finally {
            context.restore();
        }
    }
    #renderDragVisual(root) {
        const visual = this.dragVisual;
        if (!visual) return;
        const context = this.context;
        paintPaddyDropOverlay(context, this.snapshots, visual);
        const width = 184;
        const height = 50;
        const x = Math.max(4, Math.min(root.width - width - 4, visual.x + 6));
        const y = Math.max(4, Math.min(root.height - height - 4, visual.y + 6));
        const action = ({ copy: "Copy", move: "Move", link: "Link", ask: "Drop" })[
            visual.action
        ] || "Drop";
        context.save();
        try {
            context.globalAlpha = 0.76;
            roundedPath(context, { x, y, width, height }, 8);
            context.fillStyle = visual.accepted ? "#263c2e" : "#463232";
            context.fill();
            context.strokeStyle = visual.accepted ? "#73c78d" : "#d28a8a";
            context.lineWidth = 1;
            context.stroke();
            const sourceSize = 38;
            const sourceX = Math.max(0, visual.sourceX - sourceSize / 2);
            const sourceY = Math.max(0, visual.sourceY - sourceSize / 2);
            if (context.canvas && typeof context.drawImage === "function") {
                context.save();
                roundedPath(context, {
                    x: x + 6,
                    y: y + 6,
                    width: sourceSize,
                    height: sourceSize,
                }, 5);
                context.clip();
                context.globalAlpha = 0.68;
                context.drawImage(
                    context.canvas,
                    sourceX,
                    sourceY,
                    sourceSize,
                    sourceSize,
                    x + 6,
                    y + 6,
                    sourceSize,
                    sourceSize,
                );
                context.restore();
            } else {
                context.fillStyle = "#f5f5f5";
                context.fillRect(x + 14, y + 12, 18, 24);
            }
            if (visual.action === "copy") {
                roundedPath(context, {
                    x: x + 34,
                    y: y + 34,
                    width: 16,
                    height: 16,
                }, 8);
                context.fillStyle = "#22c55e";
                context.fill();
                context.fillStyle = "#ffffff";
                context.font = "700 14px sans-serif";
                context.textAlign = "center";
                context.textBaseline = "middle";
                context.fillText("+", x + 42, y + 42);
                context.textAlign = "start";
            }
            context.fillStyle = "#ffffff";
            context.font = "600 12px sans-serif";
            context.textBaseline = "middle";
            context.fillText(visual.label, x + 51, y + 18, 126);
            context.fillStyle = visual.accepted ? "#a9e7bb" : "#efb0b0";
            context.font = "11px sans-serif";
            context.fillText(
                visual.accepted ? `${action} here` : "Not a drop target",
                x + 51,
                y + 35,
                126,
            );
        } finally {
            context.restore();
        }
    }
    #validate(snapshot) {
        if (
            !snapshot ||
            !Array.isArray(snapshot.commands) ||
            snapshot.commands.length > MAX_NATIVE_COMMANDS ||
            !snapshot.frame ||
            snapshot.width !== snapshot.frame.width ||
            snapshot.height !== snapshot.frame.contentHeight
        ) {
            throw new TypeError("invalid native UI snapshot");
        }
        for(const value of [snapshot.frame.x,snapshot.frame.y]) {
            if(!Number.isInteger(value)||value < -0x7fffffff || value > 0x7fffffff)throw new RangeError("native UI position is out of bounds");
        }
        for (const value of [
            snapshot.frame.width,
            snapshot.frame.height,
            snapshot.frame.contentHeight,
            snapshot.frame.titleHeight,
        ]) {
            if (!Number.isInteger(value) || value < 0 || value > 16_384) {
                throw new RangeError("native UI frame is out of bounds");
            }
        }
    }
    #captureStructuralBackdrop(native, chrome = null) {
        if (
            chrome?.glassTitlebar !== true &&
            !native.commands?.some(command =>
                command.type === "glass" && command.structural === true
            )
        ) return null;
        const source = this.context?.canvas;
        if (!source || source.width < 1 || source.height < 1) return null;
        if (!this.structuralBackdrop) {
            this.structuralBackdrop = this.frostedIceMaterial.canvasFactory(
                source.width,
                source.height,
            );
        } else if (
            this.structuralBackdrop.width !== source.width ||
            this.structuralBackdrop.height !== source.height
        ) {
            resizeCanvasBackingStore(this.structuralBackdrop, source.width, source.height);
        }
        const target = this.structuralBackdrop?.getContext?.("2d", {
            alpha: true,
        });
        if (!target?.drawImage) return null;
        target.setTransform?.(1, 0, 0, 1, 0, 0);
        target.clearRect?.(0, 0, source.width, source.height);
        target.drawImage(source, 0, 0);
        return this.structuralBackdrop;
    }
    #renderWindow(native, root) {
        const frame = native.frame;
        const visible=this.layoutPaintFrame||frame;
        if (!root.preview && (
            visible.x >= root.width + 32 || visible.x + visible.width < -32 ||
            visible.y >= root.height + 32 || visible.y + visible.height < -32 ||
            frame.width === 0 || frame.height === 0
        )) {
            return;
        }
        const context = this.context;
        this.commandRenderer.setStructuralGlassSource(null);
        if (native.role === "desktop") {
            const wallpaperCommand = native.commands.find(command =>
                command.type === "media" &&
                command.rect?.x === 0 &&
                command.rect?.y === 0 &&
                command.rect?.width === frame.width &&
                command.rect?.height === frame.contentHeight
            );
            const background = {
                ...native.desktopBackground,
                type:native.desktopBackground?.scene? (native.desktopBackground.scene.type==='video'?'Video':'Image'):native.desktopBackground?.type,
                surface:native.desktopBackground?.scene ? this.mediaSurface(native.desktopBackground.scene.request) : native.desktopBackground?.type === "Video"
                    ? this.mediaSurface(native.desktopBackground.videoSession)
                    : wallpaperCommand?.surface ||
                        (wallpaperCommand
                            ? this.mediaSurface(wallpaperCommand.request)
                            : null),
            };
            const wallpaperRects = this.outputRects?.length ? this.outputRects : [this.snapshot?.shell?.dockStyle === "Compact dock" ? {
                x: 0, y: 0, width: root.width, height: root.height,
            } : {
                x: frame.x,
                y: Math.min(frame.y, root.wallpaperTop ?? frame.y),
                width: frame.width,
                height: frame.contentHeight + Math.max(0, frame.y - (root.wallpaperTop ?? frame.y)),
            }];
            // Extended outputs each receive a complete wallpaper crop. Desktop
            // icons and panels remain on their designated primary output.
            for (const rect of wallpaperRects) this.wallpaper.render(background, rect);
            context.save();
            context.beginPath();
            context.rect(
                frame.x,
                frame.y,
                frame.width,
                frame.contentHeight,
            );
            context.clip();
            context.translate(frame.x, frame.y);
            this.commandRenderer.render(
                this.#desktopCommandsWithApplicationIcons(
                    native.commands.filter(command => command !== wallpaperCommand),
                    native,
                ).map(command=>command.type==='inline-image'||command.type==='web-scene'
                    ? {...command,cacheNamespace:String(native.app)}:command),
                null,
            );
            context.restore();
            return;
        }
        const chrome = native.chrome || derivedChrome(native);
        if(root.preview&&root.paper&&!native.dialogOnly){
            context.save();
            try{
                context.translate(frame.x,frame.y);
                context.beginPath();context.roundRect(0,0,frame.width,frame.height,4);context.clip();
                context.fillStyle=root.paper.fill;context.fillRect(0,0,frame.width,frame.height);
                context.strokeStyle=root.paper.shade;context.lineWidth=1;context.strokeRect(.5,.5,frame.width-1,frame.height-1);
                context.fillStyle=root.paper.ink;context.font='13px Georgia,serif';context.textAlign='center';context.textBaseline='middle';context.fillText(String(native.title||native.entrypoint||'Application'),frame.width/2,frame.titleHeight/2,Math.max(1,frame.width-32));
                context.beginPath();context.rect(0,frame.titleHeight,frame.width,frame.contentHeight);context.clip();context.translate(0,frame.titleHeight);
                this.#renderContentCommands(native,chrome);
            }finally{context.restore();}
            return;
        }
        if (native.dialogOnly) {
            // Host-owned dialogs use an isolated actor for their controls, but
            // only the dialog pane is visible, never a second application frame.
            if (!native.dialog?.rect) return;
            context.save();
            try {
                context.translate(frame.x, frame.y + frame.titleHeight);
                this.#renderContentCommands(native, chrome);
            } finally {
                context.restore();
            }
            return;
        }
        if (native.fullscreen === true && frame.titleHeight === 0) {
            // Borderless native fullscreen owns every output pixel. Window
            // silhouettes, theme rims and shadows must not clip its content.
            this.#captureTerminalBackdrops(native, frame, chrome);
            context.save();
            try {
                context.beginPath();
                context.rect(frame.x, frame.y, frame.width, frame.height);
                context.clip();
                context.translate(frame.x, frame.y);
                this.commandRenderer.render((this.directTerminalGrid
                    ? native.commands.filter(command => command.type !== 'terminal-grid' && command.type !== 'terminal-row')
                    : native.commands).map(command=>command.type==='inline-image'||command.type==='web-scene'
                        ? {...command,cacheNamespace:String(native.app)}:command), chrome);
            } finally {
                context.restore();
            }
            return;
        }
        const structuralSource = this.#captureStructuralBackdrop(native, chrome);
        this.commandRenderer.setStructuralGlassSource(structuralSource);
        const titlebar = nativeTitlebarLayout(
            frame,
            native.title,
            native.titleTools,
            chrome.windowDecorations,
        );
        const glassTitlebar = chrome.glassTitlebar === true && structuralSource;
        if (chrome.redmond98 && !this.classicFontsRequested) {
            this.classicFontsRequested = true;
            ensureClassicUiFonts()?.then(() => this.onDamage()).catch(() => {});
        }
        const titleChrome = glassTitlebar
            ? {
                ...chrome,
                text: adaptiveStructuralInk(
                    structuralSource,
                    {
                        x: frame.x,
                        y: frame.y,
                        width: Math.min(frame.width, 240),
                        height: frame.titleHeight,
                    },
                    chrome.glassTint,
                    chrome.glassText || [245, 247, 246, 255],
                ),
            }
            : { ...chrome };
        if (chrome.redmond98 && !native.active) titleChrome.text = [192, 192, 192, 255];
        const titleInk = native.shell?.surface?.titleInk;
        if (native.shell?.surface?.frame === 'bracket') titleChrome.text = chrome.text;
        if (native.shell?.surface?.frame !== 'bracket' && titleInk && native.active !== false && !glassTitlebar) {
            const packed = Number.parseInt(titleInk.slice(1),16);
            titleChrome.text = readableInk([(packed>>16)&255,(packed>>8)&255,packed&255,255],chrome.active);
        }
        const titleControlChrome = glassTitlebar
            ? {
                ...titleChrome,
                // The controls have opaque button fills, unlike the glass title.
                controlText: chrome.controlText || contrastingControlInk(chrome.control),
                text: adaptiveStructuralInk(
                    structuralSource,
                    {
                        x: frame.x + Math.max(0, frame.width - 160),
                        y: frame.y,
                        width: Math.min(frame.width, 160),
                        height: frame.titleHeight,
                    },
                    chrome.glassTint,
                    titleChrome.text,
                ),
            }
            : titleChrome;
        this.#renderFrostedIceBackdrop(frame, chrome, root, titlebar);
        this.#renderActiveWindowShadow(native, frame, chrome, root, titlebar);
        this.#captureTerminalBackdrops(native, frame, chrome);
        context.save();
        context.beginPath();
        context.rect(0, 0, root.width, root.height);
        context.clip();
        context.translate(frame.x, frame.y);
        context.beginPath();
        appendWindowSilhouette(
            context,
            { ...frame, x: 0, y: 0 },
            chrome,
            titlebar,
        );
        context.clip();
        if (native.shell?.surface?.frame === 'bracket') {
            const inset=native.shell.surface.inset ?? 5;
            if(native.shell.surface.variant==='cyberpunk')cyberpunkPath(context,{x:inset,y:inset,width:frame.width-inset*2,height:frame.height-inset*2},native.shell.surface.cornerLength);
            else {context.beginPath();context.rect(inset,inset,frame.width-inset*2,frame.height-inset*2);}
            context.clip();
        }
        if (chrome.material !== "frosted-ice") {
            context.save();
            context.globalAlpha *= this.windowBodyAlpha;
            context.fillStyle = rgbaColor(chrome.body);
            context.beginPath();
            appendWindowSilhouette(
                context,
                { ...frame, x: 0, y: 0 },
                chrome,
                titlebar,
            );
            context.fill();
            context.restore();
        }
        if (glassTitlebar) {
            paintStructuralGlass(
                context,
                structuralSource,
                { x: 0, y: 0, width: frame.width, height: frame.titleHeight },
                {
                    x: frame.x,
                    y: frame.y,
                    width: frame.width,
                    height: frame.titleHeight,
                },
                (chrome.glassTint || [23, 33, 31, 255]).slice(0, 3),
            );
        } else {
            if (chrome.redmond98) {
                context.fillStyle = "#c0c0c0";
                context.fillRect(0, 0, frame.width, frame.titleHeight);
                const gradient = typeof context.createLinearGradient === "function"
                    ? context.createLinearGradient(3, 0, Math.max(4, frame.width - 3), 0)
                    : null;
                if (gradient) {
                    gradient.addColorStop(0, rgbaColor(native.active ? chrome.active : chrome.inactive));
                    gradient.addColorStop(1, rgbaColor(native.active
                        ? chrome.activeTitleEnd || chrome.active
                        : chrome.inactiveTitleEnd || chrome.inactive));
                    context.fillStyle = gradient;
                } else {
                    context.fillStyle = rgbaColor(
                        native.shell?.surface?.frame === 'bracket' ? chrome.body : native.active ? chrome.active : (chrome.inactive || chrome.active),
                    );
                }
                context.fillRect(3, 3, Math.max(0, frame.width - 6), Math.max(1, frame.titleHeight - 5));
            } else {
                context.fillStyle = rgbaColor(
                    native.shell?.surface?.frame === 'bracket' ? chrome.body : native.active ? chrome.active : (chrome.inactive || chrome.active),
                );
                context.save();
                if (chrome.frostedIce?.model === "hyprglass") context.globalAlpha *= .10;
                if (titlebar.compact) {
                    titlebarPath(context, frame, titlebar);
                    context.fill();
                } else {
                    context.fillRect(0, 0, frame.width, frame.titleHeight);
                }
                context.restore();
            }
        }
        if(chrome.chromeProfile==='redmond-xp'){
            context.save();
            const base=native.active?chrome.active:chrome.inactive;
            const gradient=context.createLinearGradient(0,0,0,frame.titleHeight);
            const shade=(amount)=>rgbaColor(base.map((v,i)=>i===3?v:Math.round(v+(amount>0?255-v:v)*amount)));
            gradient.addColorStop(0,shade(.45));gradient.addColorStop(.16,shade(.12));gradient.addColorStop(.52,shade(-.18));gradient.addColorStop(.88,shade(.08));gradient.addColorStop(1,shade(-.32));
            context.fillStyle=gradient;context.fillRect(0,0,frame.width,frame.titleHeight);
            context.strokeStyle='rgba(255,255,255,.6)';context.lineWidth=1;context.beginPath();context.moveTo(5,1);context.lineTo(frame.width-5,1);context.stroke();
            context.restore();
        }
        this.paintCustomFrame?.(context,native,root);
        if (chrome.chromeProfile === "command-deck") {
            this.#renderCommandDeckTitleRail(
                chrome,
                frame,
                titlebar,
                native.title,
            );
        }
        if (chrome.chromeProfile === "orchard") {
            context.save();
            const gradient = typeof context.createLinearGradient === "function"
                ? context.createLinearGradient(0, 0, 0, frame.titleHeight)
                : null;
            if (gradient) {
                gradient.addColorStop(0, "rgba(255, 255, 255, .64)");
                gradient.addColorStop(.55, "rgba(255, 255, 255, .32)");
                gradient.addColorStop(1, "rgba(225, 238, 250, .12)");
                context.fillStyle = gradient;
            } else {
                context.fillStyle = "rgba(255, 255, 255, .26)";
            }
            context.fillRect(0, 0, frame.width, frame.titleHeight);
            context.fillStyle = "rgba(255, 255, 255, .72)";
            context.fillRect(1, 1, Math.max(0, frame.width - 2), 1);
            context.fillStyle = "rgba(80, 105, 130, .12)";
            context.fillRect(0, frame.titleHeight - 1, frame.width, 1);
            const sidebar = native.commands?.find(command =>
                command.type === "glass" && command.structural === true &&
                command.rect?.x === 0 && command.rect?.y === 0
            );
            if (sidebar && structuralSource) {
                paintStructuralGlass(
                    context,
                    structuralSource,
                    {
                        x: 0,
                        y: 0,
                        width: sidebar.rect.width,
                        height: frame.titleHeight,
                    },
                    {
                        x: frame.x,
                        y: frame.y,
                        width: sidebar.rect.width,
                        height: frame.titleHeight,
                    },
                    Array.isArray(sidebar.tint)
                        ? sidebar.tint.slice(0, 3)
                        : [245, 245, 247],
                );
            }
            context.restore();
        } else if (chrome.chromeProfile === "yellow-jacket") {
            context.fillStyle = "rgba(255, 255, 214, .75)";
            context.fillRect(1, 1, Math.max(0, titlebar.width - 2), 1);
            context.fillStyle = rgbaColor(chrome.border);
            context.fillRect(0, frame.titleHeight - 1, titlebar.width, 1);
        }
        this.#renderDecorationAtlas(chrome, frame, titlebar, "title");
        this.windowSkinRenderer ||= new RiceThemeSkinRenderer(()=>this.onDamage());
        this.windowSkinRenderer.paint(context,{x:0,y:0,width:frame.width,height:frame.titleHeight},chrome,'window',native.active===false?'normal':'selected');
        context.fillStyle = rgbaColor(
            chrome.chromeProfile === "command-deck"
                ? chrome.pressed
                : titleChrome.text,
        );
        context.font = chrome.chromeProfile === "neon-grid"
            ? '600 12px "RiceWM Mono", "RiceWM CJK", monospace'
            : chrome.chromeProfile === "yellow-jacket"
                ? '700 15px "RiceWM Sans", "RiceWM CJK", sans-serif'
                : chrome.chromeProfile === "command-deck"
                    ? '700 14px "RiceWM Sans", "RiceWM CJK", sans-serif'
                : chrome.redmond98
                    ? `700 11px ${classicUiFont}`
                    : '600 13px "RiceWM Sans", "RiceWM CJK", sans-serif';
        context.textAlign = "left";
        context.textBaseline = "middle";
        const titleInset = native.shell?.surface?.frame === 'bracket' ? native.shell.surface.inset ?? 5 : 0;
        if(chrome.typography&&Object.keys(chrome.typography).length)context.font=desktopThemeFont(chrome.typography,'title');
        const titleCenterY = (titleInset + frame.titleHeight) / 2;
        const titleIconSize = chrome.redmond98 ? 16 : Math.max(12, Math.min(18, frame.titleHeight - titleInset - 8));
        const titleIconX = nativeTitleIconX(chrome.windowDecorations,frame,native.title);
        const titleIconY = Math.round(titleCenterY - titleIconSize / 2);
        // The application glyph is a functional window-menu target as well
        // as decoration. Yellow Jacket previously reserved this space but
        // deliberately skipped the draw, leaving a conspicuous blank square.
        this.iconRenderer.renderNamed({
            type: "icon",
            applicationId:native.riceAppId||native.applicationId,
            icon: native.iconName || nativeEntrypointIconName(
                native.applicationId?.startsWith("native:")
                    ? native.applicationId.slice(7)
                    : native.entrypoint,
            ),
            rect: {
                x: titleIconX,
                y: titleIconY,
                width: titleIconSize,
                height: titleIconSize,
            },
            color: titleChrome.text,
            colorArtwork: native.entrypoint === "rice-store",
        });
        if (native.shell?.surface?.frame === 'bracket') {
            // The icon remains at the window-menu hit target. All ornament and
            // text live inside the title's existing allocation, away from controls.
            const titleX = Math.max(titlebar.titleX, titleIconX + 26 + 6);
            const right = titlebar.titleX + titlebar.titleWidth;
            const textX = titleX + (native.shell.surface.variant==='cyberpunk'?52:10);
            const available = Math.max(0, right - textX);
            const translations={'rice-files':'文件管理','rice-settings':'系统设置','rice-term':'终端','rice-paper':'文档','rice-calc':'计算器','rice-bowl':'浏览器'};
            const cyber=native.shell.surface.variant==='cyberpunk';
            const caption=cyber||native.shell.surface.captions===false?'':'//  '+(translations[native.entrypoint] || '綾波');
            context.save();context.beginPath();context.rect(titleX,titleInset,Math.max(0,right-titleX),frame.titleHeight-titleInset);context.clip();
            context.fillStyle=rgbaColor(chrome.selection || chrome.active);
            if(cyber){
                cyberpunkPath(context,{x:titleX,y:titleInset+2,width:Math.min(44,Math.max(0,right-titleX)),height:frame.titleHeight-titleInset-4},6);context.fill();
                context.fillStyle='#10140a';context.font='bold 10px monospace';context.fillText('AB',titleX+10,titleCenterY);
            }else context.fillRect(titleX,Math.round(titleCenterY-6),3,12);
            context.font='10px "RiceWM Mono", "RiceWM CJK", monospace, "RiceWM Signal"';
            const captionWidth=context.measureText(caption).width;
            const showCaption=available>captionWidth+85;
            const labelWidth=Math.max(0,available-(showCaption?captionWidth+16:0));
            context.font='11px "RiceWM Mono", "RiceWM CJK", monospace';
            let label=String(native.title || 'RiceWM').toUpperCase();
            if(context.measureText(label).width>labelWidth){while(label.length && context.measureText(label+'…').width>labelWidth)label=label.slice(0,-1);label+='…';}
            context.fillStyle=rgbaColor(titleChrome.text);context.fillText(label,textX,titleCenterY);
            if(showCaption){
                const suffixX=textX+context.measureText(label).width+16;
                context.fillStyle=rgbaColor(chrome.selection || chrome.active);
                context.font='10px "RiceWM Mono", "RiceWM CJK", monospace, "RiceWM Signal"';context.fillText(caption,suffixX,titleCenterY);
            }
            context.restore();
        } else {
            const matrix = native.shell?.surface?.frame === 'matrix';
            if (matrix) {
                context.save();context.beginPath();context.rect(titlebar.titleX,0,titlebar.titleWidth,frame.titleHeight);context.clip();
                context.font='11px "RiceWM Mono", "RiceWM CJK", monospace';context.fillStyle=rgbaColor(titleChrome.text);
                const label=String(native.title || 'RiceWM').toUpperCase();
                const tag=`[ ${String(native.app).padStart(3,'0')} / ${native.active !== false ? 'ACTIVE' : 'IDLE'} ]`;
                const reserve=titlebar.titleWidth>260?context.measureText(tag).width+24:0;
                context.fillText('⌁ '+label,titlebar.titleX,titleCenterY,Math.max(1,titlebar.titleWidth-reserve));
                if(reserve){context.fillStyle=native.shell.surface.edge;context.textAlign='right';context.fillText(tag,titlebar.titleX+titlebar.titleWidth-8,titleCenterY);}
                context.restore();
            } else context.fillText(String(native.title || 'RiceWM'),titlebar.titleX,titleCenterY,Math.max(1,titlebar.titleWidth));
        }
        context.save();
        context.beginPath();
        context.rect(
            0,
            frame.titleHeight,
            frame.width,
            frame.contentHeight,
        );
        context.clip();
        context.translate(0, frame.titleHeight);
        // Content glass samples the completed window chrome, not the earlier
        // desktop/window backdrop epoch.
        this.invalidateBackdrop();
        context.save();
        context.globalAlpha *= this.windowContentAlpha;
        this.#renderContentCommands(native, chrome);
        context.restore();
        context.restore();
        this.#renderDecorationAtlas(chrome, frame, titlebar, "frame");
        // Corner artwork belongs behind interactive chrome. In particular,
        // Neon Grid's upper-right corner shares space with a right-side
        // control box; painting controls last keeps both elements legible.
        // Title tools are a new glass layer and sample the completed chrome.
        this.invalidateBackdrop();
        context.save();
        context.translate(0, titleInset);
        const titleControlFrame = { ...frame, titleHeight: frame.titleHeight - titleInset };
        this.#renderTitleTools(
            titleControlFrame,
            titleControlChrome,
            native.titleTools,
            native.hoveredControl,
            native.title,
            chrome.windowDecorations,
        );
        this.#renderTitleControls(
            titleControlFrame,
            titleControlChrome,
            native.maximized,
            native.hoveredControl,
            native.pressedControl,
            native.title,
            native.titleTools,
            chrome.windowDecorations,
        );
        context.restore();
        if (chrome.redmond98) {
            paintRedmondBevel(context, { x: 0, y: 0, width: frame.width, height: frame.height });
        }
        context.restore();
        if (chrome.material === "frosted-ice") {
            if (chrome.frostedIce?.model === 'hyprglass' && native.active) {
                // A steady palette-colored focus outline survives busy glass
                // backgrounds without another blur, shadow, or animation pass.
                context.save();
                context.strokeStyle = rgbaColor(chrome.close || chrome.text);
                context.lineWidth = 2;
                context.beginPath();
                appendWindowSilhouette(context, {...frame,x:frame.x+1,y:frame.y+1,width:frame.width-2,height:frame.height-2}, chrome, titlebar);
                context.stroke();
                context.restore();
            }
            const windowBorder = Math.max(0, Math.min(
                100,
                Number(chrome.frostedIce?.windowBorder) || 0,
            ));
            if (windowBorder > 0) {
                const isolated = compactTitlebarIsolated(chrome, titlebar);
                this.frostedIceMaterial.drawRim(
                    context,
                    {
                        x: frame.x,
                        y: frame.y,
                        width: frame.width,
                        height: frame.height,
                    },
                    chrome.radius,
                    { ...chrome.frostedIce, rimWidth: windowBorder },
                    isolated || chrome.windowShape === "Wedged"
                        ? target => appendWindowSilhouette(
                            target,
                            frame,
                            chrome,
                            titlebar,
                        )
                        : null,
                );
            }
        } else if (chrome.frameBorderWidth !== 0 && !chrome.redmond98 && native.shell?.surface?.frame !== 'bracket') {
            context.strokeStyle = rgbaColor(chrome.border);
            context.lineWidth = chrome.frameBorderWidth ?? 1;
            context.beginPath();
            appendWindowSilhouette(context, frame, chrome, titlebar);
            context.stroke();
            if (chrome.chromeProfile === "yellow-jacket") {
                const bodyTop = frame.y + frame.titleHeight;
                const bodyBottom = frame.y + frame.height - 1;
                const bodyRight = frame.x + frame.width - 1;
                for (const [inset, light, shade] of [
                    [2, "rgba(255,255,255,.95)", "rgba(75,75,75,.95)"],
                    [4, "rgba(238,238,238,.95)", "rgba(130,130,130,.95)"],
                ]) {
                    context.lineWidth = 1;
                    context.strokeStyle = light;
                    context.beginPath();
                    context.moveTo(frame.x + inset, bodyBottom - inset);
                    context.lineTo(frame.x + inset, bodyTop + inset);
                    context.lineTo(bodyRight - inset, bodyTop + inset);
                    context.stroke();
                    context.strokeStyle = shade;
                    context.beginPath();
                    context.moveTo(bodyRight - inset, bodyTop + inset);
                    context.lineTo(bodyRight - inset, bodyBottom - inset);
                    context.lineTo(frame.x + inset, bodyBottom - inset);
                    context.stroke();
                }
                if (!native.maximized) {
                    this.#renderYellowJacketResizeGrip(frame);
                }
            }
        }
        this.renderSurfaceFrame(native, frame);
        this.commandRenderer.setStructuralGlassSource(null);
    }
    #renderSurfaceCommands(native, commands, chrome, rect, header = false) {
        if(native.shell?.surface?.frame !== 'bracket' || !rect) {this.commandRenderer.render(commands, chrome);return;}
        const inset=native.shell.surface.inset ?? 5;
        if (header && commands.length) {
            const firstText = commands.find(command=>command.type==='text' && command.rect?.y>=rect.y && command.rect.y<rect.y+80);
            if(firstText) {
                const bottom=Math.min(rect.y+80,firstText.rect.y+firstText.rect.height+6);
                                const textColor=readableInk(chrome.text,chrome.body);
                const titleFill={type:'fill',rect:{x:rect.x+inset,y:rect.y+inset,width:rect.width-inset*2,height:bottom-rect.y-inset},color:chrome.body};
                commands=[commands[0],titleFill,...commands.slice(1).map(command=>['text','icon'].includes(command.type) && command.rect.y<bottom ? {...command,color:textColor}:command)];
            }
        }
        this.context.save();this.context.beginPath();this.context.rect(rect.x+inset,rect.y+inset,Math.max(0,rect.width-inset*2),Math.max(0,rect.height-inset*2));this.context.clip();
        this.commandRenderer.render(commands, chrome);this.context.restore();this.renderSurfaceFrame(native,rect);
    }
    renderSurfaceFrame(native, rect) {
        const surface = native.shell?.surface;
        if(surface?.variant === 'cyberpunk' && rect) {paintCyberpunkFrame(this.context,rect,surface,native.active !== false);return;}
        if (surface?.frame === 'matrix' && rect) { paintMatrixFrame(this.context, rect, surface, native.active !== false); return; }
        if (surface?.frame !== 'bracket' || !rect) return;
        const context = this.context, inset = surface.inset ?? 3, length = surface.cornerLength ?? 8;
        context.save(); context.strokeStyle = surface.edge; context.lineWidth = 1;
        context.globalAlpha *= .45;
        context.strokeRect(rect.x + inset + .5, rect.y + inset + .5, Math.max(0,rect.width-inset*2-1), Math.max(0,rect.height-inset*2-1));
        context.globalAlpha /= .45; context.lineWidth = 1.5;
        for(const [x,y,sx,sy] of [[rect.x+1,rect.y+1,1,1],[rect.x+rect.width-1,rect.y+1,-1,1],[rect.x+1,rect.y+rect.height-1,1,-1],[rect.x+rect.width-1,rect.y+rect.height-1,-1,-1]]) {
            context.beginPath();context.moveTo(x,y+sy*length);context.lineTo(x,y);context.lineTo(x+sx*length,y);context.stroke();
        }
        context.restore();
    }
    #renderYellowJacketResizeGrip(frame) {
        const context = this.context;
        const right = frame.x + frame.width - 7;
        const bottom = frame.y + frame.height - 7;
        context.save();
        try {
            context.lineWidth = 1;
            for (let offset = 0; offset < 4; offset += 1) {
                const inset = (offset + 1) * 3;
                context.strokeStyle = offset % 2 === 0
                    ? "rgba(255,255,255,.95)"
                    : "rgba(92,92,92,.92)";
                context.beginPath();
                context.moveTo(right - inset, bottom);
                context.lineTo(right, bottom - inset);
                context.stroke();
            }
        } finally {
            context.restore();
        }
    }
    #desktopCommandsWithApplicationIcons(commands, native = null) {
        commands = this.#commandsWithFileAssociationIcons(commands);
        const replacements = new Map();
        for (let textIndex = 0; textIndex < commands.length; textIndex += 1) {
            const text = commands[textIndex];
            if (text.type !== "text") continue;
            const applicationIcon = this.desktopApplicationIcons.get(
                String(text.text || ""),
            );
            let best = null;
            let bestDistance = Number.POSITIVE_INFINITY;
            for (let index = textIndex - 1; index >= 0; index -= 1) {
                const icon = commands[index];
                if (icon.type !== "icon") continue;
                // Desktop labels occupy the whole icon-grid cell, while the
                // 48px desktop glyph can overlap the label rectangle by 24px
                // (text geometry still uses icon_medium).  Pair by the cell's
                // horizontal centre and tolerate that intentional overlap.
                const iconCenterX = icon.rect.x + icon.rect.width / 2;
                const horizontal = iconCenterX >= text.rect.x - 2 &&
                    iconCenterX <= text.rect.x + text.rect.width + 2;
                const gap = text.rect.y - (icon.rect.y + icon.rect.height);
                if (
                    !horizontal ||
                    gap < -icon.rect.height ||
                    gap > 16
                ) continue;
                const distance = Math.abs(gap) + Math.abs(
                    iconCenterX -
                    (text.rect.x + text.rect.width / 2),
                );
                if (distance < bestDistance) {
                    best = icon;
                    bestDistance = distance;
                }
            }
            const iconName = applicationIcon || best?.namedIcon ||
                riceNativeIconName(best?.icon);
            if (best && iconName) replacements.set(best, iconName);
        }
        if (replacements.size === 0) return commands;
        return commands.map(command => {
            const namedIcon = replacements.get(command);
            if (!namedIcon) return command;
            const decorated = { ...command, namedIcon };
            const key = this.#desktopLaunchKey(decorated);
            decorated.hovered = this.desktopHoveredIcon === key;
            if (native?.shell?.motionEnabled !== true) return decorated;
            const animation = this.desktopLaunchAnimations.get(key);
            if (!animation) return decorated;
            if (animation.external) {
                return {
                    ...decorated,
                    color: [
                        ...(Array.isArray(decorated.color)
                            ? decorated.color.slice(0, 3)
                            : [0, 0, 0]),
                        0,
                    ],
                };
            }
            const elapsed = this.now() - animation.startedAt;
            if (elapsed >= RICE_ICON_BOUNCE_DURATION_MS) {
                this.desktopLaunchAnimations.delete(key);
                return decorated;
            }
            this.onDamage([expandedRect(animation.rect, 16)]);
            return {
                ...decorated,
                rect: {
                    ...decorated.rect,
                    y: decorated.rect.y + riceIconBounceOffset(
                        elapsed,
                        decorated.rect.height,
                    ),
                },
            };
        });
    }
    #commandsWithFileAssociationIcons(commands) {
        const replacements = new Map();
        for (let textIndex = 0; textIndex < commands.length; textIndex += 1) {
            const text = commands[textIndex];
            if (text.type !== "text") continue;
            const mimeType = riceFileMimeType(text.text);
            const fileIcon = riceFileIconName(text.text);
            // Application labels can sit next to the same semantic icons as
            // files (for example Presentation in About). A generic filename
            // fallback is not evidence that the label represents a file.
            if (mimeType === "application/octet-stream" && fileIcon === "file") continue;
            const iconName = this.fileAssociationIcons.get(mimeType) || fileIcon;
            let best = null;
            let bestDistance = Number.POSITIVE_INFINITY;
            for (let index = textIndex - 1; index >= 0; index -= 1) {
                const icon = commands[index];
                if (icon.type !== "icon" || !FILE_ICON_IDS.has(icon.icon)) continue;
                const iconCenterX = icon.rect.x + icon.rect.width / 2;
                const iconCenterY = icon.rect.y + icon.rect.height / 2;
                const textCenterY = text.rect.y + text.rect.height / 2;
                const gridGap = text.rect.y - (icon.rect.y + icon.rect.height);
                const gridAligned = iconCenterX >= text.rect.x - 4 &&
                    iconCenterX <= text.rect.x + text.rect.width + 4 &&
                    gridGap >= -icon.rect.height && gridGap <= 32;
                const rowGap = text.rect.x - (icon.rect.x + icon.rect.width);
                const rowAligned = rowGap >= -4 && rowGap <= 32 &&
                    Math.abs(iconCenterY - textCenterY) <= Math.max(
                        icon.rect.height,
                        text.rect.height,
                    );
                if (!gridAligned && !rowAligned) continue;
                const distance = gridAligned
                    ? Math.abs(gridGap) + Math.abs(
                        iconCenterX - (text.rect.x + text.rect.width / 2),
                    )
                    : Math.abs(rowGap) + Math.abs(iconCenterY - textCenterY);
                if (distance < bestDistance) {
                    best = icon;
                    bestDistance = distance;
                }
            }
            if (best) replacements.set(best, iconName);
        }
        if (replacements.size === 0) return commands;
        return commands.map(command => {
            const namedIcon = replacements.get(command);
            return namedIcon ? { ...command, namedIcon } : command;
        });
    }
    #desktopLaunchKey(command) {
        return `${command.namedIcon}:${command.rect.x}:${command.rect.y}`;
    }
    #renderFrostedIceBackdrop(frame, chrome, root, titlebar) {
        if (chrome.material !== "frosted-ice" || !this.context.canvas) return;
        const context = this.context;
        const rect = {
            x: frame.x,
            y: frame.y,
            width: Math.min(frame.width, root.width - frame.x),
            height: Math.min(frame.height, root.height - frame.y),
        };
        const now = globalThis.performance?.now?.() || Date.now();
        const source = this.cpuTileBackdrop || context.canvas;
        const profile = this.cpuTileBackdrop ? {...chrome.frostedIce, sourceScale:.25,
            backdropCacheKey:this.layoutPaintFrame ? null : this.tileBackdropKey} : chrome.frostedIce;
        context.save();
        context.beginPath();
        context.rect(
            frame.x,
            frame.y + frame.titleHeight,
            rect.width,
            Math.max(0, rect.height - frame.titleHeight),
        );
        context.clip();
        this.frostedIceMaterial.draw(
            context,
            source,
            rect,
            chrome.radius,
            {
                ...profile,
                pointerLight: chrome.frostedIce?.pointerLight === true &&
                    chrome.frostedIce?.windowContentPointerLight !== false,
            },
            now,
            this.layoutPaintFrame || rect,
        );
        context.restore();
        const fullWidth = !titlebar.compact ||
            chrome.windowDecorations
                ?.compactTitlebarFullWidthBackdrop === true;
        const backdropLayout = fullWidth
            ? { ...titlebar, compact: false, width: rect.width }
            : titlebar;
        context.save();
        titlebarPath(context, frame, backdropLayout, frame.x, frame.y);
        context.clip();
        this.frostedIceMaterial.draw(
            context,
            source,
            rect,
            chrome.radius,
            profile,
            now,
            this.layoutPaintFrame || rect,
        );
        context.restore();
    }
    #renderActiveWindowShadow(native, frame, chrome, root, titlebar) {
        if (
            native.shell?.surface?.frame === 'matrix' ||
            chrome.redmond98 ||
            !native.active ||
            native.maximized ||
            native.shell?.activeWindowShadow === false
        ) return;
        const context = this.context;
        context.save();
        try {
            // Keep the shadow outside the window. In particular, it must not
            // become part of the captured Frosted Ice backdrop.
            context.beginPath();
            context.rect(0, 0, root.width, root.height);
            appendWindowSilhouette(context, frame, chrome, titlebar);
            context.clip("evenodd");
            const orchard = chrome.chromeProfile === "orchard";
            context.shadowColor = orchard ? "rgba(28, 57, 88, .24)" : "rgba(0, 0, 0, 0.46)";
            context.shadowBlur = orchard ? 34 : 28;
            context.shadowOffsetX = 0;
            context.shadowOffsetY = orchard ? 12 : 10;
            // Canvas shadows inherit the caster's alpha. The previous almost
            // transparent fill reduced the configured 46% shadow to below 1%,
            // making it effectively invisible. The even-odd clip hides this
            // opaque caster while retaining its full-strength outside shadow.
            context.fillStyle = "#000000";
            context.beginPath();
            appendWindowSilhouette(context, frame, chrome, titlebar);
            context.fill();
        } finally {
            context.restore();
        }
    }
    #renderInactiveOverlay(native) {
        if (
            native.active ||
            native.role === "desktop" ||
            hasTerminalPortal(native) ||
            native.shell?.inactiveWindowsTranslucent === false
        ) return;
        const frame = native.frame;
        const chrome = native.chrome || derivedChrome(native);
        this.context.save();
        try {
            this.context.beginPath();
            appendWindowSilhouette(
                this.context,
                frame,
                chrome,
                nativeTitlebarLayout(
                    frame,
                    native.title,
                    native.titleTools,
                    chrome.windowDecorations,
                ),
            );
            this.context.clip();
            this.context.fillStyle = "rgba(255, 255, 255, 0.055)";
            this.context.fillRect(frame.x, frame.y, frame.width, frame.height);
        } finally {
            this.context.restore();
        }
    }
    #captureTerminalBackdrops(native, frame, chrome) {
        if (!this.context.canvas || typeof globalThis.OffscreenCanvas !== "function") {
            return;
        }
        for (const command of native.commands || []) {
            if (command.type !== "terminal-portal" && command.type !== "terminal-grid") continue;
            if (command.type === "terminal-grid" && this.directTerminalGrid) continue;
            // An opaque grid cannot reveal its backdrop. Avoid copying the
            // desktop into an offscreen surface on every keystroke.
            if (command.type === "terminal-grid" && !(native.commands.find(
                item => item.type === "terminal-row")?.row?.transparency > 0)) continue;
            const width = Math.max(1, Number(command.rect?.width) || 1);
            const height = Math.max(1, Number(command.rect?.height) || 1);
            const key = `${String(native.app)}:${String(command.session)}`;
            let surface = this.terminalBackdrops.get(key);
            const bounds = globalCommandRect(native, command);
            const retained = surface && surface.width === width && surface.height === height;
            this.liveTerminalBackdrops?.add(key);
            if (surface && surface.width === width && surface.height === height &&
                !riceNativeBackdropIntersectsDamage(bounds, this.paintDamageRects,
                    this.backdropOcclusions || [])) continue;
            if (!surface || surface.width !== width || surface.height !== height) {
                surface = new globalThis.OffscreenCanvas(width, height);
                this.terminalBackdrops.set(key, surface);
            }
            const target = surface.getContext("2d", { alpha: command.type === "terminal-grid" });
            if (!target) continue;
            const regions = [];
            target.save();
            try {
                target.setTransform?.(1, 0, 0, 1, 0, 0);
                if (retained && Array.isArray(this.paintDamageRects)) {
                    target.beginPath();
                    for (const damage of this.paintDamageRects) {
                        const left=Math.max(bounds.x,damage.x),top=Math.max(bounds.y,damage.y);
                        const right=Math.min(bounds.x+width,damage.x+damage.width),bottom=Math.min(bounds.y+height,damage.y+damage.height);
                        if(right>left&&bottom>top) {
                            target.rect(left-bounds.x,top-bounds.y,right-left,bottom-top);
                            regions.push({x:left-bounds.x,y:top-bounds.y,width:right-left,height:bottom-top});
                        }
                    }
                    target.clip();
                } else regions.push({x:0,y:0,width,height});
                // An opaque dark fallback prevents a transient white portal if
                // the retained canvas has not produced every backdrop pixel
                // yet (notably while another native window is being added).
                if (command.type === "terminal-grid") {
                    target.clearRect(0, 0, width, height);
                } else {
                    target.fillStyle = "#09090b";
                    target.fillRect(0, 0, width, height);
                }
                // Native terminal transparency reveals the desktop unchanged.
                // A filtered canvas copy forces a costly readback every frame.
                target.filter = command.type === "terminal-portal" && chrome.material === "frosted-ice"
                    ? "saturate(1.2) contrast(1.03)"
                    : "none";
                for (const region of regions) target.drawImage(
                    this.context.canvas,
                    bounds.x + region.x - (this.paintOrigin?.x || 0),
                    bounds.y + region.y - (this.paintOrigin?.y || 0),
                    region.width, region.height,
                    region.x, region.y, region.width, region.height,
                );
                target.filter = "none";
            } finally {
                target.restore();
            }
            this.liveTerminalBackdrops?.add(key);
        }
    }
    #renderWindowContained(native, root) {
        // Advance lifetime even when a scrolling tile is outside its clip.
        // Otherwise its transition keeps scheduling frames indefinitely and
        // can reappear when unrelated input changes the visible geometry.
        const key = `window:${native.app}`;
        const transition = root.preview ? null : this.transitions.get(key);
        const progress = root.preview ? 1 : this.#transitionProgress(native, key);
        if(native.tileClip&&!root.preview) {
            const frame=this.#layoutFrame(native),clip=native.tileClip;
            if(frame.x+frame.width<=clip.x||frame.x>=clip.x+clip.width||
                frame.y+frame.height<=clip.y||frame.y>=clip.y+clip.height)return;
        }
        this.context.save();
        if(native.tileClip&&!root.preview){const clip=native.tileClip;this.context.beginPath();this.context.rect(clip.x,clip.y,clip.width,clip.height);this.context.clip();}
        const previousLayoutFrame = this.layoutPaintFrame;
        const previousRasterScale = this.commandRenderer.motionRasterScale;
        const previousContentAlpha = this.windowContentAlpha;
        const previousBodyAlpha = this.windowBodyAlpha;
        const workStarted = performance.now();
        const previousImageVisible = this.commandRenderer.terminalImageVisible;
        const covers = !root.preview && !this.layoutPreview && !this.dragVisual && !this.transitions.size
            ? this.snapshots.slice(this.snapshots.indexOf(native) + 1).map(riceNativeOpaqueInterior).filter(Boolean) : [];
        this.commandRenderer.terminalImageVisible = rect => !covers.some(cover =>
            cover.x <= rect.x && cover.y <= rect.y &&
            cover.x + cover.width >= rect.x + rect.width && cover.y + cover.height >= rect.y + rect.height);
        try {
            if (native.dialogOnly) {
                if (transition?.direction === "close") return;
                // Dialog motion and material are applied to the pane itself.
                // Window curtains/dimming would repaint its transparent margin.
                this.#renderWindow(native, root);
                return;
            }
            const visualFrame = root.preview ? native.frame : this.#layoutFrame(native);
            if (!root.preview && (visualFrame !== native.frame || this.transitions.has(`window:${native.app}`))) {
                const transform = this.context.getTransform?.();
                this.commandRenderer.motionRasterScale = transform && {x:transform.a,y:transform.d};
            }
            if (visualFrame !== native.frame) {
                this.layoutPaintFrame = visualFrame;
                this.context.translate(visualFrame.x, visualFrame.y);
                this.context.scale(visualFrame.width / native.frame.width, visualFrame.height / native.frame.height);
                this.context.translate(-native.frame.x, -native.frame.y);
            }
            let curtainReveal = 1, curtainProgress = 1;
            let matrixTransition = null;
            if (native.role !== 'desktop' && progress < 1 && native.shell?.surface?.frame === 'matrix' && native.shell?.motion?.definitions?.[transition?.direction === 'close' ? 'window.close' : 'window.open']?.preset === 'Curtain') {
                const width=Math.max(1,native.frame.width)+4,height=Math.max(1,native.frame.height)+4;
                const bounds={x:native.frame.x-2,y:native.frame.y-2,width,height};
                matrixTransition = transition.matrixMorph;
                if (!matrixTransition || matrixTransition.source.width !== width || matrixTransition.source.height !== height) {
                    matrixTransition = transition.matrixMorph = {
                        source:this.frostedIceMaterial.canvasFactory(width,height),
                        backdrop:this.frostedIceMaterial.canvasFactory(width,height),model:null,bounds
                    };
                    const background=matrixTransition.backdrop.getContext('2d');
                    background.drawImage(this.context.canvas,bounds.x,bounds.y,width,height,0,0,width,height);
                }
                matrixTransition.bounds=bounds;
                this.context.drawImage(matrixTransition.backdrop,bounds.x,bounds.y,width,height);
            }
            this.windowContentAlpha = 1;
            this.windowBodyAlpha = 1;
            if (transition?.direction === "close" && progress >= 1) return;
            if (native.role !== "desktop" && progress < 1) {
                const motionProgress = transition?.direction === "close"
                    ? 1 - progress
                    : progress;
                const closing = transition?.direction === "close";
                const motion = windowMotion(native, motionProgress, closing);
                const centerX = native.frame.x + native.frame.width / 2;
                const centerY = native.frame.y + native.frame.height / 2;
                this.context.translate(centerX, centerY);
                this.context.translate(motion.x, motion.y);
                this.context.scale(native.shell?.surface?.frame === 'matrix' ? 1 : motion.scaleX, motion.scaleY);
                this.context.translate(-centerX, -centerY);
                this.context.globalAlpha *= motion.alpha;
                this.windowContentAlpha = motion.contentAlpha;
                if (native.shell?.motion?.window === "Foldout") {
                    this.windowBodyAlpha = motion.contentAlpha;
                }
                const definition = native.shell?.motion?.definitions?.[closing ? 'window.close' : 'window.open'];
                if (definition?.preset === 'Curtain' && !definition.keyframes) {
                    curtainReveal = motion.reveal;
                    curtainProgress = motionProgress;
                    this.windowContentAlpha = this.windowBodyAlpha = 1;
                }
            }
            if (
                native.role !== "desktop" &&
                !native.active &&
                !hasTerminalPortal(native) &&
                native.shell?.inactiveWindowsTranslucent !== false
            ) {
                this.context.globalAlpha *= 0.88;
            }
            let grids = !native.dialog ? native.commands?.filter(command => command.type === "terminal-grid")||[] : [];
            if (grids.some(grid => terminalGridHasForeground(native, grid))) grids = [];
            if (grids.length) {
                // Keep the desktop under the terminal intact while painting its
                // ordinary window chrome. Then alpha-composite the grid in place:
                // no canvas readback or separate terminal presentation surface.
                const opticalGlass = native.chrome?.frostedIce?.model === 'hyprglass';
                this.context.save();
                this.directTerminalGrid = true;
                try {
                    if (!opticalGlass) {
                        this.context.beginPath();
                        this.context.rect(0, 0, root.width, root.height);
                        for(const grid of grids){const bounds=globalCommandRect(native,grid);this.context.rect(bounds.x,bounds.y,bounds.width,bounds.height);}
                        this.context.clip("evenodd");
                    }
                    this.#renderWindow(native, root);
                } finally {
                    this.directTerminalGrid = false;
                    this.context.restore();
                }
                for(const grid of grids){
                this.context.save();
                try {
                    this.context.translate(native.frame.x, native.frame.y + native.frame.titleHeight);
                    this.context.beginPath();
                    this.context.rect(grid.rect.x, grid.rect.y, grid.rect.width, grid.rect.height);
                    this.context.clip();
                    const rows = native.commands.filter(command => command.type === "terminal-row"&&command.rect.x>=grid.rect.x&&command.rect.x<grid.rect.x+grid.rect.width&&command.rect.y>=grid.rect.y&&command.rect.y<grid.rect.y+grid.rect.height);
                    this.commandRenderer.render([
                        { ...grid, directBackdrop: true, transparency: opticalGlass ? 94 : rows[0]?.row?.transparency || 0 },
                        ...rows.filter(command => riceNativeCommandIntersectsDamage(native, command, this.paintDamageRects)),
                    ], native.chrome || derivedChrome(native));
                    if(grids.length>1){
                        const focused=Number(native.focusedWidget)===100000+Number(grid.session)*512;
                        const color=focused?(native.chrome?.selection||[120,167,244,255]):(native.chrome?.border||[83,91,111,255]);
                        this.context.strokeStyle=rgbaColor(color);this.context.lineWidth=focused?2:1;
                        this.context.strokeRect(grid.rect.x+1,grid.rect.y+1,Math.max(0,grid.rect.width-2),Math.max(0,grid.rect.height-2));
                    }
                } finally { this.context.restore(); }
                }
            } else this.#renderWindow(native, root);
            if (matrixTransition) {
                this.#renderInactiveOverlay(native);
                const {source,backdrop,bounds:frame}=matrixTransition;
                const capture=source.getContext('2d');capture.clearRect(0,0,source.width,source.height);
                capture.drawImage(this.context.canvas,frame.x,frame.y,frame.width,frame.height,0,0,source.width,source.height);
                if(!matrixTransition.model || matrixTransition.commandCount!==native.commands.length) {
                    matrixTransition.model=createMatrixMorph(source);matrixTransition.commandCount=native.commands.length;
                }
                paintMatrixMorph(this.context,frame,curtainProgress,matrixTransition.model,{background:backdrop});
            } else if (curtainReveal < 1) {
                const accent = native.chrome?.selection || native.chrome?.close || [204,21,21,255];
                this.context.fillStyle = `rgba(${accent[0]},${accent[1]},${accent[2]},${(accent[3] ?? 255)/255})`;
                if (native.shell?.surface?.frame === 'matrix') paintMatrixCurtain(this.context,native.frame,curtainReveal,native.shell.surface.edge,curtainProgress);
                else if (native.shell?.surface?.frame === 'bracket') paintSignalCurtain(this.context,native.frame,curtainReveal,this.context.fillStyle,native.shell.surface);
                else this.context.fillRect(native.frame.x, native.frame.y, native.frame.width * (1-curtainReveal), native.frame.height);
            }
            if (!matrixTransition) this.#renderInactiveOverlay(native);
        } catch (error) {
            this.onFault(Object.freeze({
                app: String(native.app || ""),
                entrypoint: String(native.entrypoint || "native-app"),
                error: error?.message || String(error),
            }));
            this.#renderFaultCard(native, root);
        } finally {
            this.layoutPaintFrame = previousLayoutFrame;
            this.commandRenderer.motionRasterScale = previousRasterScale;
            this.windowContentAlpha = previousContentAlpha;
            this.windowBodyAlpha = previousBodyAlpha;
            this.commandRenderer.terminalImageVisible = previousImageVisible;
            const app = String(native.app);
            this.cpuWorkByApp.set(app, (this.cpuWorkByApp.get(app) || 0) + Math.max(0, performance.now() - workStarted));
            this.context.restore();
        }
        if (native.dialog?.title?.startsWith("panel:drawer-dialog:")) this.#renderFloatingDrawerDialog(native, root);
    }
    #renderFloatingDrawerDialog(native, root) {
        const rect = native.dialog.rect, chrome = native.chrome || derivedChrome(native);
        const commands = native.commands;
        const start = commands.findIndex(command => command.type === 'fill' &&
            command.rect?.x === rect.x && command.rect?.y === rect.y &&
            command.rect?.width === rect.width && command.rect?.height === rect.height);
        if (start < 0) return;
        this.context.save();
        try {
            this.context.beginPath();
            this.context.rect(0, 0, root.width, root.height);
            this.context.clip();
            this.context.translate(native.frame.x, native.frame.y + native.frame.titleHeight);
            this.invalidateBackdrop();
            this.#renderDialogShadow(native, chrome);
            this.#renderDialogMaterial(native, chrome);
            this.#renderSurfaceCommands(native, commands.slice(start), chrome, rect, true);
        } finally {
            this.context.restore();
        }
    }
    #renderContentCommands(native, chrome) {
        const commands = this.#adaptiveStructuralForegrounds(
            this.#commandsWithFileAssociationIcons(this.directTerminalGrid
                ? native.commands.filter(command => command.type !== "terminal-grid" && command.type !== "terminal-row")
                : native.commands),
            native,
            chrome,
        )
            .map(command => hyprglassSurfaceCommand(command, chrome))
            // Let the already blurred window backdrop transmit through Emerald's
            // dark reading planes. Text, images, selection and small controls
            // retain their original opacity.
            .map(command => native.shell?.surface?.frame === 'matrix' && chrome.material === 'frosted-ice' &&
                command.type === 'fill' && command.rect?.width * command.rect?.height > 12000 &&
                command.color?.length >= 3 && Math.max(...command.color.slice(0,3)) < 50 &&
                command.color[1] >= command.color[0] && command.color[1] >= command.color[2]
                    ? {...command,color:[...command.color.slice(0,3),Math.min(command.color[3] ?? 255,148)]}
                    : command)
            .map(command => command.type === "terminal-grid" ? {...command,
                backdrop: this.terminalBackdrop(native.app, command.session),
                transparency: native.commands.find(item => item.type === "terminal-row")?.row?.transparency || 0,
            } : command)
            .map(command => command.type === "web-scene" || command.type === "inline-image"
                ? { ...command, cacheNamespace: String(native.app) }
                : command)
            .map(command => native.entrypoint === "rice-store" && command.type === "icon"
                ? { ...command, colorArtwork: true } : command)
            // Mail actions need the theme's readable ink, including compose
            // controls whose color icon variants can be black in dark themes.
            .map(command => native.entrypoint === "rice-box" && command.type === "icon"
                ? { ...command, monochrome: true } : command)
            // A native status/toolbar repaint must not even issue a cached
            // page-sized drawImage. The compositor's clip remains the final
            // safety net, while this cull avoids page work altogether.
            .filter(command => command.type !== "web-scene" ||
                riceNativeCommandIntersectsDamage(
                    native,
                    command,
                    this.paintDamageRects,
                )
            );
        const dialog = native.dialog;
        const key = dialog?.widget
            ? `dialog:${native.app}:${String(dialog.widget)}`
            : "";
        const progress = this.#transitionProgress(native, key);
        if (!dialog?.rect) {
            if (this.#renderEditorPreviewTransition(native, commands, chrome)) return;
            this.commandRenderer.render(commands, chrome);
            return;
        }
        const rect = dialog.rect;
        const dialogStart = commands.findIndex(command =>
            command.type === "fill" &&
            command.rect?.x === rect.x &&
            command.rect?.y === rect.y &&
            command.rect?.width === rect.width &&
            command.rect?.height === rect.height
        );
        if (dialogStart < 0) {
            if (native.dialogOnly) return;
            this.commandRenderer.render(commands, chrome);
            return;
        }
        const background = commands.slice(0, dialogStart);
        const foreground = commands.slice(dialogStart);
        if (!native.dialogOnly) this.commandRenderer.render(background, chrome);
        if (dialog.title?.startsWith("panel:drawer-dialog:")) return;
        // A dialog's dense reading plane is explicitly above the application
        // content. Preserve that z-order contract while still retaining one
        // capture for all dialog controls.
        this.invalidateBackdrop();
        if (progress >= 1) {
            this.#renderDialogShadow(native, chrome);
            this.#renderDialogMaterial(native, chrome);
            this.#renderSurfaceCommands(native, foreground, chrome, rect, true);
            return;
        }
        const motion = dialogMotion(native, progress);
        this.context.save();
        const originX = rect.x + rect.width / 2;
        const originY = rect.y + rect.height / 2;
        this.context.translate(originX, originY);
        this.context.translate(motion.x, motion.y);
        this.context.scale(native.shell?.surface?.frame === 'matrix' ? 1 : motion.scaleX, motion.scaleY);
        this.context.translate(-originX, -originY);
        this.context.globalAlpha *= motion.alpha;
        this.#renderDialogShadow(native, chrome);
        this.context.beginPath();
        this.context.rect(rect.x, rect.y, rect.width, rect.height);
        this.context.clip();
        this.#renderDialogMaterial(native, chrome);
        this.#renderSurfaceCommands(native, foreground, chrome, rect, true);
        this.context.restore();
    }
    #adaptiveStructuralForegrounds(commands, native, chrome) {
        const source = this.structuralBackdrop;
        const panels = commands.filter(command =>
            command.type === "glass" && command.structural === true && command.rect
        ).map(command => {
            const tint = chrome?.frostedIce?.enabled === true &&
                Array.isArray(chrome.frostedIce.tint)
                ? [...chrome.frostedIce.tint.slice(0, 3), command.tint?.[3] ?? 112]
                : command.tint;
            return {
                rect: command.rect,
                tint,
            };
        });
        if (!source || panels.length === 0) return commands;
        const sameRgb = (left, right) => Array.isArray(left) && Array.isArray(right) &&
            left[0] === right[0] && left[1] === right[1] && left[2] === right[2];
        return commands.map(command => {
            if (!command.rect || !["text", "icon"].includes(command.type)) return command;
            const centerX = command.rect.x + command.rect.width / 2;
            const centerY = command.rect.y + command.rect.height / 2;
            const panel = panels.find(candidate =>
                centerX >= candidate.rect.x &&
                centerX <= candidate.rect.x + candidate.rect.width &&
                centerY >= candidate.rect.y &&
                centerY <= candidate.rect.y + candidate.rect.height
            );
            if (!panel) return command;
            const adaptiveText = command.type === "text" &&
                sameRgb(command.color, chrome?.text);
            const adaptiveIcon = command.type === "icon" &&
                chrome?.iconVariant !== "color" &&
                sameRgb(command.color, chrome?.icon);
            if (!adaptiveText && !adaptiveIcon) return command;
            const ink = adaptiveStructuralInk(
                source,
                {
                    x: native.frame.x + command.rect.x,
                    y: native.frame.y + native.frame.titleHeight + command.rect.y,
                    width: command.rect.width,
                    height: command.rect.height,
                },
                panel.tint,
                command.color,
            );
            return {
                ...command,
                color: [
                    ink[0],
                    ink[1],
                    ink[2],
                    command.color?.[3] ?? 255,
                ],
            };
        });
    }
    #renderEditorPreviewTransition(native, commands, chrome) {
        const key = `editor-preview:${native.app}`;
        const transition = this.transitions.get(key);
        if (!transition || native.shell?.motionEnabled !== true) return false;
        const progress = this.#transitionProgress(native, key);
        const eased = 1 - (1 - progress) ** 3;
        const rect = transition.rect;
        if (!rect || rect.width <= 0 || rect.height <= 0) return false;
        if (transition.direction === "close") {
            this.commandRenderer.render(commands, chrome);
            this.context.save();
            this.context.beginPath();
            this.context.rect(rect.x, rect.y, rect.width, rect.height);
            this.context.clip();
            this.context.translate(eased * rect.width, 0);
            this.context.globalAlpha *= 1 - eased;
            this.commandRenderer.render((transition.commands || []).map(command=>
                command.type==='inline-image'||command.type==='web-scene'
                    ? {...command,cacheNamespace:String(native.app)}:command), chrome);
            this.context.restore();
            return true;
        }
        this.context.save();
        this.context.beginPath();
        this.context.rect(0, 0, native.frame.width, native.frame.contentHeight);
        this.context.rect(rect.x, rect.y, rect.width, rect.height);
        this.context.clip("evenodd");
        this.commandRenderer.render(commands, chrome);
        this.context.restore();
        this.context.save();
        this.context.beginPath();
        this.context.rect(rect.x, rect.y, rect.width, rect.height);
        this.context.clip();
        this.context.translate((1 - eased) * rect.width, 0);
        this.context.globalAlpha *= eased;
        this.commandRenderer.render(commands, chrome);
        this.context.restore();
        return true;
    }
    #renderDialogMaterial(native, chrome) {
        if (chrome.material !== "frosted-ice" || !this.context.canvas) return;
        const rect = native.dialog.rect;
        const panelSurface = native.commands.find(command =>
            command.type === "fill" &&
            command.rect?.x === rect.x &&
            command.rect?.y === rect.y &&
            command.rect?.width === rect.width &&
            command.rect?.height === rect.height
        );
        const radius = panelSurface?.radius ?? Math.min(14, chrome.radius || 8);
        this.frostedIceMaterial.draw(
            this.context,
            this.context.canvas,
            rect,
            radius,
            {
                ...chrome.frostedIce,
                opacity: Math.max(68, Number(chrome.frostedIce?.opacity) || 0),
                blur: Math.max(72, Number(chrome.frostedIce?.blur) || 0),
                rim: false,
            },
            this.now(),
            {
                x: native.frame.x + rect.x,
                y: native.frame.y + native.frame.titleHeight + rect.y,
                width: rect.width,
                height: rect.height,
            },
        );
    }
    #renderDialogShadow(native, chrome) {
        const dialog = native.dialog;
        if (!dialog?.rect) return;
        const context = this.context;
        const rect = dialog.rect;
        const panelSurface = native.commands.find(command =>
            command.type === "fill" &&
            command.rect?.x === rect.x &&
            command.rect?.y === rect.y &&
            command.rect?.width === rect.width &&
            command.rect?.height === rect.height
        );
        const radius = panelSurface?.radius ?? Math.min(14, chrome.radius || 8);
        context.save();
        try {
            context.beginPath();
            context.rect(0, 0, native.frame.width, native.frame.contentHeight);
            if (typeof context.roundRect === "function") {
                context.roundRect(rect.x, rect.y, rect.width, rect.height, radius);
            } else {
                context.rect(rect.x, rect.y, rect.width, rect.height);
            }
            context.clip("evenodd");
            const sideSheet = dialog.title === "panel:side-sheet";
            context.shadowColor = "rgba(0, 0, 0, 0.52)";
            context.shadowBlur = 26;
            context.shadowOffsetX = sideSheet ? -8 : 0;
            context.shadowOffsetY = sideSheet ? 4 : 10;
            context.fillStyle = "#000000";
            roundedPath(context, rect, radius);
            context.fill();
        } finally {
            context.restore();
        }
    }
    #layoutFrame(native) {
        const key = `layout:${native.app}`;
        const transition = this.transitions.get(key);
        if (!transition) return native.frame;
        const duration = native.shell?.motion?.definitions?.['window.layout']?.duration ?? 320;
        const t = Math.max(0, Math.min(1, (this.now() - transition.startedAt) / Math.max(1, duration)));
        if (t >= 1 || native.shell?.motionEnabled !== true) {
            this.transitions.delete(key);
            return native.frame;
        }
        // Smooth deceleration, with no overshoot across neighboring tiles.
        const eased = 1 - Math.pow(1 - t, 4);
        const frame = {...native.frame};
        for (const field of ['x', 'y', 'width', 'height']) {
            frame[field] = transition.from[field] + (transition.to[field] - transition.from[field]) * eased;
        }
        return frame;
    }
    #transitionProgress(native, key) {
        if (!key || native.shell?.motionEnabled !== true) return 1;
        const transition = this.transitions.get(key);
        if (!transition) return 1;
        const window = key.startsWith("window:");
        const kind = key.startsWith("popup:")
            ? "menu"
            : window ? "window" : "dialog";
        if (
            (window && native.shell?.motion?.window === "None") ||
            (kind === "menu" && native.shell?.motion?.menu === "None")
        ) {
            this.transitions.delete(key);
            return 1;
        }
        const foldout = window &&
            native.shell?.motion?.window === "Foldout";
        const duration = native.shell?.motion?.definitions?.[`${kind}.${transition.direction || "open"}`]?.duration ?? riceWindowTransitionDuration({
            speed: native.shell?.motion?.speed,
            kind,
            direction: transition.direction || "open",
            foldout,
        });
        const progress = Math.max(
            0,
            Math.min(1, (this.now() - transition.startedAt) / Math.max(1, duration)),
        );
        if (progress >= 1) this.transitions.delete(key);
        return progress;
    }
    #renderFaultCard(native, root) {
        if (native.role === "desktop") return;
        const frame = native.frame;
        const x = Math.max(0, Math.min(frame.x, root.width - 1));
        const y = Math.max(0, Math.min(frame.y, root.height - 1));
        const width = Math.max(1, Math.min(frame.width, root.width - x));
        const height = Math.max(1, Math.min(frame.height, root.height - y));
        this.context.setTransform?.(1, 0, 0, 1, -(this.paintOrigin?.x || 0), -(this.paintOrigin?.y || 0));
        this.context.fillStyle = "#201c25";
        this.context.fillRect(x, y, width, height);
        this.context.fillStyle = "#ffb4ab";
        this.context.font = '600 14px "RiceWM Sans", "RiceWM CJK", sans-serif';
        this.context.textAlign = "center";
        this.context.textBaseline = "middle";
        this.context.fillText(
            `${native.title || "Application"} stopped drawing safely`,
            x + width / 2,
            y + height / 2,
            Math.max(1, width - 24),
        );
    }
    #renderTitleControls(
        frame,
        chrome,
        maximized,
        hoveredControl = null,
        pressedControl = null,
        title = "RiceWM",
        tools = [],
        decorations = {},
    ) {
        const context = this.context;
        const controls = nativeTitleControlLayout(
            frame, title, tools, decorations,
        );
        const customCaption = decorations.buttonStyle === 'Borderless' || Object.keys(decorations).some(key=> /^(caption|glyph)/.test(key));
        if(customCaption) {
            for(const control of controls) {
                if(control.action.startsWith('tile-'))continue;
                const close=control.action==='close', hovered=!control.disabled&&control.action===hoveredControl, pressed=!control.disabled&&control.action===pressedControl;
                const state=pressed?'Pressed':hovered?'Hover':'';
                const pick=(kind,fallback)=>themeColor(decorations['caption'+(close?'Close':'')+state+kind],themeColor(decorations['caption'+state+kind],fallback));
                const background=pick('Background',decorations.buttonStyle==='Borderless'?(hovered||pressed?chrome.pressed:[0,0,0,0]):chrome.control);
                const ink=pick('Foreground',chrome.controlText||chrome.text);
                context.save();
                if(control.disabled)context.globalAlpha*=.35;
                const b=control.button;
                context.fillStyle=rgbaColor(background);roundedPath(context,b,decorations.captionRadius??0);context.fill();
                if(decorations.buttonStyle!=='Borderless'){context.strokeStyle=rgbaColor(chrome.border);context.lineWidth=1;roundedPath(context,b,decorations.captionRadius??0);context.stroke();}
                const icon=control.action==='maximize'&&maximized?57:control.icon;
                if(decorations.glyphStyle==='geometric'&&!this.iconRenderer.hasOverride(icon)) {
                    const g=control.glyph,w=g.width,h=g.height;
                    context.strokeStyle=rgbaColor(ink);context.lineWidth=decorations.glyphStrokeWidth??1;context.lineCap='butt';
                    context.beginPath();
                    if(close){context.moveTo(g.x,g.y);context.lineTo(g.x+w,g.y+h);context.moveTo(g.x+w,g.y);context.lineTo(g.x,g.y+h);}
                    else if(control.action==='minimize'){context.moveTo(g.x,g.y+h/2);context.lineTo(g.x+w,g.y+h/2);}
                    else if(maximized){const d=Math.max(2,Math.min(w,h)*.25);context.rect(g.x,g.y+d,w-d,h-d);context.moveTo(g.x+d,g.y+d);context.lineTo(g.x+d,g.y);context.lineTo(g.x+w,g.y);context.lineTo(g.x+w,g.y+h-d);context.lineTo(g.x+w-d,g.y+h-d);}
                    else context.rect(g.x,g.y,w,h);
                    context.stroke();
                } else if(!this.iconRenderer.renderWithColor({type:'icon',icon,color:ink,rect:control.glyph,hovered,hoverBackground:background}))this.#renderControlFallback(icon,control.glyph,ink);
                context.restore();
            }
            // Tiling actions retain their established renderer and geometry.
            if(!decorations.tiled)return;
        }
        const orchard = chrome.chromeProfile === "orchard";
        const yellowJacket = chrome.chromeProfile === "yellow-jacket";
        const wedged = chrome.buttonStyle === "Wedged";
        const chamfered = chrome.buttonStyle === "Chamfered";
        const commandDeck = chrome.chromeProfile === "command-deck";
        for (const control of controls) {
            if (control.action.startsWith('tile-')) {
                if(control.disabled){context.save();context.globalAlpha*=0.35;}
                const hovered=!control.disabled&&control.action===hoveredControl, pressed=!control.disabled&&control.action===pressedControl;
                const b=control.button,cx=b.x+b.width/2,cy=b.y+b.height/2;
                if(hovered||pressed){
                    context.fillStyle=rgbaColor(chrome.pressed);
                    roundedPath(context,b,wedged||chamfered?0:Math.min(4,b.width/4));context.fill();
                }
                context.strokeStyle=rgbaColor(hovered?chrome.selectionText||chrome.text:chrome.text);
                context.lineWidth=1.5;context.lineCap='round';context.lineJoin='round';
                context.beginPath();
                const direction=control.action.slice(5);
                if(['up','down','left','right'].includes(direction)){
                    const dx=direction==='left'?-1:direction==='right'?1:0,dy=direction==='up'?-1:direction==='down'?1:0;
                    context.moveTo(cx-dx*4,cy-dy*4);context.lineTo(cx+dx*4,cy+dy*4);
                    context.moveTo(cx+dx*1-dy*3,cy+dy*1+dx*3);context.lineTo(cx+dx*4,cy+dy*4);context.lineTo(cx+dx*1+dy*3,cy+dy*1-dx*3);
                }else if(direction==='expand'){
                    for(const [dx,dy] of [[-1,-1],[1,-1],[-1,1],[1,1]]){
                        context.moveTo(cx+dx*2,cy+dy*5);context.lineTo(cx+dx*5,cy+dy*5);context.lineTo(cx+dx*5,cy+dy*2);
                    }
                }else{
                    context.moveTo(cx-4,cy);context.lineTo(cx+4,cy);
                    if(direction==='grow'){context.moveTo(cx,cy-4);context.lineTo(cx,cy+4);}
                }
                context.stroke();context.lineCap='butt';context.lineJoin='miter';
                if(control.disabled)context.restore();
                continue;
            }
            if (customCaption) continue;
            if (chrome.redmond98) {
                // The layout owns the classic 16x14 caption-button metric and
                // its four-pixel right gutter.  Do not reconstruct it here:
                // the old hard-coded y=4 pulled the controls into the outer
                // bevel even though hit testing and layout correctly used y=6.
                const button = control.button;
                const pressed = control.action === pressedControl;
                context.fillStyle = "#c0c0c0";
                context.fillRect(button.x, button.y, button.width, button.height);
                paintRedmondBevel(context, button, pressed);
                paintClassicCaptionGlyph(context, button, control.action, maximized, pressed);
                continue;
            }
            if (orchard) {
                const colors = {
                    close: [255, 95, 87, 255],
                    minimize: [255, 189, 46, 255],
                    maximize: [40, 200, 64, 255],
                };
                const diameter = Math.min(13, control.button.width, control.button.height);
                const trafficLight = {
                    x: control.button.x + (control.button.width - diameter) / 2,
                    y: control.button.y + (control.button.height - diameter) / 2,
                    width: diameter,
                    height: diameter,
                };
                context.fillStyle = rgbaColor(colors[control.action]);
                roundedPath(context, trafficLight, diameter / 2);
                context.fill();
                context.strokeStyle = "rgba(0, 0, 0, .18)";
                context.lineWidth = 1;
                roundedPath(context, trafficLight, diameter / 2);
                context.stroke();
                continue;
            }
            if (commandDeck) {
                const palette = {
                    minimize: [255, 204, 102, 255],
                    maximize: [204, 153, 204, 255],
                    close: [239, 139, 114, 255],
                };
                const button = {
                    x: control.button.x + 1,
                    y: control.button.y + 5,
                    width: Math.max(8, control.button.width - 2),
                    height: Math.max(7, control.button.height - 10),
                };
                context.fillStyle = rgbaColor(
                    control.action === hoveredControl
                        ? chrome.pressed
                        : palette[control.action],
                );
                roundedPath(context, button, button.height / 2);
                context.fill();
                continue;
            }
            if (yellowJacket) {
                // This artwork is the complete control surface, not an icon
                // nested inside the generic title-control button.
                const inset = 2;
                this.#renderYellowJacketClose({
                    x: control.cell.x + inset,
                    y: control.cell.y + inset,
                    width: control.cell.width - inset * 2,
                    height: control.cell.height - inset * 2,
                }, {
                    hovered: control.action === hoveredControl,
                    pressed: control.action === pressedControl,
                });
                continue;
            }
            if (wedged || chamfered) {
                const { x, y, width, height } = control.button;
                const cut = Math.min(5, width / 4, height / 4);
                context.beginPath();
                if (wedged) {
                    // "Wedged" is the Neon control-box silhouette: square at
                    // the upper-right/lower-left, cut at the opposing corners.
                    context.moveTo(x + cut, y);
                    context.lineTo(x + width, y);
                    context.lineTo(x + width, y + height - cut);
                    context.lineTo(x + width - cut, y + height);
                    context.lineTo(x, y + height);
                    context.lineTo(x, y + cut);
                } else {
                    context.moveTo(x + cut, y);
                    context.lineTo(x + width - cut, y);
                    context.lineTo(x + width, y + cut);
                    context.lineTo(x + width, y + height - cut);
                    context.lineTo(x + width - cut, y + height);
                    context.lineTo(x + cut, y + height);
                    context.lineTo(x, y + height - cut);
                    context.lineTo(x, y + cut);
                }
                context.closePath();
                const controlFill = control.action === hoveredControl
                    ? chrome.pressed
                    : chrome.control;
                context.fillStyle = rgbaColor(controlFill);
                context.fill();
                context.strokeStyle = rgbaColor(
                    control.action === "close" ? chrome.close : chrome.border,
                );
                context.lineWidth = 1;
                context.stroke();
                if (chamfered) {
                    context.strokeStyle = rgbaColor(chrome.controlLight || chrome.text);
                    context.beginPath();
                    context.moveTo(x + 1, y + height - cut);
                    context.lineTo(x + 1, y + cut);
                    context.lineTo(x + cut, y + 1);
                    context.lineTo(x + width - cut, y + 1);
                    context.stroke();
                    context.strokeStyle = rgbaColor(chrome.controlShadow || chrome.border);
                    context.beginPath();
                    context.moveTo(x + width - cut, y + height - 1);
                    context.lineTo(x + cut, y + height - 1);
                    context.lineTo(x + 1, y + height - cut);
                    context.moveTo(x + width - 1, y + cut);
                    context.lineTo(x + width - 1, y + height - cut);
                    context.lineTo(x + width - cut, y + height - 1);
                    context.stroke();
                }
                continue;
            }
            if(chrome.chromeProfile==='redmond-xp'){
                const b=control.button,base=control.action==='close'?chrome.close:chrome.control;
                const gradient=context.createLinearGradient(b.x,b.y,b.x+b.width,b.y+b.height);
                const shade=(amount)=>rgbaColor(base.map((v,i)=>i===3?v:Math.round(v+(amount>0?255-v:v)*amount)));
                gradient.addColorStop(0,shade(.65));gradient.addColorStop(.45,shade(.1));gradient.addColorStop(1,shade(control.action===pressedControl?-.45:-.25));
                context.fillStyle=gradient;roundedPath(context,b,3);context.fill();context.strokeStyle='rgba(255,255,255,.85)';context.lineWidth=1;roundedPath(context,{x:b.x+.5,y:b.y+.5,width:b.width-1,height:b.height-1},3);context.stroke();
                paintClassicCaptionGlyph(context,b,control.action,maximized,control.action===pressedControl,base[0]*.299+base[1]*.587+base[2]*.114>150?'#101010':'#ffffff');
                continue;
            }
            if (
                chrome.material === "frosted-ice" &&
                control.action !== hoveredControl
            ) {
                continue;
            }
            const familiarCloseHover = chrome.familiar &&
                control.action === "close" && control.action === hoveredControl;
            context.fillStyle = rgbaColor(
                chrome.material === "frosted-ice"
                    ? control.action === "close"
                        ? [...chrome.close.slice(0, 3), 190]
                        : [255, 255, 255, 66]
                    : familiarCloseHover
                        ? chrome.close
                        : chrome.control,
            );
            roundedPath(
                context,
                control.button,
                chrome.material === "frosted-ice"
                    ? 5
                    : chrome.radius > 0
                        ? Math.floor(control.button.height / 2)
                        : 0,
            );
            context.fill();
            if (chrome.material === "frosted-ice") continue;
            if (chrome.familiar && !familiarCloseHover) {
                const { x, y, width, height } = control.button;
                context.lineWidth = 1;
                context.beginPath();
                context.strokeStyle = rgbaColor(chrome.controlLight);
                context.moveTo(x + .5, y + height - .5);
                context.lineTo(x + .5, y + .5);
                context.lineTo(x + width - .5, y + .5);
                context.stroke();
                context.beginPath();
                context.strokeStyle = rgbaColor(chrome.controlShadow);
                context.moveTo(x + width - .5, y + .5);
                context.lineTo(x + width - .5, y + height - .5);
                context.lineTo(x + .5, y + height - .5);
                context.stroke();
            } else {
                context.strokeStyle = rgbaColor(chrome.border);
                context.lineWidth = 1;
                roundedPath(
                    context,
                    control.button,
                    chrome.radius > 0 ? Math.floor(control.button.height / 2) : 0,
                );
                context.stroke();
            }
        }
        for (const control of controls) {
            if (customCaption || control.action.startsWith("tile-") || yellowJacket || commandDeck || chrome.redmond98 || (chrome.chromeProfile==='redmond-xp'&&!wedged&&!chamfered)) continue;
            if (orchard && control.action !== hoveredControl) continue;
            const familiarCloseHover = chrome.familiar &&
                control.action === "close" && control.action === hoveredControl;
            const icon = control.action === "maximize" && maximized
                ? 57
                : control.icon;
            const glyphInset = chrome.glassTitlebar && !orchard
                ? 2
                : chrome.material === "frosted-ice" ? 1 : 0;
            const rect = glyphInset
                ? {
                    x: control.glyph.x - glyphInset,
                    y: control.glyph.y - glyphInset,
                    width: control.glyph.width + glyphInset * 2,
                    height: control.glyph.height + glyphInset * 2,
                }
                : control.glyph;
            const rendered = this.iconRenderer.renderAvailable({
                type: "icon",
                icon,
                color: familiarCloseHover
                    ? chrome.text
                    : orchard
                        ? [35, 35, 35, 220]
                        : (wedged || chamfered) && control.action === hoveredControl
                            ? contrastingControlInk(chrome.pressed)
                        : wedged || chamfered
                            ? control.action === "close" ? chrome.close : chrome.border
                        : chrome.controlText || chrome.text,
                rect,
                hovered: control.action === hoveredControl,
                hoverBackground: familiarCloseHover
                    ? chrome.close
                    : chrome.control,
            });
            if (!rendered) {
                this.#renderControlFallback(
                    icon,
                    rect,
                    familiarCloseHover
                        ? chrome.text
                        : orchard
                            ? [35, 35, 35, 220]
                            : (wedged || chamfered) && control.action === hoveredControl
                                ? contrastingControlInk(chrome.pressed)
                            : wedged || chamfered
                                ? control.action === "close" ? chrome.close : chrome.border
                            : chrome.controlText || chrome.text,
                );
            }
        }
    }
    #renderYellowJacketClose(control, { hovered = false, pressed = false } = {}) {
        const context = this.context;
        const sourceWidth = 37;
        const sourceHeight = 38;
        const scaleX = control.width / sourceWidth;
        const scaleY = control.height / sourceHeight;
        const polygon = (color, points, alpha = 1) => {
            const previousAlpha = context.globalAlpha;
            context.save();
            context.globalAlpha = previousAlpha * alpha;
            context.fillStyle = color;
            context.beginPath();
            context.moveTo(points[0][0], points[0][1]);
            for (const [pointX, pointY] of points.slice(1)) {
                context.lineTo(pointX, pointY);
            }
            context.closePath();
            context.fill();
            context.restore();
            context.globalAlpha = previousAlpha;
        };
        context.save();
        context.translate(control.x, control.y);
        context.scale(scaleX, scaleY);
        // Exact layered geometry of the supplied BeOS-style close gadget.
        context.fillStyle = "#FFD500";
        context.fillRect(0, 0, 37, 38);
        polygon("#B89F00", [[3, 4], [34, 4], [32, 6], [5, 6]]);
        polygon("#B89F00", [[3, 4], [5, 6], [5, 32], [3, 34]]);
        polygon("#FFF04A", [[3, 34], [5, 32], [32, 32], [34, 34]]);
        polygon("#FFF04A", [[34, 4], [34, 34], [32, 32], [32, 6]]);
        context.fillStyle = "#E7C400";
        context.fillRect(5, 6, 27, 26);
        polygon(
            pressed ? "#9A8700" : "#FFF35C",
            [[7, 8], [30, 8], [28, 10], [9, 10]],
        );
        polygon(
            pressed ? "#9A8700" : "#FFE84A",
            [[7, 8], [9, 10], [9, 29], [7, 31]],
        );
        polygon(
            pressed ? "#FFF35C" : "#9A8700",
            [[7, 31], [9, 29], [28, 29], [30, 31]],
        );
        polygon(
            pressed ? "#FFE84A" : "#9A8700",
            [[30, 8], [30, 31], [28, 29], [28, 10]],
        );
        context.fillStyle = pressed
            ? "#D7B600"
            : hovered
                ? "#FFF06A"
                : "#F4CF17";
        context.fillRect(9, 10, 19, 19);
        polygon(
            "#FFE43A",
            [[9, 29], [28, 10], [28, 12.3], [11.3, 29]],
            pressed ? .25 : hovered ? .78 : .6,
        );
        context.restore();
    }
    #renderTitleTools(
        frame,
        chrome,
        tools,
        hoveredControl,
        title = "RiceWM",
        decorations = {},
    ) {
        for (const tool of nativeTitleToolLayout(
            frame, tools, title, decorations,
        )) {
            if (hoveredControl === tool.action || ["Wedged", "Chamfered"].includes(
                chrome.buttonStyle,
            )) {
                this.context.fillStyle = rgbaColor(chrome.control);
                const { x, y, width, height } = tool.button;
                const cut = Math.min(5, width / 4, height / 4);
                if (chrome.buttonStyle === "Wedged") {
                    this.context.beginPath();
                    this.context.moveTo(x + cut, y);
                    this.context.lineTo(x + width, y);
                    this.context.lineTo(x + width, y + height - cut);
                    this.context.lineTo(x + width - cut, y + height);
                    this.context.lineTo(x, y + height);
                    this.context.lineTo(x, y + cut);
                    this.context.closePath();
                } else if (chrome.buttonStyle === "Chamfered") {
                    this.context.beginPath();
                    this.context.moveTo(x + cut, y);
                    this.context.lineTo(x + width - cut, y);
                    this.context.lineTo(x + width, y + cut);
                    this.context.lineTo(x + width, y + height - cut);
                    this.context.lineTo(x + width - cut, y + height);
                    this.context.lineTo(x + cut, y + height);
                    this.context.lineTo(x, y + height - cut);
                    this.context.lineTo(x, y + cut);
                    this.context.closePath();
                } else {
                    roundedPath(
                        this.context,
                        tool.button,
                        chrome.buttonStyle === "Pill"
                            ? tool.button.height / 2
                            : chrome.radius > 0 ? 4 : 0,
                    );
                }
                this.context.fill();
            }
            const rendered = this.iconRenderer.renderAvailable({
                type: "icon",
                icon: tool.icon,
                color: chrome.text,
                rect: tool.glyph,
                hovered: hoveredControl === tool.action,
            });
            if (!rendered) {
                this.#renderControlFallback(tool.icon, tool.glyph, chrome.text);
            }
        }
    }
    #renderControlFallback(icon, rect, color) {
        const context = this.context;
        context.strokeStyle = rgbaColor(color);
        context.lineWidth = 1.25;
        context.beginPath();
        const left = rect.x;
        const top = rect.y;
        const right = rect.x + rect.width;
        const bottom = rect.y + rect.height;
        if (icon === 55) {
            context.moveTo(left, top + rect.height / 2);
            context.lineTo(right, top + rect.height / 2);
        } else if (icon === 54) {
            context.moveTo(left, top);
            context.lineTo(right, bottom);
            context.moveTo(right, top);
            context.lineTo(left, bottom);
        } else if (icon === 57) {
            const offset = 2;
            context.rect(
                left + offset,
                top,
                rect.width - offset,
                rect.height - offset,
            );
            context.rect(
                left,
                top + offset,
                rect.width - offset,
                rect.height - offset,
            );
        } else {
            context.rect(left, top, rect.width, rect.height);
        }
        context.stroke();
    }
}
function popupIdentity(native) {
    const bounds = native?.popup?.rect;
    // The popup bounds include every open submenu. Cascading must not restart
    // the parent menu's animation or retain a fading copy of the whole chain.
    const rect = bounds && popupSurfaceRects(native.popupCommands, bounds)[0];
    return rect
        ? `${rect.x}:${rect.y}:${rect.width}:${rect.height}`
        : "";
}
function editorPreviewRect(native) {
    const title = native?.commands?.find(command =>
        command.type === "text" && command.text === "Live preview"
    );
    if (!title?.rect || !native?.frame) return null;
    const x = Math.max(0, title.rect.x - 12);
    const y = Math.max(0, title.rect.y - 10);
    return {
        x,
        y,
        width: Math.max(0, native.frame.width - x),
        height: Math.max(0, native.frame.contentHeight - y),
    };
}
function eased(progress) {
    const remaining = 1 - Math.max(0, Math.min(1, progress));
    return 1 - remaining * remaining * remaining;
}
function spring(progress, elastic = true) {
    if (!elastic) return eased(progress);
    const value = 1 - Math.exp(-7.5 * progress) * Math.cos(10.5 * progress);
    return Math.max(0, Math.min(1.075, value));
}
function windowMotion(native, progress, closing = false) {
    const definition = native.shell?.motion?.definitions?.[closing ? "window.close" : "window.open"];
    if (definition) {
        const motion = sampleThemeMotion(definition, progress, native.shell?.motionEnabled !== true);
        return { ...motion, alpha: motion.opacity, contentAlpha: motion.reveal };
    }
    const style = native.shell?.motion?.window || "Spring";
    const elastic = native.shell?.motion?.elastic !== false;
    if (style === "None") {
        return { x: 0, y: 0, scaleX: 1, scaleY: 1, alpha: 1, contentAlpha: 1 };
    }
    if (style === "Foldout") {
        const frameProgress = closing ? progress : Math.min(1, progress / .72);
        const frame = frameProgress * frameProgress * (3 - 2 * frameProgress);
        const content = closing
            ? Math.min(1, progress * 2.4)
            : eased(Math.max(0, Math.min(1, (progress - .70) / .30)));
        return {
            x: 0,
            y: 0,
            scaleX: .055 + .945 * frame,
            scaleY: .025 + .975 * frame,
            alpha: 1,
            contentAlpha: content,
        };
    }
    const value = closing
        ? progress * progress
        : style === "Spring" ? spring(progress, elastic) : eased(progress);
    if (style === "Glide") {
        return {
            x: 0,
            y: 28 * (1 - value),
            scaleX: 1,
            scaleY: 1,
            alpha: value,
            contentAlpha: 1,
        };
    }
    const start = style === "Scale" ? .9 : .88;
    return {
        x: 0,
        y: style === "Spring" ? 18 * (1 - value) : 0,
        scaleX: start + (1 - start) * value,
        scaleY: start + (1 - start) * value,
        alpha: Math.min(1, eased(progress) * 1.35),
        contentAlpha: 1,
    };
}
function menuMotion(native, progress, closing = false) {
    const definition = native.shell?.motion?.definitions?.[closing ? "menu.close" : "menu.open"];
    if (definition) {
        const motion = sampleThemeMotion(definition, progress, native.shell?.motionEnabled !== true);
        return { ...motion, alpha: motion.opacity };
    }
    const style = native.shell?.motion?.menu || "Spring Expand";
    const elastic = native.shell?.motion?.elastic !== false;
    if (style === "None") {
        return { x: 0, y: 0, scaleX: 1, scaleY: 1, alpha: 1 };
    }
    const value = style === "Spring Expand"
        ? spring(progress, elastic)
        : eased(progress);
    if (style === "Slide") {
        return {
            x: 34 * (1 - value),
            y: 0,
            scaleX: 1,
            scaleY: 1,
            alpha: value,
        };
    }
    if (style === "Bloom") {
        return {
            x: 0,
            y: 8 * (1 - value),
            scaleX: .86 + .14 * value,
            scaleY: .86 + .14 * value,
            alpha: Math.min(1, value * 1.4),
        };
    }
    return {
        x: 0,
        y: -5 * (1 - value),
        scaleX: .96 + .04 * value,
        scaleY: .72 + .28 * value,
        alpha: Math.min(1, eased(progress) * 1.45),
    };
}
function dialogMotion(native, progress) {
    const base = menuMotion(native, progress);
    if (native.dialog?.title !== "panel:side-sheet") return base;
    const value = spring(
        progress,
        native.shell?.motion?.elastic !== false,
    );
    return {
        x: 76 * (1 - value),
        y: 0,
        scaleX: .97 + .03 * value,
        scaleY: 1,
        alpha: Math.min(1, eased(progress) * 1.3),
    };
}
function validDragVisual(value) {
    if (!value) return null;
    const x = Number(value.x);
    const y = Number(value.y);
    const sourceX = Number(value.sourceX);
    const sourceY = Number(value.sourceY);
    if (
        !Number.isFinite(x) ||
        !Number.isFinite(y) ||
        !Number.isFinite(sourceX) ||
        !Number.isFinite(sourceY)
    ) return null;
    return Object.freeze({
        x: Math.max(0, Math.min(16_384, Math.floor(x))),
        y: Math.max(0, Math.min(16_384, Math.floor(y))),
        sourceX: Math.max(0, Math.min(16_384, Math.floor(sourceX))),
        sourceY: Math.max(0, Math.min(16_384, Math.floor(sourceY))),
        label: String(value.label || "File").slice(0, 96),
        count: Math.max(1, Math.min(128, Math.floor(Number(value.count)) || 1)),
        action: ["copy", "move", "link", "ask"].includes(value.action)
            ? value.action
            : "move",
        accepted: Boolean(value.accepted),
    });
}
function validSnapPreview(value) {
    if (!value) return null;
    const rect = {};
    for (const key of ["x", "y", "width", "height"]) {
        const number = Number(value[key]);
        if (!Number.isFinite(number)) return null;
        rect[key] = Math.max(0, Math.min(16_384, Math.floor(number)));
    }
    if (rect.width === 0 || rect.height === 0) return null;
    return Object.freeze(rect);
}
