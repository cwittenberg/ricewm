import {validateThemeIcons} from "./rice-theme-customization.mjs";
import { rgbaColor } from "./rice-native-canvas-primitives.mjs";

const RICE_ICON_FONT_FAMILY = "RiceWM Icons";
const RICE_ICON_FONT_FILE = "assets/icons/ricewm.woff2";
const RICE_ICON_CSS_FILE = "assets/icons/ricewm.css";
const RICE_ICON_WOFF2_DECODER_FILE = "assets/icons/ricewm-woff2-decompress.js";
const RICE_NEW_ICON_DIRECTORY = "assets/icons/new-icons";
const RICE_THEME_ICON_DIRECTORY = "assets/icons/themes";
const SAFE_ICON_NAME = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const RICE_GUEST_APPLICATION_ICON_NAME =
    /^guest-app-[a-z0-9]+(?:-[a-z0-9]+)*-[a-f0-9]{16}$/;
export const RICE_ICON_BOUNCE_DURATION_MS = 1_300;

function matchesMailIcon(name) {
    return name === "mail-unread" || name === "mail-read";
}

const RICE_OT_SVG_COLOR_SEMANTICS = new Set([
    "audio", "binary", "bowl", "css", "csv", "doc", "docker", "documents",
    "downloads", "env", "file-zip", "home-folder", "html", "js",
    "manifest", "markdown", "pdf", "pictures", "ppt", "program", "secret",
    "text", "video", "xls", "yaml", "network-mounts", "trash",
    "desktop-folder", "system-monitor", "windows", "linux", "snap",
]);
const RICE_FORCED_MONOCHROME_SEMANTICS = new Set([
    "bowl",
    "calendar",
    "calendar-empty",
    "inbox",
    "welcome-speed",
    "welcome-desktop",
    "welcome-private",
    "grain",
]);
const RICE_RAW_SVG_SEMANTICS = new Set([
    "welcome-speed",
    "welcome-desktop",
    "welcome-private",
]);
const RICE_FILE_ICON_SEMANTICS = new Set([
    "audio", "binary", "css", "csv", "doc", "docker", "env", "file-zip", "html", "js",
    "manifest", "markdown", "pdf", "ppt", "program", "secret", "text", "video", "xls", "yaml",
]);
const RICE_THEME_RASTER_NO_FALLBACK = new Set([
    "accessibility", "appearance", "taskbar", "extensions", "computer",
    ...RICE_FILE_ICON_SEMANTICS,
    "binary", "bowl", "document-new", "document-save", "document-save-as",
    "chevron-down", "chevron-left", "chevron-right", "chevron-up",
    "edit-copy", "edit-cut", "edit-delete", "edit-paste", "edit-redo",
    "edit-rename", "edit-undo", "format-bold", "format-italic",
    "format-font-smaller", "format-font-larger", "format-align-left", "format-align-center", "format-align-right", "format-align-justify", "format-bullets", "format-numbering",
    "format-strike", "format-underline", "fullscreen", "help", "menu",
    "navigate-up", "object-add", "object-remove", "ricewm-logo", "selection-check",
    "power-on", "presentation",
    "submenu-left", "submenu-right", "view-detail", "view-grid",
    "terminal", "view-refresh", "window-close", "window-maximize", "window-minimize",
    "window-restore", "windows", "linux", "snap",
]);

function relativeLuminance(color) {
    const channel = value => {
        const normalized = Math.max(0, Math.min(255, Number(value) || 0)) / 255;
        return normalized <= .04045
            ? normalized / 12.92
            : ((normalized + .055) / 1.055) ** 2.4;
    };
    return .2126 * channel(color?.[0]) +
        .7152 * channel(color?.[1]) +
        .0722 * channel(color?.[2]);
}

function contrastRatio(left, right) {
    const light = Math.max(relativeLuminance(left), relativeLuminance(right));
    const dark = Math.min(relativeLuminance(left), relativeLuminance(right));
    return (light + .05) / (dark + .05);
}

export function riceContrastingIconColor(hover, background, fallback) {
    if (!Array.isArray(hover) || !Array.isArray(background)) return hover;
    if (contrastRatio(hover, background) >= 3) return hover;
    const candidates = [
        Array.isArray(fallback) ? fallback : null,
        [255, 255, 255, 255],
        [0, 0, 0, 255],
    ].filter(Boolean);
    return candidates.reduce((best, candidate) =>
        contrastRatio(candidate, background) > contrastRatio(best, background)
            ? candidate
            : best
    );
}

export function riceIconBounceOffset(elapsed, size) {
    const progress = Math.max(
        0,
        Math.min(1, Number(elapsed) / RICE_ICON_BOUNCE_DURATION_MS),
    );
    const keyframes = [
        [0, 0],
        [.2, 0],
        [.4, -.25],
        [.5, 0],
        [.6, -.125],
        [.8, 0],
        [1, 0],
    ];
    for (let index = 1; index < keyframes.length; index += 1) {
        const [rightProgress, rightOffset] = keyframes[index];
        if (progress > rightProgress) continue;
        const [leftProgress, leftOffset] = keyframes[index - 1];
        const segment = (progress - leftProgress) /
            Math.max(Number.EPSILON, rightProgress - leftProgress);
        const eased = .5 - Math.cos(segment * Math.PI) / 2;
        return Number(size) * (leftOffset + (rightOffset - leftOffset) * eased);
    }
    return 0;
}

export const RICE_ICON_GLYPHS = Object.freeze({
    "audio-color": "\ueb27",
    "bowl-color": "\ueb23",
    bowl: "\ueb24",
    "ricewm-logo-color": "\uea01",
    "ricewm-logo": "\uea02",
    audio: "\ueb28",
    "audio-muted": "\uea04",
    "audio-volume": "\uea06",
    camera: "\ueac2",
    calculator: "\uea08",
    calendar: "\ueb1e",
    inbox: "\ueaf4",
    "grain-color": "\ueb21",
    grain: "\ueb22",
    "user-color": "\uea97",
    user: "\uea98",
    "comment-color": "\uead1",
    comment: "\uead2",
    coffee: "\uead0",
    "binary-color": "\ueb29",
    binary: "\ueb2a",
    "chevron-down": "\uea0a",
    "chevron-left": "\uea0c",
    "chevron-right": "\uea0e",
    "chevron-up": "\uea10",
    desktop: "\uea12",
    "css-color": "\ueb2b",
    css: "\ueb2c",
    "csv-color": "\ueb2d",
    csv: "\ueb2e",
    "doc-color": "\ueb2f",
    doc: "\ueb30",
    "docker-color": "\ueb31",
    docker: "\ueb32",
    "documents-color": "\ueb33",
    documents: "\ueb34",
    "downloads-color": "\ueb35",
    downloads: "\ueb36",
    "env-color": "\ueb37",
    env: "\ueb38",
    "dialog-information": "\uea14",
    "dialog-warning": "\uea16",
    "document-new": "\uea18",
    "document-open": "\uea1a",
    "document-save": "\uea1c",
    "document-save-as": "\uea1e",
    drive: "\uea20",
    "edit-copy": "\uea22",
    "edit-cut": "\uea24",
    "edit-delete": "\uea26",
    "edit-find": "\uea28",
    "edit-paste": "\uea2a",
    "edit-redo": "\uea2c",
    "edit-rename": "\uea2e",
    "edit-undo": "\uea30",
    file: "\uea32",
    "file-color": "\uea31",
    "file-zip-color": "\ueb39",
    "file-zip": "\ueb3a",
    files: "\uea34",
    folder: "\uea36",
    "folder-color": "\uea35",
    fullscreen: "\uea38",
    home: "\uea3a",
    "home-folder-color": "\ueb3b",
    "home-folder": "\ueb3c",
    "html-color": "\ueb3d",
    html: "\ueb3e",
    image: "\uea3c",
    "image-viewer": "\uea3e",
    link: "\uea40",
    "logout-color": "\uea85",
    logout: "\uea86",
    "js-color": "\ueb3f",
    js: "\ueb40",
    lock: "\uea42",
    "media-pause": "\uea44",
    "media-play": "\uea46",
    "media-seek-back": "\uea48",
    "media-seek-forward": "\uea4a",
    menu: "\uea4c",
    "manifest-color": "\ueb41",
    manifest: "\ueb42",
    "markdown-color": "\ueb43",
    markdown: "\ueb44",
    "navigate-up": "\uea4e",
    network: "\uea50",
    "network-mounts-color": "\ueb57",
    "network-mounts": "\ueb58",
    "object-add": "\uea52",
    "object-remove": "\uea54",
    search: "\uea56",
    "pdf-color": "\ueb45",
    pdf: "\ueb46",
    "pictures-color": "\ueb55",
    pictures: "\ueb56",
    "power-on": "\ueb00",
    "ppt-color": "\ueb47",
    ppt: "\ueb48",
    "program-color": "\ueb49",
    program: "\ueb4a",
    reboot: "\ueb0c",
    "secret-color": "\ueb4b",
    secret: "\ueb4c",
    "selection-check": "\uea58",
    settings: "\uea5a",
    spreadsheet: "\ueaa4",
    "spreadsheet-color": "\ueaa3",
    terminal: "\uea5c",
    "text-color": "\ueb4d",
    text: "\ueb4e",
    "text-editor": "\uea5e",
    "system-monitor-color": "\ueb5e",
    "system-monitor": "\ueb5f",
    "trash-empty": "\uea60",
    "trash-full": "\uea62",
    "trash-color": "\ueb59",
    trash: "\ueb5a",
    "desktop-folder-color": "\ueb5b",
    "desktop-folder": "\ueb5c",
    "windows-color": "\ueb60",
    windows: "\ueb61",
    "linux-color": "\ueb64",
    linux: "\ueb65",
    "snap-color": "\ueb66",
    snap: "\ueb67",
    "zoom-in-color": "\ueae1",
    "zoom-in": "\ueae2",
    "zoom-out-color": "\ueae3",
    "zoom-out": "\ueae4",
    "view-detail": "\uea64",
    "view-grid": "\uea66",
    "view-refresh": "\uea68",
    "video-color": "\ueb4f",
    video: "\ueb50",
    "window-close": "\uea6a",
    "window-maximize": "\uea6c",
    "window-minimize": "\uea6e",
    "window-restore": "\uea70",
    "xls-color": "\ueb51",
    xls: "\ueb52",
    "yaml-color": "\ueb53",
    yaml: "\ueb54",
});

const NATIVE_ICON_NAMES = Object.freeze({
    1: "program",
    2: "file",
    3: "folder",
    4: "folder",
    5: "home-folder",
    6: "trash",
    10: "document-new",
    11: "document-open",
    12: "document-save",
    20: "edit-undo",
    21: "edit-redo",
    22: "edit-cut",
    23: "edit-copy",
    24: "edit-paste",
    25: "edit-delete",
    30: "search",
    31: "chevron-left",
    32: "chevron-right",
    33: "navigate-up",
    34: "view-refresh",
    40: "view-detail",
    41: "view-grid",
    50: "settings",
    51: "dialog-information",
    52: "dialog-warning",
    53: "dialog-warning",
    54: "window-close",
    55: "window-minimize",
    56: "window-maximize",
    57: "window-restore",
    58: "trash-full",
    60: "calculator",
    61: "text-editor",
    62: "files",
    63: "image-viewer",
    64: "drive",
    65: "network",
    66: "lock",
    67: "document-save-as",
    68: "edit-find",
    69: "edit-rename",
    70: "desktop-folder",
    71: "image",
    72: "terminal",
    73: "chevron-up",
    74: "chevron-down",
    75: "menu",
    76: "object-add",
    77: "object-remove",
    78: "selection-check",
    79: "view-detail",
    80: "link",
    81: "media-seek-back",
    82: "media-play",
    83: "media-pause",
    84: "media-seek-forward",
    85: "audio-volume",
    86: "audio-muted",
    87: "fullscreen",
    88: "calendar",
    89: "audio",
    90: "binary",
    91: "css",
    92: "csv",
    93: "doc",
    94: "docker",
    95: "documents",
    96: "downloads",
    97: "env",
    98: "file-zip",
    99: "html",
    100: "js",
    101: "manifest",
    102: "markdown",
    103: "pdf",
    104: "presentation",
    105: "program",
    106: "secret",
    107: "text",
    108: "video",
    109: "spreadsheet",
    110: "yaml",
    111: "pictures",
    112: "network-mounts",
    113: "ricewm-logo",
    114: "welcome-everyday",
    115: "welcome-create",
    116: "welcome-connect",
    117: "welcome-extensions",
    118: "welcome-packages",
    119: "welcome-privacy",
    120: "welcome-hero-color",
    121: "bowl",
    122: "welcome-speed",
    123: "welcome-desktop",
    124: "welcome-private",
    125: "welcome-brand-color",
    126: "format-bold",
    127: "format-italic",
    128: "format-underline",
    129: "format-strike",
    130: "mail-unread",
    131: "mail-read",
    132: "grain",
    133: "star",
    600: "sun",
    601: "star-outline",
    602: "briefcase",
    134: "submenu-right",
    135: "submenu-left",
    136: "windows",
    137: "printer",
    138: "inbox",
    139: "user",
    140: "comment",
    141: "linux",
    142: "snap",
    143: "power-on",
    144: "logout",
    145: "reboot",
    147: "camera",
    148: "attachment",
    149: "mail-reply",
    150: "mail-forward",
    151: "format-align-left",
    152: "format-align-center",
    153: "format-align-right",
    154: "format-align-justify",
    155: "format-bullets",
    156: "format-numbering",
    157: "format-font-smaller",
    158: "format-font-larger",
    159: "google-drive-color",
    160: "nextcloud-color",
    161: "s3-storage-color",
    162: "data-filter",
    163: "data-filter-clear",
    164: "sort-ascending",
    165: "sort-descending",
    166: "formula-function",
    167: "formula-sum",
    168: "formula-lookup",
    169: "fill-down",
    170: "fill-right",
    171: "paste-values",
    172: "paste-formulas",
    173: "row-height",
    174: "column-width",
    175: "format-number",
    176: "format-percent",
    177: "format-currency",
    178: "insert-rows",
    179: "insert-columns",
    180: "delete-rows",
    181: "delete-columns",
    182: "shapes",
    183: "draw-line",
    184: "shape-rectangle",
    185: "shape-ellipse",
    186: "fill-color",
    187: "outline-color",
    188: "format-text-color",
    189: "arrange",
    190: "align-objects",
    191: "distribute-horizontal",
    192: "distribute-vertical",
    193: "crop",
    194: "text-box",
    195: "rotate",
    196: "bring-forward",
    197: "send-backward",
    198: "shape-rectangle",
    199: "shape-rounded-rectangle",
    200: "shape-ellipse",
    201: "shape-triangle",
    202: "shape-right-triangle",
    203: "shape-diamond",
    204: "shape-parallelogram",
    205: "shape-trapezoid",
    206: "shape-cross",
    207: "shape-heart",
    208: "shape-pentagon",
    209: "shape-hexagon",
    210: "shape-heptagon",
    211: "shape-octagon",
    212: "shape-decagon",
    213: "shape-dodecagon",
    214: "shape-right-arrow",
    215: "shape-down-arrow",
    216: "shape-left-arrow",
    217: "shape-up-arrow",
    218: "shape-left-right-arrow",
    219: "shape-up-down-arrow",
    220: "shape-chevron",
    221: "shape-home-plate",
    222: "shape-star4",
    223: "shape-star5",
    224: "shape-star6",
    225: "shape-star8",
    226: "shape-star10",
    227: "shape-star12",
    228: "shape-speech-callout",
    229: "shape-document",
    230: "object-align-left",
    231: "object-align-center",
    232: "object-align-right",
    233: "object-align-top",
    234: "object-align-middle",
    235: "object-align-bottom",
    236: "cell-styles",
    237: "cell-borders",
    238: "format-cells",
    239: "object-group",
    240: "object-ungroup",
    241: "chart-column",
    242: "chart-line",
    243: "chart-format",
    244: "chart-data",
    245: "chart-bar",
    246: "chart-area",
    247: "chart-pie",
    248: "chart-doughnut",
    249: "chart-scatter",
    250: "chart-bubble",
    251: "chart-radar",
    252: "chart-stock",
    253: "chart-combo",
    254: "chart-surface",
    255: "chart-statistical",
    256: "chart-waterfall",
    512: "chart-funnel",
    513: "chart-hierarchy",
    514: "format-painter",
    565: "format-wrap-text",
    566: "format-decimal-increase",
    567: "format-decimal-decrease",
    568: "data-remove-duplicates",
    569: "data-text-to-columns",
    570: "shape-shadow",
    515: "format-line-spacing",
    516: "zoom-in",
    517: "zoom-out",
    518: "draw-polygon",
    519: "draw-polyline",
    520: "shape-snip1-rect",
    521: "shape-snip2-same-rect",
    522: "shape-snip2-diag-rect",
    523: "shape-round1-rect",
    524: "shape-round2-same-rect",
    525: "shape-round2-diag-rect",
    526: "shape-snip-round-rect",
    527: "shape-bevel",
    528: "shape-cylinder",
    529: "shape-cube",
    530: "shape-corner",
    531: "shape-folded-corner",
    532: "shape-lightning-bolt",
    533: "shape-moon",
    534: "shape-sun",
    535: "shape-teardrop",
    536: "shape-donut",
    537: "shape-quad-arrow",
    538: "shape-left-right-up-arrow",
    539: "shape-bent-arrow",
    540: "shape-uturn-arrow",
    541: "shape-left-up-arrow",
    542: "shape-notched-right-arrow",
    543: "shape-math-plus",
    544: "shape-math-minus",
    545: "shape-math-multiply",
    546: "shape-math-divide",
    547: "shape-math-equal",
    548: "shape-math-not-equal",
    549: "shape-flow-chart-predefined-process",
    550: "shape-flow-chart-internal-storage",
    551: "shape-flow-chart-terminator",
    552: "shape-flow-chart-preparation",
    553: "shape-flow-chart-manual-input",
    554: "shape-flow-chart-manual-operation",
    555: "shape-flow-chart-offpage-connector",
    556: "shape-flow-chart-delay",
    557: "shape-flow-chart-display",
    558: "shape-star16",
    559: "shape-star24",
    560: "shape-star32",
    561: "shape-round-callout",
    562: "shape-ellipse-callout",
    563: "xls",
    564: "ppt",
    571: "accessibility",
    572: "appearance",
    573: "taskbar",
    574: "extensions",
    575: "computer",
    576: "brain",
    577: "clock",

});

export function riceNativeIconName(value) {
    return NATIVE_ICON_NAMES[Number(value)] || null;
}

export const RICE_ICON_CATALOG = Object.freeze(Object.keys(RICE_ICON_GLYPHS));
// Non-font native semantics retain their canonical SVG fallback names. Adding
// these names to ricewm.css/WOFF2 automatically takes precedence over the SVG.
const RICE_OFFICE_ICON_NAMES = new Set(Object.entries(NATIVE_ICON_NAMES).filter(([id]) => (Number(id) >= 162 && Number(id) <= 256 || Number(id) >= 512 && Number(id) <= 515)).map(([, name]) => name));
for (const name of RICE_OFFICE_ICON_NAMES) RICE_THEME_RASTER_NO_FALLBACK.add(name);
const RICE_ICON_NAMES = new Set([...RICE_ICON_CATALOG, ...Object.values(NATIVE_ICON_NAMES)]);

export function parseRiceIconCss(source) {
    const glyphs = new Map();
    const rule = /\.icon-([a-z0-9]+(?:-[a-z0-9]+)*)::before\s*\{\s*content:\s*"\\([0-9a-f]{4,6})";\s*\}/gi;
    for (const match of String(source || "").matchAll(rule)) {
        const codepoint = Number.parseInt(match[2], 16);
        if (codepoint >= 0xe000 && codepoint <= 0xf8ff) {
            glyphs.set(match[1].toLowerCase(), String.fromCodePoint(codepoint));
        }
    }
    return glyphs;
}

export function riceIconThemeSlug(value) {
    const slug = String(value || "")
        .trim()
        .toLowerCase()
        .replace(/[’']/g, "")
        .replace(/ +/g, "-");
    // The built-in theme uses the base catalog already loaded above; it has
    // no separate themes/ricewm pack. Avoid two guaranteed 404 fallbacks.
    if (slug === "ricewm" || slug === "construct") return "";
    return SAFE_ICON_NAME.test(slug) ? slug : "";
}

export async function loadRiceIconCatalog(fetch_ = globalThis.fetch) {
    if (typeof fetch_ !== "function") {
        return new Map(Object.entries(RICE_ICON_GLYPHS));
    }
    const response = await fetch_(iconAssetUrl(RICE_ICON_CSS_FILE));
    if (!response.ok) throw new Error(`icon CSS request failed: ${response.status}`);
    const source = await response.text();
    if (source.length > 512 * 1024) throw new Error("icon CSS is too large");
    const glyphs = parseRiceIconCss(source);
    if (glyphs.size < RICE_ICON_CATALOG.length) {
        throw new Error("icon CSS is missing required monochrome glyphs");
    }
    return glyphs;
}

export function riceIconName(value) {
    const source = String(value || "").trim().split(/[\\/]/).at(-1)
        ?.replace(/\.svg$/i, "").replace(/-symbolic$/i, "").toLowerCase() || "";
    if (["appointment-soon", "preferences-system-time"].includes(source)) return "clock";
    if (source === "document-edit") return "text-editor";
    if (source === "show-desktop") return source;
    if (RICE_GUEST_APPLICATION_ICON_NAME.test(source)) return source;
    if (RICE_ICON_NAMES.has(source)) return source;
    for (const [pattern, icon] of [
        [/(?:calc)/, "calculator"],
        [/(?:terminal|console|shell)/, "terminal"],
        [/(?:text|editor|paper|writer)/, "text-editor"],
        [/(?:file.?manager|files)/, "files"],
        [/(?:folder|director)/, "folder"],
        [/(?:image|photo|picture|viewer)/, "image-viewer"],
        [/(?:media|player|video|movie|audio)/, "media-play"],
        [/(?:setting|config|preference)/, "settings"],
        [/(?:desktop|workspace)/, "desktop"],
        [/(?:trash|recycle)/, "trash-empty"],
        [/(?:network|internet|browser|web)/, "network"],
        [/(?:drive|disk|volume)/, "drive"],
        [/(?:search|find)/, "search"],
        [/(?:link|shortcut)/, "link"],
    ]) {
        if (pattern.test(source)) return icon;
    }
    return SAFE_ICON_NAME.test(source) ? source : null;
}

export function riceTopbarSymbolicIconName(value) {
    const name = riceIconName(value);
    const monochrome = name?.endsWith("-color") ? name.slice(0, -6) : name;
    return ["brain", "clock"].includes(monochrome) || RICE_ICON_CATALOG.includes(monochrome)
        ? monochrome : null;
}

function iconAssetUrl(relative) {
    const url = new URL(relative, import.meta.url);
    const assetVersion = new URL(import.meta.url).searchParams.get("asset_version");
    if (assetVersion) url.searchParams.set("asset_version", assetVersion);
    return url;
}

function canLoadBrowserCatalog() {
    const protocol = new URL(import.meta.url).protocol;
    return typeof globalThis.fetch === "function" &&
        !(protocol === "file:" && globalThis.process?.versions?.node);
}

export function riceThemeRasterManifestLoadsFont(manifest) {
    return manifest?.font !== false;
}

let woff2DecoderPromise = null;

function loadWoff2Decoder() {
    if (woff2DecoderPromise) return woff2DecoderPromise;
    woff2DecoderPromise = new Promise((resolve, reject) => {
        if (!globalThis.document?.head) {
            reject(new Error("WOFF2 decoder requires a browser document"));
            return;
        }
        const previous = globalThis.Module;
        const module = {
            onRuntimeInitialized() {
                resolve(module);
            },
            printErr() {},
        };
        globalThis.Module = module;
        const script = globalThis.document.createElement("script");
        script.async = true;
        script.src = iconAssetUrl(RICE_ICON_WOFF2_DECODER_FILE).href;
        script.addEventListener("error", () => {
            globalThis.Module = previous;
            reject(new Error("RiceWM WOFF2 decoder could not be loaded"));
        }, { once: true });
        globalThis.document.head.append(script);
    });
    return woff2DecoderPromise;
}

function sfntTables(bytes) {
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const count = view.getUint16(4, false);
    if (count < 1 || count > 128 || 12 + count * 16 > bytes.byteLength) {
        throw new RangeError("invalid decompressed RiceWM font table directory");
    }
    const tables = new Map();
    for (let index = 0; index < count; index += 1) {
        const offset = 12 + index * 16;
        const tag = String.fromCharCode(...bytes.subarray(offset, offset + 4));
        const start = view.getUint32(offset + 8, false);
        const length = view.getUint32(offset + 12, false);
        if (start + length > bytes.byteLength) {
            throw new RangeError(`invalid ${tag} font table extent`);
        }
        tables.set(tag, { start, length });
    }
    return { view, tables };
}

function cmapGlyph(view, table, codepoint) {
    const subtables = [];
    const count = view.getUint16(table.start + 2, false);
    for (let index = 0; index < count; index += 1) {
        const record = table.start + 4 + index * 8;
        const platform = view.getUint16(record, false);
        const encoding = view.getUint16(record + 2, false);
        const start = table.start + view.getUint32(record + 4, false);
        if (view.getUint16(start, false) === 4) {
            subtables.push({ platform, encoding, start });
        }
    }
    subtables.sort((left, right) =>
        Number(right.platform === 3 && right.encoding === 1) -
        Number(left.platform === 3 && left.encoding === 1)
    );
    for (const subtable of subtables) {
        const segmentCount = view.getUint16(subtable.start + 6, false) / 2;
        const ends = subtable.start + 14;
        const starts = ends + segmentCount * 2 + 2;
        const deltas = starts + segmentCount * 2;
        const ranges = deltas + segmentCount * 2;
        for (let index = 0; index < segmentCount; index += 1) {
            const start = view.getUint16(starts + index * 2, false);
            const end = view.getUint16(ends + index * 2, false);
            if (codepoint < start || codepoint > end) continue;
            const delta = view.getUint16(deltas + index * 2, false);
            const range = view.getUint16(ranges + index * 2, false);
            if (range === 0) return (codepoint + delta) & 0xffff;
            const address = ranges + index * 2 + range + (codepoint - start) * 2;
            const glyph = view.getUint16(address, false);
            return glyph ? (glyph + delta) & 0xffff : 0;
        }
    }
    return 0;
}

function svgGlyphDocuments(bytes, glyphs, include = () => true) {
    const { view, tables } = sfntTables(bytes);
    const cmap = tables.get("cmap");
    const svg = tables.get("SVG ");
    if (!cmap || !svg) throw new Error("RiceWM font lacks cmap or SVG color data");
    const index = svg.start + view.getUint32(svg.start + 2, false);
    const count = view.getUint16(index, false);
    if (count > 512 || index + 2 + count * 12 > svg.start + svg.length) {
        throw new RangeError("invalid RiceWM SVG glyph index");
    }
    const records = [];
    for (let entry = 0; entry < count; entry += 1) {
        const offset = index + 2 + entry * 12;
        records.push({
            first: view.getUint16(offset, false),
            last: view.getUint16(offset + 2, false),
            start: index + view.getUint32(offset + 4, false),
            length: view.getUint32(offset + 8, false),
        });
    }
    const decoder = new TextDecoder("utf-8", { fatal: true });
    const documents = new Map();
    for (const [name, character] of glyphs) {
        if (!include(name)) continue;
        const codepoint = character.codePointAt(0);
        const glyph = cmapGlyph(view, cmap, codepoint);
        const record = records.find(candidate =>
            candidate.first <= glyph && glyph <= candidate.last
        );
        if (!record || record.length < 1 || record.length > 128 * 1024) continue;
        if (record.start + record.length > svg.start + svg.length) {
            throw new RangeError("invalid RiceWM SVG glyph extent");
        }
        const source = decoder.decode(bytes.subarray(
            record.start,
            record.start + record.length,
        ));
        if (
            !/<svg[\s>]/i.test(source) ||
            /<script|<foreignObject|javascript:|\son[a-z]+\s*=/i.test(source)
        ) continue;
        documents.set(name, source);
    }
    return documents;
}

const RICE_STANDARD_ICON_SEMANTICS = new Set([
    ...Object.keys(RICE_ICON_GLYPHS).map(name => name.replace(/-color$/, "")),
    ...Object.values(NATIVE_ICON_NAMES),
]);

export class RiceNativeIconRenderer {
    constructor(context, onDamage = () => {}, { loadColorGlyphs = true, monochrome = false } = {}) {
        this.context = context;
        this.loadColorGlyphs = loadColorGlyphs;
        this.monochrome = monochrome;
        this.onDamage = onDamage;
        this.fontReady = false;
        this.catalogReady = !canLoadBrowserCatalog();
        this.themeColor = null;
        this.themeHoverColor = null;
        this.themeHoverBackground = null;
        this.themeIconVariant = "monochrome";
        this.customThemeIcons = new Map();
        this.customThemeSignature = "";
        this.customThemeGeneration = 0;
        this.themeIconOverrides = new Map();
        this.themeFontSlug = "";
        this.themeFontFamily = "";
        this.themeFontReady = false;
        this.themeFont = null;
        this.themeFontGeneration = 0;
        this.themeGlyphs = new Map();
        this.themeColorGlyphImages = new Map();
        this.themeColorGlyphUrls = new Set();
        this.themeRasterIcons = new Map();
        this.themeRasterFallback = [];
        this.font = null;
        this.glyphs = new Map(Object.entries(RICE_ICON_GLYPHS));
        this.fallbackIcons = new Map();
        this.tintedIcons = new Map();
        this.notifiedFallbacks = new Set();
        this.colorGlyphImages = new Map();
        this.colorGlyphUrls = new Set();
        this.applicationIconImages = new Map();
        this.applicationIconUrls = new Set();
        this.applicationIconSignature = "";
        this.ready = Promise.all([this.#loadCatalog(),this.#loadFont()]);
    }

    /** Install bounded, full-colour application artwork supplied by the guest.
     *
     * Package icons are files in the composed Linux filesystem, not deployable
     * RiceWM assets.  The runtime validates and bounds them before they reach
     * this method; validate again at the browser boundary before creating blob
     * URLs.  Names use a private prefix so guest artwork cannot replace shell
     * controls or theme icons.
     */
    setApplicationIconAssets(assets) {
        const accepted = [];
        let totalBytes = 0;
        for (const asset of Array.isArray(assets) ? assets.slice(0, 512) : []) {
            const name = String(asset?.name || "");
            const bytes = asset?.bytes instanceof Uint8Array
                ? asset.bytes
                : new Uint8Array(asset?.bytes || 0);
            const png = asset?.mimeType === "image/png" &&
                bytes[0] === 0x89 && bytes[1] === 0x50 &&
                bytes[2] === 0x4e && bytes[3] === 0x47 &&
                bytes[4] === 0x0d && bytes[5] === 0x0a &&
                bytes[6] === 0x1a && bytes[7] === 0x0a;
            const jpeg = asset?.mimeType === "image/jpeg" &&
                bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff;
            const webp = asset?.mimeType === "image/webp" &&
                bytes[0] === 0x52 && bytes[1] === 0x49 &&
                bytes[2] === 0x46 && bytes[3] === 0x46 &&
                bytes[8] === 0x57 && bytes[9] === 0x45 &&
                bytes[10] === 0x42 && bytes[11] === 0x50;
            if (
                !RICE_GUEST_APPLICATION_ICON_NAME.test(name) ||
                bytes.byteLength < 8 || bytes.byteLength > 1024 * 1024 ||
                !(png || jpeg || webp)
            ) continue;
            totalBytes += bytes.byteLength;
            if (totalBytes > 8 * 1024 * 1024) break;
            accepted.push({ name, bytes, mimeType: asset.mimeType });
        }
        const signature = accepted.map(asset =>
            `${asset.name}:${asset.bytes.byteLength}`
        ).join("\n");
        if (signature === this.applicationIconSignature) return;

        for (const url of this.applicationIconUrls) {
            globalThis.URL?.revokeObjectURL?.(url);
        }
        this.applicationIconImages.clear();
        this.applicationIconUrls.clear();
        this.applicationIconSignature = signature;
        if (
            typeof globalThis.Image !== "function" ||
            typeof globalThis.Blob !== "function" ||
            typeof globalThis.URL?.createObjectURL !== "function"
        ) {
            this.onDamage();
            return;
        }
        for (const asset of accepted) {
            const url = globalThis.URL.createObjectURL(new Blob(
                [asset.bytes],
                { type: asset.mimeType },
            ));
            const image = new globalThis.Image();
            image.decoding = "async";
            image.addEventListener("load", () => this.onDamage(), { once: true });
            image.addEventListener("error", () => this.onDamage(), { once: true });
            image.src = url;
            this.applicationIconImages.set(asset.name, image);
            this.applicationIconUrls.add(url);
        }
        this.onDamage();
    }

    setThemeColor(color) {
        if (
            !Array.isArray(color) || color.length !== 4 ||
            color.some(channel => !Number.isInteger(channel) || channel < 0 || channel > 255)
        ) {
            this.themeColor = null;
            return;
        }
        this.themeColor = [...color];
    }

    setThemeHoverColor(color) {
        if (
            !Array.isArray(color) || color.length !== 4 ||
            color.some(channel => !Number.isInteger(channel) || channel < 0 || channel > 255)
        ) {
            this.themeHoverColor = null;
            return;
        }
        this.themeHoverColor = [...color];
    }

    setThemeHoverBackground(color) {
        if (
            !Array.isArray(color) || color.length !== 4 ||
            color.some(channel => !Number.isInteger(channel) || channel < 0 || channel > 255)
        ) {
            this.themeHoverBackground = null;
            return;
        }
        this.themeHoverBackground = [...color];
    }

    setThemeIcons(iconVariant, iconOverrides = {}, themeName = "", icons = {}) {
        const signature=JSON.stringify(icons);
        if(signature!==this.customThemeSignature) {
            const accepted=validateThemeIcons(icons);
            this.customThemeSignature=signature;this.customThemeGeneration++;
            this.customThemeIcons.clear();this.tintedIcons.clear();
            for(const [name,value] of Object.entries(accepted)) {
                if(typeof value==='string'){this.customThemeIcons.set(name,{alias:value});continue;}
                if(typeof Image==='undefined')continue;
                const image=new Image(),generation=this.customThemeGeneration;
                image.onload=()=>{if(generation===this.customThemeGeneration)this.onDamage();};
                image.src=value.png?'data:image/png;base64,'+value.png:'data:image/svg+xml;charset=utf-8,'+encodeURIComponent(value.svg.replace(/currentColor/gi,'#ffffff'));
                this.customThemeIcons.set(name,{image,monochrome:value.monochrome===true});
            }
        }
        this.themeIconVariant = iconVariant === "color" ? "color" : "monochrome";
        const entries = iconOverrides && typeof iconOverrides === "object"
            ? Object.entries(iconOverrides)
            : [];
        this.themeIconOverrides = new Map(entries.filter(([semantic, glyph]) =>
            SAFE_ICON_NAME.test(semantic) && SAFE_ICON_NAME.test(String(glyph))
        ).map(([semantic, glyph]) => [semantic, String(glyph)]));
        const slug = riceIconThemeSlug(themeName);
        if (slug !== this.themeFontSlug) this.#selectThemeFont(slug);
    }

    render(command) {
        if(command.scopedColor){const {scopedColor,...plain}=command;return this.renderWithColor(plain);}
        if (this.renderAvailable(command)) return true;
        this.context.fillStyle = this.#color(command.color);
        this.context.font =
            `${Math.max(10, Math.min(command.rect.height, 18))}px ` +
            '"RiceWM Sans", sans-serif';
        this.context.textAlign = "center";
        this.context.textBaseline = "middle";
        this.context.fillText(
            "□",
            command.rect.x + command.rect.width / 2,
            command.rect.y + command.rect.height / 2,
        );
        return false;
    }

    /** Caption state ink is explicit and independent from global toolbar ink. */
    renderNamedWithColor(command){const color=this.themeColor,hover=this.themeHoverColor;this.themeColor=null;this.themeHoverColor=null;try{return this.renderNamed(command);}finally{this.themeColor=color;this.themeHoverColor=hover;}}
    renderWithColor(command) {
        const color=this.themeColor,hover=this.themeHoverColor;
        this.themeColor=null;this.themeHoverColor=null;
        try{return this.renderAvailable(command);}finally{this.themeColor=color;this.themeHoverColor=hover;}
    }

    renderAvailable(command) {
        if(command.applicationId&&this.renderOverride('app:'+command.applicationId,command))return true;
        const semantic = riceNativeIconName(command.icon);
        if (semantic && this.#renderCustomThemeIcon(semantic,command))return true;
        if ((command.monochrome || (this.monochrome && !command.colorArtwork)) && semantic) {
            return this.#renderMonochrome(semantic, { ...command, monochrome: true });
        }
        if (semantic && this.#renderApplicationIcon(semantic, command)) return true;
        if (semantic && this.#renderThemeRaster(semantic, command)) return true;
        // The monochrome font contains only the paper silhouette for MIME
        // artwork. Its identifying letters live in the coloured SVG layers.
        if (RICE_FILE_ICON_SEMANTICS.has(semantic) && this.#renderColorGlyph(`${semantic}-color`, command)) return true;
        const themeName = this.#themeColorGlyphName(semantic) ||
            (RICE_OFFICE_ICON_NAMES.has(semantic) && this.themeGlyphs.has(this.themeIconOverrides.get(semantic) || semantic)
                ? (this.themeIconOverrides.get(semantic) || semantic) : null);
        if (themeName && this.#renderThemeGlyph(themeName, command)) return true;
        const name = this.#themedGlyphName(semantic);
        if (name?.endsWith("-color") && this.#renderColorGlyph(name, command)) {
            return true;
        }
        if (name && !this.glyphs.has(name) && semantic && name !== semantic) {
            if (this.#renderGlyph(semantic, command)) return true;
            if (this.#renderBuiltInVector(semantic, command)) return true;
            return this.catalogReady && this.#renderFallback(semantic, command);
        }
        if (this.#renderGlyph(name, command)) return true;
        if (!this.catalogReady || !name) return false;
        return this.#renderFallback(name, command);
    }

    renderNamed(command) {
        if(command.scopedColor){const {scopedColor,...plain}=command;return this.renderNamedWithColor(plain);}
        if(command.applicationId&&this.renderOverride('app:'+command.applicationId,command))return true;
        if(this.customThemeIcons.has(command.icon)&&this.#renderCustomThemeIcon(command.icon,command))return true;
        const semantic = riceIconName(command.icon);
        if (semantic && this.#renderCustomThemeIcon(semantic,command))return true;
        if ((command.monochrome || (this.monochrome && !command.colorArtwork)) && semantic) {
            return this.#renderMonochrome(semantic, { ...command, monochrome: true });
        }
        if (semantic && this.#renderApplicationIcon(semantic, command)) return true;
        if (semantic && this.#renderThemeRaster(semantic, command)) return true;
        if (RICE_FILE_ICON_SEMANTICS.has(semantic) && this.#renderColorGlyph(`${semantic}-color`, command)) return true;
        const themeName = this.#themeColorGlyphName(semantic) ||
            (RICE_OFFICE_ICON_NAMES.has(semantic) && this.themeGlyphs.has(this.themeIconOverrides.get(semantic) || semantic)
                ? (this.themeIconOverrides.get(semantic) || semantic) : null);
        if (themeName && this.#renderThemeGlyph(themeName, command)) return true;
        const name = this.#themedGlyphName(semantic);
        if (!name) return false;
        if (name.endsWith("-color") && this.#renderColorGlyph(name, command)) {
            return true;
        }
        if (this.glyphs.has(name)) return this.#renderGlyph(name, command);
        if (semantic && name !== semantic && this.glyphs.has(semantic)) {
            return this.#renderGlyph(semantic, command);
        }
        if (!this.catalogReady) return false;
        return this.#renderFallback(
            semantic && name !== semantic ? semantic : name,
            command,
        );
    }

    /** Render a base RiceWM icon without per-theme substitutions. */
    renderBuiltInNamed(command) {
        const semantic = riceIconName(command.icon);
        return semantic ? this.#renderCustomThemeIcon(semantic,command) || this.#renderGlyph(semantic, command) : false;
    }

    /** Draw the shell shortcut badge independently from the target icon.
     *
     * Keeping this as a second paint pass means every theme still supplies
     * the normal MIME/folder/application artwork.  The badge never aliases a
     * shortcut to a browser or generic hyperlink icon.
     */
    renderShortcutOverlay(command) {
        if (command?.shortcut !== true || !command.rect) return false;
        const { context } = this;
        const extent = Math.max(1, Math.min(command.rect.width, command.rect.height));
        const size = Math.max(7, Math.min(15, Math.round(extent * .34)));
        const x = Math.round(command.rect.x);
        const y = Math.round(command.rect.y + command.rect.height - size);
        const alpha = Number.isInteger(command.color?.[3])
            ? Math.max(0, Math.min(255, command.color[3])) / 255
            : 1;
        context.save?.();
        try {
            context.globalAlpha *= alpha;
            context.imageSmoothingEnabled = false;
            context.fillStyle = "#ffffff";
            context.fillRect(x, y, size, size);
            context.strokeStyle = "#000000";
            context.lineWidth = 1;
            context.strokeRect(x + .5, y + .5, size - 1, size - 1);

            // A compact, pixel-aligned bent arrow matching the conventional
            // desktop-shortcut mark at 16, 32, and 48 pixel icon sizes.
            const left = x + Math.max(2, Math.round(size * .2));
            const bottom = y + size - Math.max(2, Math.round(size * .2));
            const turnY = y + Math.max(3, Math.round(size * .48));
            const tipX = x + size - Math.max(2, Math.round(size * .16));
            const tipY = y + Math.max(2, Math.round(size * .16));
            context.beginPath();
            context.moveTo(left, bottom);
            context.lineTo(left, turnY);
            context.lineTo(tipX, turnY);
            context.lineTo(tipX, tipY);
            context.stroke();
            context.beginPath();
            context.moveTo(tipX, tipY);
            context.lineTo(tipX - Math.max(2, Math.round(size * .28)), tipY);
            context.moveTo(tipX, tipY);
            context.lineTo(tipX, tipY + Math.max(2, Math.round(size * .28)));
            context.stroke();
        } finally {
            context.restore?.();
        }
        return true;
    }

    glyph(name) {
        const semantic = riceIconName(name);
        const themeName = this.#themeColorGlyphName(semantic) ||
            (RICE_OFFICE_ICON_NAMES.has(semantic) && this.themeGlyphs.has(this.themeIconOverrides.get(semantic) || semantic)
                ? (this.themeIconOverrides.get(semantic) || semantic) : null);
        if (themeName) return this.themeGlyphs.get(themeName) || "";
        const themed = this.#themedGlyphName(semantic);
        return this.glyphs.get(themed) || this.glyphs.get(semantic) || "";
    }

    dispose() {
        this.customThemeGeneration++;this.customThemeIcons.clear();
        this.fontReady = false;
        this.themeColor = null;
        this.themeHoverColor = null;
        this.themeHoverBackground = null;
        this.themeIconVariant = "monochrome";
        this.themeIconOverrides.clear();
        this.themeFontGeneration += 1;
        this.themeFontSlug = "";
        this.themeFontFamily = "";
        this.themeFontReady = false;
        this.themeFont = null;
        this.themeGlyphs.clear();
        for (const url of this.themeColorGlyphUrls) globalThis.URL?.revokeObjectURL?.(url);
        this.themeColorGlyphImages.clear();
        this.themeColorGlyphUrls.clear();
        this.themeRasterIcons.clear();
        this.themeRasterFallback = [];
        this.font = null;
        this.glyphs.clear();
        this.fallbackIcons.clear();
        this.tintedIcons.clear();
        for (const url of this.colorGlyphUrls) globalThis.URL?.revokeObjectURL?.(url);
        this.colorGlyphImages.clear();
        this.colorGlyphUrls.clear();
        for (const url of this.applicationIconUrls) {
            globalThis.URL?.revokeObjectURL?.(url);
        }
        this.applicationIconImages.clear();
        this.applicationIconUrls.clear();
        this.applicationIconSignature = "";
    }

    #color(commandColor, hovered = false, commandHoverBackground = null, commandBackground = null) {
        const semanticInk = Array.isArray(commandColor) &&
            ((commandColor[0] === 255 && commandColor[1] === 255 && commandColor[2] === 255) ||
                (commandColor[0] === 10 && commandColor[1] === 8 && commandColor[2] === 13));
        if (semanticInk) return rgbaColor(commandColor);
        let themed = hovered && this.themeHoverColor
            ? this.themeHoverColor
            : this.themeColor;
        const background = Array.isArray(commandBackground)
            ? commandBackground
            : Array.isArray(commandHoverBackground)
                ? commandHoverBackground
                : this.themeHoverBackground;
        // An explicit surface applies to idle glyphs too (for example dark menus
        // in an otherwise light theme). Global hover defaults remain hover-only.
        if (themed && background && (hovered || Array.isArray(commandBackground))) {
            themed = riceContrastingIconColor(
                themed,
                background,
                commandColor,
            );
        }
        if (!themed) return rgbaColor(commandColor);
        const alpha = Array.isArray(commandColor) && Number.isInteger(commandColor[3])
            ? commandColor[3]
            : themed[3];
        return rgbaColor([
            themed[0],
            themed[1],
            themed[2],
            Math.min(themed[3], alpha),
        ]);
    }

    #themedGlyphName(semantic) {
        if (!semantic) return null;
        if (RICE_FORCED_MONOCHROME_SEMANTICS.has(semantic)) return semantic;
        const override = this.themeIconOverrides.get(semantic);
        if (override) return override;
        if (!semantic.endsWith("-color") && this.themeIconVariant === "color") {
            return `${semantic}-color`;
        }
        return semantic;
    }

    #themeColorGlyphName(semantic) {
        if (
            !semantic ||
            !RICE_STANDARD_ICON_SEMANTICS.has(semantic)
        ) return null;
        const override = this.themeIconOverrides.get(semantic);
        const candidates = [
            override,
            ...(this.themeIconVariant === "color"
                ? [semantic.endsWith("-color") ? semantic : `${semantic}-color`]
                : []),
        ];
        return candidates.find(name =>
            typeof name === "string" &&
            name.endsWith("-color") &&
            this.themeGlyphs.has(name)
        ) || null;
    }

    #renderThemeGlyph(name, command) {
        const image = this.themeColorGlyphImages.get(name);
        if (image?.complete && image.naturalWidth > 0) {
            return this.#renderRawImage(image, command);
        }
        const glyph = this.themeGlyphs.get(name);
        if (!glyph || !this.themeFontReady || !this.themeFontFamily) return false;
        const { context } = this;
        const { rect } = command;
        const size = Math.max(1, Math.min(rect.width, rect.height));
        context.save?.();
        try {
            context.font = `normal ${size}px "${this.themeFontFamily}"`;
            context.textAlign = "center";
            context.textBaseline = "middle";
            context.fillText(
                glyph,
                rect.x + rect.width / 2,
                rect.y + rect.height / 2,
                Math.max(1, rect.width),
            );
        } finally {
            context.restore?.();
        }
        return true;
    }

    #renderThemeRaster(semantic, command) {
        const candidates = this.themeRasterIcons.get(semantic) ||
            (RICE_THEME_RASTER_NO_FALLBACK.has(semantic) ? [] : this.themeRasterFallback);
        if (!Array.isArray(candidates) || candidates.length === 0) return false;
        const target = Math.max(1, Math.min(command.rect.width, command.rect.height));
        const ordered = [...candidates].sort((left, right) =>
            Math.abs(left.size - target) - Math.abs(right.size - target)
        );
        const selected = ordered.find(candidate =>
            candidate.image?.complete && candidate.image.naturalWidth > 0
        );
        if (!selected) return false;
        const size = Math.min(
            selected.size,
            Math.max(1, command.rect.width),
            Math.max(1, command.rect.height),
        );
        const x = Math.round(command.rect.x + (command.rect.width - size) / 2);
        const y = Math.round(command.rect.y + (command.rect.height - size) / 2);
        this.context.save?.();
        try {
            this.context.imageSmoothingEnabled = false;
            // Retain semantic opacity for native-size color artwork. Hidden
            // files carry MUTED ink from the retained UI just like their
            // labels; ignoring it here made only the raster icon fully opaque.
            const alpha = Array.isArray(command.color) &&
                Number.isFinite(command.color[3])
                ? Math.max(0, Math.min(255, command.color[3])) / 255
                : 1;
            this.context.globalAlpha *= alpha;
            // Classic resources use uneven transparent margins. In a 48px
            // icon cell, compare their visible pixels instead of their file
            // dimensions: My Documents otherwise looks much smaller than a
            // Desktop icon rendered into the same native UI rectangle.
            const normalizeLarge = this.themeFontSlug === "redmond-98" &&
                target === 48 && selected.size === 48 &&
                selected.image.src?.split("?", 1)[0].endsWith(".png");
            if (normalizeLarge && selected.visibleBounds === undefined) {
                try {
                    const surface = typeof globalThis.OffscreenCanvas === "function"
                        ? new globalThis.OffscreenCanvas(48, 48)
                        : globalThis.document?.createElement?.("canvas");
                    if (surface) {
                        surface.width = 48;
                        surface.height = 48;
                        const scan = surface.getContext("2d", { willReadFrequently: true });
                        scan.drawImage(selected.image, 0, 0, 48, 48);
                        const pixels = scan.getImageData(0, 0, 48, 48).data;
                        let left = 48, top = 48, right = -1, bottom = -1;
                        for (let row = 0; row < 48; row += 1) {
                            for (let column = 0; column < 48; column += 1) {
                                if (pixels[(row * 48 + column) * 4 + 3] <= 16) continue;
                                left = Math.min(left, column);
                                top = Math.min(top, row);
                                right = Math.max(right, column);
                                bottom = Math.max(bottom, row);
                            }
                        }
                        selected.visibleBounds = right >= left
                            ? { left, top, width: right - left + 1, height: bottom - top + 1 }
                            : null;
                    }
                } catch {
                    selected.visibleBounds = null;
                }
            }
            const bounds = normalizeLarge && selected.visibleBounds;
            if (bounds) {
                const scale = 40 / Math.max(bounds.width, bounds.height);
                const width = Math.round(bounds.width * scale);
                const height = Math.round(bounds.height * scale);
                this.context.drawImage(selected.image,
                    bounds.left, bounds.top, bounds.width, bounds.height,
                    Math.round(command.rect.x + (command.rect.width - width) / 2),
                    Math.round(command.rect.y + (command.rect.height - height) / 2),
                    width, height);
            } else {
                this.context.drawImage(selected.image, x, y, size, size);
            }
        } finally {
            this.context.restore?.();
        }
        return true;
    }

    setApplicationSelectorAliases(entries=[]){
        this.applicationSelectorAliases=new Map();
        for(const entry of entries){
            const names=[...new Set([entry.id,entry.riceAppId,entry.nativeEntrypoint?'native:'+entry.nativeEntrypoint:null].filter(Boolean))];
            for(const name of names){const aliases=this.applicationSelectorAliases.get(name)||new Set();for(const alias of names)if(alias!==name)aliases.add(alias);this.applicationSelectorAliases.set(name,aliases);}
        }
    }
    hasOverride(value){const name=typeof value==='number'?riceNativeIconName(value):value;return this.customThemeIcons.has(name)||(name==='trash'&&this.customThemeIcons.has('trash-empty'));}
    renderOverride(selector,command){
        const choices=[selector];if(selector?.startsWith('app:')){for(const alias of this.applicationSelectorAliases?.get(selector.slice(4))||[])choices.push('app:'+alias);const id=selector.slice(4);if(id.endsWith('.desktop'))choices.push('app:'+id.slice(0,-8));if(id.startsWith('dev.')||id.startsWith('org.ricewm.'))choices.push('app:riceapp:'+id);if(!id.includes(':'))choices.push('app:desktop:'+id);}if(selector?.startsWith('mime:'))choices.push(selector.split('/')[0]+'/*');
        for(const key of choices)if(this.#renderCustomThemeIcon(key,command))return true;
        return false;
    }
    #renderCustomThemeIcon(name,command,seen=new Set()) {
        if(seen.has(name))return false;seen.add(name);
        const entry=this.customThemeIcons.get(name)||this.customThemeIcons.get(name.replace(/-color$/,''))||(name==='trash'?this.customThemeIcons.get('trash-empty'):null);
        if(!entry)return false;
        if(entry.alias)return this.#renderCustomThemeIcon(entry.alias,command,seen)||this.#renderGlyph(entry.alias,command)||this.#renderBuiltInVector(entry.alias,command);
        if(!entry.image?.complete||!entry.image.naturalWidth)return false;
        if(entry.monochrome||command.monochrome)return this.#renderTintedImage('custom:'+this.customThemeGeneration+':'+name,entry.image,command);
        const {x,y,width,height}=command.rect;
        const scale=Math.min(width/entry.image.naturalWidth,height/entry.image.naturalHeight),w=entry.image.naturalWidth*scale,h=entry.image.naturalHeight*scale;
        this.context.drawImage(entry.image,x+(width-w)/2,y+(height-h)/2,w,h);return true;
    }
    #renderMonochrome(name, command) {
        const semantic = name.replace(/-color$/, "");
        // MIME artwork carries identifying letters absent from the font's
        // paper silhouette. Remove its hue while retaining those details.
        if (RICE_FILE_ICON_SEMANTICS.has(semantic) &&
            this.#renderColorGlyph(`${semantic}-color`, command)) return true;
        // Guest artwork has no font glyph. Tint it rather than losing the icon.
        const image = this.applicationIconImages.get(name);
        if (image?.complete && image.naturalWidth > 0) {
            return this.#renderTintedImage(name, image, command);
        }
        if (semantic === "grain" && this.#renderBuiltInVector(semantic, command)) return true;
        return this.#renderGlyph(semantic, command) ||
            this.#renderBuiltInVector(semantic, command) ||
            (this.catalogReady && this.#renderFallback(name.endsWith("-color") &&
                !this.glyphs.has(semantic) ? name : semantic, command));
    }

    #renderGlyph(name, command) {
        if (name === "clock") return this.#renderBuiltInVector(name, command);
        if (name === "inbox") {
            return this.#renderFallback("rice-box", command) ||
                this.#renderBuiltInVector("inbox", command);
        }
        if (this.#renderNavigationOverride(name, command)) return true;
        // Computer is distinct from Desktop for theme artwork; the base font's
        // monitor glyph remains its fallback in packs without a computer icon.
        const glyph = name ? this.glyphs.get(name === "computer" ? "desktop" : name) : null;
        if (!glyph || !this.fontReady) return false;
        const { context } = this;
        const { rect } = command;
        const size = Math.max(1, Math.min(rect.width, rect.height));
        context.save?.();
        try {
            context.fillStyle = this.#color(
                command.color,
                command.hovered === true,
                command.hoverBackground, command.background,
            );
            context.font = `normal ${size}px "${RICE_ICON_FONT_FAMILY}"`;
            context.textAlign = "center";
            context.textBaseline = "middle";
            context.fillText(
                glyph,
                rect.x + rect.width / 2,
                rect.y + rect.height / 2,
                Math.max(1, rect.width),
            );
        } finally {
            context.restore?.();
        }
        return true;
    }

    #renderColorGlyph(name, command) {
        if (this.#renderNavigationOverride(name, command)) return true;
        const image = this.colorGlyphImages.get(name);
        if (!image?.complete || image.naturalWidth <= 0) return false;
        const width = Math.max(1, command.rect.width);
        const height = Math.max(1, command.rect.height);
        const scale = Math.min(width / image.naturalWidth, height / image.naturalHeight);
        const fittedWidth = Math.max(1, image.naturalWidth * scale);
        const fittedHeight = Math.max(1, image.naturalHeight * scale);
        this.context.save?.();
        try {
            const alpha = Number.isInteger(command.color?.[3])
                ? command.color[3] / 255
                : 1;
            this.context.globalAlpha = (Number(this.context.globalAlpha) || 1) * alpha;
            if (command.monochrome) this.context.filter = "grayscale(1)";
            this.context.drawImage(
                image,
                command.rect.x + (width - fittedWidth) / 2,
                command.rect.y + (height - fittedHeight) / 2,
                fittedWidth,
                fittedHeight,
            );
        } finally {
            this.context.restore?.();
        }
        return true;
    }

    #renderApplicationIcon(name, command) {
        const image = this.applicationIconImages.get(name);
        if (!image?.complete || image.naturalWidth <= 0) return false;
        const alpha = Number.isInteger(command.color?.[3])
            ? Math.max(0, Math.min(255, command.color[3])) / 255
            : 1;
        this.context.save?.();
        try {
            this.context.globalAlpha *= alpha;
            return this.#renderRawImage(image, command);
        } finally {
            this.context.restore?.();
        }
    }

    #renderFallback(name, command) {
        if (this.#renderBuiltInVector(name, command)) return true;
        const image = this.#fallbackIcon(name);
        if (!image?.complete || image.naturalWidth <= 0) return false;
        if (!command.monochrome && !RICE_OFFICE_ICON_NAMES.has(name) && (name.endsWith("-color") || RICE_RAW_SVG_SEMANTICS.has(name))) {
            return this.#renderRawImage(image, command);
        }
        return this.#renderTintedImage(name, image, command);
    }

    // Shared navigation artwork replaces the old font silhouettes. Explicit
    // theme artwork still wins; these vectors use the current toolbar ink.
    #renderNavigationOverride(name, command) {
        const semantic = name?.replace(/-color$/, "");
        if (semantic !== "navigate-up" && semantic !== "view-refresh") return false;
        const { context: c } = this;
        const { rect } = command;
        const size = Math.max(1, Math.min(rect.width, rect.height));
        c.save();
        try {
            c.translate(rect.x + (rect.width - size) / 2, rect.y + (rect.height - size) / 2);
            c.scale(size / 24, size / 24);
            c.strokeStyle = this.#color(command.color, command.hovered === true, command.hoverBackground, command.background);
            c.lineWidth = 1.8;
            c.lineCap = "round";
            c.lineJoin = "round";
            c.beginPath();
            if (semantic === "view-refresh") {
                // One open clockwise sweep, with an unambiguous arrowhead.
                c.arc(12, 12, 7.5, Math.PI * .2, Math.PI * 1.85);
                const tipX = 12 + 7.5 * Math.cos(Math.PI * 1.85);
                const tipY = 12 + 7.5 * Math.sin(Math.PI * 1.85);
                c.moveTo(tipX, tipY - 5);
                c.lineTo(tipX, tipY);
                c.lineTo(tipX - 5, tipY);
            } else {
                // A folder tab supplies context; the inset arrow means parent.
                c.moveTo(10, 6.5);
                c.lineTo(8, 4.5);
                c.lineTo(3, 4.5);
                c.lineTo(3, 19.5);
                c.lineTo(21, 19.5);
                c.lineTo(21, 6.5);
                c.lineTo(10, 6.5);
                c.moveTo(12, 16.5);
                c.lineTo(12, 9.5);
                c.moveTo(8.5, 13);
                c.lineTo(12, 9.5);
                c.lineTo(15.5, 13);
            }
            c.stroke();
        } finally {
            c.restore();
        }
        return true;
    }

    #renderBuiltInVector(name, command) {
        if (name === "clock") {
            const c = this.context, r = command.rect, size = Math.min(r.width, r.height);
            c.save();
            c.translate(r.x + (r.width - size) / 2, r.y + (r.height - size) / 2);
            c.scale(size / 24, size / 24);
            c.strokeStyle = this.#color(command.color, command.hovered === true,
                command.hoverBackground, command.background);
            c.lineWidth = 1.8; c.lineCap = "round"; c.lineJoin = "round";
            c.beginPath(); c.arc(12, 12, 9, 0, Math.PI * 2);
            c.moveTo(12, 6); c.lineTo(12, 12); c.lineTo(16, 14);
            c.stroke(); c.restore();
            return true;
        }
        if (name === "inbox") {
            const c = this.context, r = command.rect;
            const size = Math.min(r.width, r.height);
            c.save();
            c.translate(r.x + (r.width - size) / 2, r.y + (r.height - size) / 2);
            c.scale(size / 24, size / 24);
            c.strokeStyle = this.#color(command.color, command.hovered === true,
                command.hoverBackground, command.background);
            c.lineWidth = 1.6; c.lineCap = "round"; c.lineJoin = "round";
            c.beginPath(); c.moveTo(3, 5); c.lineTo(21, 5); c.lineTo(21, 19);
            c.lineTo(3, 19); c.lineTo(3, 5); c.moveTo(3, 6); c.lineTo(12, 13);
            c.lineTo(21, 6); c.stroke(); c.restore();
            return true;
        }
        if (name === "grain" && this.context.ellipse) {
            const c = this.context, r = command.rect;
            const size = Math.min(r.width, r.height);
            c.save();
            c.translate(r.x + (r.width - size) / 2, r.y + (r.height - size) / 2);
            c.scale(size / 16, size / 16);
            c.strokeStyle = c.fillStyle = this.#color(command.color, command.hovered === true,
                command.hoverBackground, command.background);
            c.lineWidth = 1.25;
            c.lineCap = "round";
            c.beginPath();
            c.moveTo(7, 14);
            c.bezierCurveTo(6.6, 10.5, 7.7, 5.5, 9, 2);
            c.moveTo(7, 12);
            c.quadraticCurveTo(4.5, 10.3, 3, 9.4);
            c.moveTo(7.4, 10.2);
            c.quadraticCurveTo(10.3, 8.9, 12, 7.7);
            c.stroke();
            for (const [x, y, angle] of [[6.3, 4.1, -.7], [10.4, 4.9, .7],
                [5.3, 6.5, -.7], [10.4, 7.2, .7], [4.7, 9, -.7], [9.5, 9.6, .7]]) {
                c.beginPath();
                c.ellipse(x, y, 1.35, .66, angle, 0, Math.PI * 2);
                c.fill();
            }
            c.restore();
            return true;
        }
        if (name === "brain" && this.context.bezierCurveTo) {
            const c=this.context,r=command.rect,s=Math.min(r.width,r.height)/24;
            c.save();c.translate(r.x+(r.width-24*s)/2,r.y+(r.height-24*s)/2);c.scale(s,s);
            c.strokeStyle=this.#color(command.color,command.hovered===true,command.hoverBackground,command.background);
            c.lineWidth=1.7;c.lineCap='round';c.lineJoin='round';
            for (const side of [1,-1]) {
                c.save();if(side===-1){c.translate(24,0);c.scale(-1,1);}
                c.beginPath();c.moveTo(12,5);c.bezierCurveTo(12,1,6,1,6,5);
                c.bezierCurveTo(3,5,2,9,4,11);c.bezierCurveTo(1,13,2,17,5,18);
                c.bezierCurveTo(4,22,10,23,12,19);
                c.moveTo(6,5);c.bezierCurveTo(6,8,8,9,9,9);
                c.moveTo(4,11);c.bezierCurveTo(5,10,7,11,7,13);
                c.moveTo(5,18);c.bezierCurveTo(8,18,9,16,9,15);
                c.stroke();c.restore();
            }
            c.beginPath();c.moveTo(12,5);c.lineTo(12,19);c.stroke();c.restore();return true;
        }
        if ((name === "mail-reply" || name === "mail-forward") && this.context.bezierCurveTo) {
            const c=this.context,r=command.rect,s=Math.min(r.width,r.height)/24;
            c.save();c.translate(r.x,r.y);c.scale(s,s);
            if(name === "mail-forward") { c.translate(24,0);c.scale(-1,1); }
            c.strokeStyle=this.#color(command.color,command.hovered===true,command.hoverBackground, command.background);
            c.lineWidth=1.8;c.lineCap='round';c.lineJoin='round';c.beginPath();
            c.moveTo(10,5);c.lineTo(3,11);c.lineTo(10,17);c.moveTo(3,11);
            c.lineTo(12,11);c.bezierCurveTo(18,11,21,14,21,20);
            c.stroke();c.restore();return true;
        }
        if (name === "attachment" && this.context.bezierCurveTo) {
            const c=this.context,r=command.rect,s=Math.min(r.width,r.height)/24;
            c.save();c.translate(r.x,r.y);c.scale(s,s);c.strokeStyle=this.#color(command.color,command.hovered===true,command.hoverBackground, command.background);
            c.lineWidth=1.8;c.lineCap='round';c.lineJoin='round';c.beginPath();
            c.moveTo(8,13);c.lineTo(15,6);c.bezierCurveTo(18,3,22,7,19,10);c.lineTo(10,19);
            c.bezierCurveTo(5,24,-1,18,4,13);c.lineTo(13,4);c.bezierCurveTo(18,-1,25,6,20,11);
            c.stroke();c.restore();return true;
        }

        if (name === "power" && this.context.beginPath) {
            const { context } = this;
            const { rect } = command;
            const size = Math.max(1, Math.min(rect.width, rect.height));
            const centerX = rect.x + rect.width / 2;
            const centerY = rect.y + rect.height / 2 + size * .04;
            const radius = size * .32;
            context.save?.();
            try {
                context.strokeStyle = this.#color(
                    command.color,
                    command.hovered === true,
                    command.hoverBackground, command.background,
                );
                context.lineWidth = Math.max(1.5, size * .09);
                context.lineCap = "round";
                context.beginPath();
                context.arc(centerX, centerY, radius, -Math.PI * .25, Math.PI * 1.25);
                context.stroke();
                context.beginPath();
                context.moveTo(centerX, rect.y + size * .12);
                context.lineTo(centerX, centerY - size * .02);
                context.stroke();
            } finally {
                context.restore?.();
            }
            return true;
        }
        if (/^format-(align-(left|center|right|justify)|bullets|numbering)$/.test(name)) {
            const {context} = this, {rect} = command;
            context.save?.();
            try {
                context.strokeStyle = this.#color(command.color, command.hovered === true, command.hoverBackground, command.background);
                context.fillStyle = context.strokeStyle;
                context.lineWidth = Math.max(1, rect.height / 12);
                for (let row = 0; row < 4; row++) {
                    const list = name === "format-bullets" || name === "format-numbering";
                    const width = rect.width * (list ? .58 : row % 2 && name !== "format-align-justify" ? .48 : .8);
                    const left = rect.x + (list ? rect.width * .34 : name === "format-align-right" ? rect.width * .9 - width : name === "format-align-center" ? (rect.width - width) / 2 : rect.width * .1);
                    const y = rect.y + rect.height * (.18 + row * .21);
                    context.beginPath(); context.moveTo(left, y); context.lineTo(left + width, y); context.stroke();
                    if (name === "format-bullets") context.fillRect(rect.x + rect.width * .08, y - 1, 2, 2);
                    if (name === "format-numbering") { context.font = `${Math.max(5, rect.height * .2)}px sans-serif`; context.textBaseline = "middle"; context.textAlign = "left"; context.fillText(String(row + 1), rect.x, y); }
                }
            } finally { context.restore?.(); }
            return true;
        }
        if (name === "format-font-smaller" || name === "format-font-larger") {
            const {context} = this, {rect} = command;
            context.save?.();
            try {
                context.fillStyle = this.#color(command.color, command.hovered === true, command.hoverBackground, command.background);
                context.font = `600 ${rect.height * .8}px sans-serif`;
                context.textAlign = "left"; context.textBaseline = "middle";
                context.fillText("A", rect.x, rect.y + rect.height / 2);
                context.font = `600 ${rect.height * .55}px sans-serif`;
                context.fillText(name === "format-font-larger" ? "+" : "−", rect.x + rect.width * .62, rect.y + rect.height * .3);
            } finally { context.restore?.(); }
            return true;
        }
        const format = {
            "format-bold": ["B", "700", "normal", false],
            "format-italic": ["I", "500", "italic", false],
            "format-underline": ["U", "500", "normal", true],
            "format-strike": ["S", "500", "normal", true],
        }[name];
        if (format) {
            const [letter, weight, style, line] = format;
            const { context } = this;
            const { rect } = command;
            const size = Math.max(10, Math.min(rect.width, rect.height) * .78);
            context.save?.();
            try {
                context.fillStyle = this.#color(
                    command.color,
                    command.hovered === true,
                    command.hoverBackground, command.background,
                );
                context.strokeStyle = context.fillStyle;
                context.lineWidth = Math.max(1, size / 11);
                context.font = `${style} ${weight} ${size}px "RiceWM Sans", sans-serif`;
                context.textAlign = "center";
                context.textBaseline = "middle";
                context.fillText(
                    letter,
                    rect.x + rect.width / 2,
                    rect.y + rect.height / 2,
                    Math.max(1, rect.width),
                );
                if (line && context.beginPath) {
                    const y = name === "format-underline"
                        ? rect.y + rect.height * .82
                        : rect.y + rect.height * .52;
                    context.beginPath();
                    context.moveTo(rect.x + rect.width * .24, y);
                    context.lineTo(rect.x + rect.width * .76, y);
                    context.stroke();
                }
            } finally {
                context.restore?.();
            }
            return true;
        }
        if ((name === "sun" || name === "briefcase") && this.context.beginPath) {
            const {context}=this,{rect}=command;const cx=rect.x+rect.width/2,cy=rect.y+rect.height/2,s=Math.min(rect.width,rect.height);
            context.save?.();
            try {
                context.strokeStyle=this.#color(command.color,command.hovered===true,command.hoverBackground,command.background);context.lineWidth=Math.max(1.2,s/12);context.lineJoin='round';context.beginPath();
                if(name==='sun'){
                    context.arc(cx,cy,s*.22,0,Math.PI*2);
                    for(let i=0;i<8;i++){const a=i*Math.PI/4;context.moveTo(cx+Math.cos(a)*s*.32,cy+Math.sin(a)*s*.32);context.lineTo(cx+Math.cos(a)*s*.44,cy+Math.sin(a)*s*.44);}
                }else{
                    context.rect(rect.x+s*.12,rect.y+s*.32,s*.76,s*.54);context.moveTo(rect.x+s*.36,rect.y+s*.32);context.lineTo(rect.x+s*.36,rect.y+s*.16);context.lineTo(rect.x+s*.64,rect.y+s*.16);context.lineTo(rect.x+s*.64,rect.y+s*.32);context.moveTo(rect.x+s*.12,cy+s*.08);context.lineTo(rect.x+s*.88,cy+s*.08);
                }
                context.stroke();
            } finally {context.restore?.();}
            return true;
        }
        if ((name === "star" || name === "star-outline") && this.context.beginPath) {
            const { context } = this;
            const { rect } = command;
            const centerX = rect.x + rect.width / 2;
            const centerY = rect.y + rect.height / 2;
            const outer = Math.max(2, Math.min(rect.width, rect.height) * .38);
            const inner = outer * .45;
            context.save?.();
            try {
                context.strokeStyle = this.#color(
                    command.color,
                    command.hovered === true,
                    command.hoverBackground, command.background,
                );
                context.lineWidth = Math.max(1.3, outer / 6);
                context.lineJoin = "round";
                context.beginPath();
                let first = null;
                for (let point = 0; point < 10; point += 1) {
                    const radius = point % 2 === 0 ? outer : inner;
                    const angle = -Math.PI / 2 + point * Math.PI / 5;
                    const x = centerX + Math.cos(angle) * radius;
                    const y = centerY + Math.sin(angle) * radius;
                    if (point === 0) {
                        first = [x, y];
                        context.moveTo(x, y);
                    } else context.lineTo(x, y);
                }
                context.lineTo(first[0], first[1]);
                context.stroke();
            } finally {
                context.restore?.();
            }
            return true;
        }
        if (!matchesMailIcon(name) || !this.context.beginPath) return false;
        const { context } = this;
        const { rect } = command;
        const inset = Math.max(1, Math.min(rect.width, rect.height) * .16);
        const left = rect.x + inset;
        const right = rect.x + rect.width - inset;
        const top = rect.y + rect.height * .27;
        const bottom = rect.y + rect.height * .75;
        context.save?.();
        try {
            context.strokeStyle = this.#color(
                command.color,
                command.hovered === true,
                command.hoverBackground, command.background,
            );
            context.fillStyle = context.strokeStyle;
            context.lineWidth = Math.max(1.4, Math.min(rect.width, rect.height) / 11);
            context.lineJoin = "round";
            context.beginPath();
            context.rect(left, top, right - left, bottom - top);
            context.moveTo(left, top);
            context.lineTo((left + right) / 2, rect.y + rect.height * .53);
            context.lineTo(right, top);
            context.stroke();
            if (name === "mail-unread") {
                context.beginPath();
                context.arc(right - inset * .05, top + inset * .05, Math.max(2, inset * .34), 0, Math.PI * 2);
                context.fill();
            }
        } finally {
            context.restore?.();
        }
        return true;
    }

    #renderRawImage(image, command) {
        const width = Math.max(1, command.rect.width);
        const height = Math.max(1, command.rect.height);
        const scale = Math.min(width / image.naturalWidth, height / image.naturalHeight);
        const fittedWidth = Math.max(1, image.naturalWidth * scale);
        const fittedHeight = Math.max(1, image.naturalHeight * scale);
        this.context.drawImage(
            image,
            command.rect.x + (width - fittedWidth) / 2,
            command.rect.y + (height - fittedHeight) / 2,
            fittedWidth,
            fittedHeight,
        );
        return true;
    }

    #renderTintedImage(name, image, command) {
        const width = Math.max(1, command.rect.width);
        const height = Math.max(1, command.rect.height);
        const color = this.#color(
            command.color,
            command.hovered === true,
            command.hoverBackground, command.background,
        );
        const key = `${name}:${width}:${height}:${color}`;
        let surface = this.tintedIcons.get(key);
        if (!surface) {
            const offscreen = typeof globalThis.OffscreenCanvas === "function";
            surface = offscreen
                ? new globalThis.OffscreenCanvas(width, height)
                : globalThis.document?.createElement?.("canvas");
            if (surface && !offscreen) {
                surface.width = width;
                surface.height = height;
            }
            const context = surface?.getContext?.("2d");
            if (!context) return false;
            const scale = Math.min(width / image.naturalWidth, height / image.naturalHeight);
            const fittedWidth = Math.max(1, image.naturalWidth * scale);
            const fittedHeight = Math.max(1, image.naturalHeight * scale);
            context.drawImage(
                image,
                (width - fittedWidth) / 2,
                (height - fittedHeight) / 2,
                fittedWidth,
                fittedHeight,
            );
            context.globalCompositeOperation = "source-in";
            context.fillStyle = color;
            context.fillRect(0, 0, width, height);
            context.globalCompositeOperation = "source-over";
            if (this.tintedIcons.size >= 128) this.tintedIcons.clear();
            this.tintedIcons.set(key, surface);
        }
        this.context.drawImage(surface, command.rect.x, command.rect.y, width, height);
        return true;
    }

    #fallbackIcon(name) {
        if (this.fallbackIcons.has(name)) return this.fallbackIcons.get(name);
        if (typeof globalThis.Image !== "function") return null;
        const image = new globalThis.Image();
        image.addEventListener("load", () => this.onDamage(), { once: true });
        image.addEventListener("error", () => this.onDamage(), { once: true });
        image.src = iconAssetUrl(`${RICE_NEW_ICON_DIRECTORY}/${name}.svg`).href;
        this.fallbackIcons.set(name, image);
        if (name !== "rice-box" && !this.notifiedFallbacks.has(name)) {
            this.notifiedFallbacks.add(name);
            globalThis.console?.warn?.(
                `RiceWM icon '${name}' is not mapped in ricewm.css; ` +
                `trying new-icons/${name}.svg`,
            );
            if (typeof globalThis.CustomEvent === "function") {
                globalThis.dispatchEvent?.(new globalThis.CustomEvent(
                    "ricewm-new-icon",
                    { detail: Object.freeze({ name }) },
                ));
            }
        }
        return image;
    }

    #loadCatalog() {
        if (!canLoadBrowserCatalog()) return;
        return loadRiceIconCatalog().then(glyphs => {
            this.glyphs = glyphs;
            if (this.loadColorGlyphs) return this.#loadColorGlyphs(glyphs);
        }).catch(error => {
            globalThis.console?.error?.("RiceWM icon catalog could not be loaded", error);
        }).finally(() => {
            this.catalogReady = true;
            this.onDamage();
        });
    }

    async #loadColorGlyphs(glyphs) {
        const [module, response] = await Promise.all([
            loadWoff2Decoder(),
            globalThis.fetch(iconAssetUrl(RICE_ICON_FONT_FILE)),
        ]);
        if (!response.ok) throw new Error(`icon font request failed: ${response.status}`);
        const compressed = new Uint8Array(await response.arrayBuffer());
        if (compressed.byteLength < 4 || compressed.byteLength > 4 * 1024 * 1024) {
            throw new RangeError("RiceWM icon font is outside its byte limit");
        }
        const decompressed = module.decompress(compressed);
        if (!(decompressed instanceof Uint8Array) || decompressed.byteLength > 8 * 1024 * 1024) {
            throw new RangeError("RiceWM icon font decompression failed");
        }
        for (const [name, source] of svgGlyphDocuments(
            decompressed,
            glyphs,
            candidate => candidate.endsWith("-color") &&
                RICE_OT_SVG_COLOR_SEMANTICS.has(candidate.slice(0, -6)),
        )) {
            const blob = new Blob([source], { type: "image/svg+xml" });
            const url = globalThis.URL.createObjectURL(blob);
            const image = new globalThis.Image();
            image.addEventListener("load", () => this.onDamage(), { once: true });
            image.addEventListener("error", () => this.onDamage(), { once: true });
            image.src = url;
            this.colorGlyphImages.set(name, image);
            this.colorGlyphUrls.add(url);
        }
    }

    #selectThemeFont(slug) {
        const generation = ++this.themeFontGeneration;
        this.themeFontSlug = slug;
        this.themeFontFamily = "";
        this.themeFontReady = false;
        this.themeFont = null;
        this.themeGlyphs.clear();
        for (const url of this.themeColorGlyphUrls) globalThis.URL?.revokeObjectURL?.(url);
        this.themeColorGlyphImages.clear();
        this.themeColorGlyphUrls.clear();
        this.themeRasterIcons.clear();
        this.themeRasterFallback = [];
        if (!slug || !canLoadBrowserCatalog()) return;
        this.#loadThemeRasterIcons(slug, generation).catch(error => {
            if (generation !== this.themeFontGeneration) return false;
            globalThis.console?.error?.(
                `RiceWM theme image icons '${slug}' could not be loaded`,
                error,
            );
            return true;
        }).then(loadFont => {
            if (!loadFont || generation !== this.themeFontGeneration) return;
            return this.#loadThemeFont(slug, generation).catch(error => {
                if (generation !== this.themeFontGeneration) return;
                globalThis.console?.error?.(
                    `RiceWM theme icon font '${slug}' could not be loaded`,
                    error,
                );
            });
        }).finally(() => {
            if (generation === this.themeFontGeneration) this.onDamage();
        });
    }

    async #loadThemeRasterIcons(slug, generation) {
        const directory = `${RICE_THEME_ICON_DIRECTORY}/${slug}`;
        const response = await globalThis.fetch(iconAssetUrl(`${directory}/index.json`));
        if (generation !== this.themeFontGeneration) return false;
        // A theme without an image manifest may still be a legacy icon-font
        // pack, so retain the font fallback in that case.
        if (response.status === 404) return true;
        if (!response.ok) throw new Error(`theme image manifest request failed: ${response.status}`);
        const source = await response.text();
        if (source.length > 64 * 1024) throw new RangeError("theme image manifest is too large");
        const manifest = JSON.parse(source);
        const decodeVariants = variants => {
            if (typeof variants === "string" && SAFE_ICON_NAME.test(variants)) {
                const sizes = Array.isArray(manifest?.sizes?.[variants])
                    ? manifest.sizes[variants]
                    : [16, 32, 48];
                variants = sizes.map(size => ({
                    size,
                    file: `${variants}-${size}.png`,
                }));
            }
            if (!Array.isArray(variants) || variants.length > 3) return [];
            return variants.flatMap(variant => {
                const size = Number(variant?.size);
                const file = String(variant?.file || "");
                if (
                    ![16, 32, 48].includes(size) ||
                    !/^[a-z0-9]+(?:[.-][a-z0-9]+)*\.(?:png|svg)$/.test(file)
                ) {
                    return [];
                }
                const image = new globalThis.Image();
                image.addEventListener("load", () => this.onDamage(), { once: true });
                image.addEventListener("error", () => this.onDamage(), { once: true });
                image.src = iconAssetUrl(`${directory}/${file}`);
                return [{ size, image }];
            });
        };
        const icons = new Map();
        for (const [semantic, variants] of Object.entries(manifest?.icons || {})) {
            if (!SAFE_ICON_NAME.test(semantic)) continue;
            if (RICE_FILE_ICON_SEMANTICS.has(semantic) && variants === manifest.fallback) continue;
            const decoded = decodeVariants(variants);
            if (decoded.length > 0) icons.set(semantic, decoded);
        }
        const fallback = decodeVariants(manifest?.fallback);
        if (generation !== this.themeFontGeneration) return false;
        this.themeRasterIcons = icons;
        this.themeRasterFallback = fallback;
        this.onDamage();
        // Image-only packs explicitly suppress the otherwise speculative
        // `<slug>.css`/`.woff2` requests. Redmond '98 is such a pack.
        return riceThemeRasterManifestLoadsFont(manifest);
    }

    async #loadThemeFont(slug, generation) {
        const cssFile = `${RICE_THEME_ICON_DIRECTORY}/${slug}.css`;
        const fontFile = `${RICE_THEME_ICON_DIRECTORY}/${slug}.woff2`;
        const cssResponse = await globalThis.fetch(iconAssetUrl(cssFile));
        if (generation !== this.themeFontGeneration) return;
        if (cssResponse.status === 404) return;
        if (!cssResponse.ok) {
            throw new Error(`theme icon CSS request failed: ${cssResponse.status}`);
        }
        const source = await cssResponse.text();
        if (source.length > 512 * 1024) throw new Error("theme icon CSS is too large");
        const glyphs = new Map(
            [...parseRiceIconCss(source)].filter(([name]) => name.endsWith("-color")),
        );
        if (glyphs.size < 1 || glyphs.size > 256) return;
        const response = await globalThis.fetch(iconAssetUrl(fontFile));
        if (!response.ok) {
            throw new Error(`theme icon font request failed: ${response.status}`);
        }
        const compressed = new Uint8Array(await response.arrayBuffer());
        if (compressed.byteLength < 4 || compressed.byteLength > 4 * 1024 * 1024) {
            throw new RangeError("theme icon font is outside its byte limit");
        }
        const declaredSfntSize = compressed.byteLength >= 20
            ? new DataView(
                compressed.buffer,
                compressed.byteOffset,
                compressed.byteLength,
            ).getUint32(16, false)
            : 0;
        if (declaredSfntSize < 12 || declaredSfntSize > 24 * 1024 * 1024) {
            throw new RangeError("theme icon font declares an unsafe decompressed size");
        }
        const family = `RiceWM Theme Icons ${slug}`;
        const font = typeof globalThis.FontFace === "function"
            ? new globalThis.FontFace(
                family,
                compressed.buffer.slice(
                    compressed.byteOffset,
                    compressed.byteOffset + compressed.byteLength,
                ),
                { style: "normal", weight: "normal" },
            )
            : null;
        const [loaded, module] = await Promise.all([
            font ? font.load() : null,
            loadWoff2Decoder(),
        ]);
        if (generation !== this.themeFontGeneration) return;
        const decompressed = module.decompress(compressed);
        if (
            !(decompressed instanceof Uint8Array) ||
            decompressed.byteLength > 24 * 1024 * 1024 ||
            decompressed.byteLength !== declaredSfntSize
        ) {
            throw new RangeError("theme icon font decompression failed");
        }
        let documents;
        try {
            documents = svgGlyphDocuments(
                decompressed,
                glyphs,
                name => name.endsWith("-color"),
            );
        } catch (error) {
            if (error?.message !== "RiceWM font lacks cmap or SVG color data") throw error;
            // COLR/CPAL and bitmap-color fonts render directly through the
            // loaded FontFace and do not carry an OpenType SVG table.
            documents = new Map();
        }
        this.themeFontFamily = family;
        this.themeFont = font;
        this.themeGlyphs = glyphs;
        if (loaded) {
            (globalThis.document?.fonts || globalThis.fonts)?.add?.(loaded);
            this.themeFontReady = true;
        }
        for (const [name, svgSource] of documents) {
            const blob = new Blob([svgSource], { type: "image/svg+xml" });
            const url = globalThis.URL.createObjectURL(blob);
            const image = new globalThis.Image();
            image.addEventListener("load", () => this.onDamage(), { once: true });
            image.addEventListener("error", () => this.onDamage(), { once: true });
            image.src = url;
            this.themeColorGlyphImages.set(name, image);
            this.themeColorGlyphUrls.add(url);
        }
    }

    #loadFont() {
        if (typeof globalThis.FontFace !== "function") return;
        const font = new globalThis.FontFace(
            RICE_ICON_FONT_FAMILY,
            `url("${iconAssetUrl(RICE_ICON_FONT_FILE).href}") format("woff2")`,
            { style: "normal", weight: "normal" },
        );
        this.font = font;
        return Promise.resolve(font.load()).then(loaded => {
            if (this.font !== font) return;
            (globalThis.document?.fonts || globalThis.fonts)?.add?.(loaded);
            this.fontReady = true;
            this.onDamage();
        }).catch(() => {
            if (this.font === font) this.fontReady = false;
        });
    }
}
