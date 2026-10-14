import {RiceThemeSkinRenderer} from './rice-theme-skin-renderer.mjs';
import {desktopThemeFont,themeColor} from './rice-theme-customization.mjs';
import {classicUiFont, ensureClassicUiFonts} from "./rice-classic-ui.mjs";
import {isDocumentEditor, paintDocumentEditor} from './rice-document-editor-layout.mjs';
import { documentCanvasFont, singleLineGeometry, documentTextBlockGeometry, ensureDocumentTextFonts } from "./rice-document-text-metrics.mjs";
// Advertised only when paired with the matching native snapshot decoder.
export const GRID_TEXT_STYLE_RENDERER_VERSION = 1;
import {t} from './rice-i18n.mjs';
const graphemeSegmenter = typeof Intl.Segmenter === 'function' ? new Intl.Segmenter(undefined, {granularity:'grapheme'}) : null;
const wordSegmenter = typeof Intl.Segmenter === 'function' ? new Intl.Segmenter(undefined, {granularity:'word'}) : null;
const graphemes = text => graphemeSegmenter ? Array.from(graphemeSegmenter.segment(text), part=>part.segment) : Array.from(text);

import {
    clearTerminalRowCache,
    renderTerminalRows,
} from "./rice-term-renderer.mjs";
import {
    rgbaColor,
    roundedPath,
} from "./rice-native-canvas-primitives.mjs";
import { FrostedIceMaterial } from "./rice-frosted-ice.mjs";

const MANROPE_FONT_URL = new URL(
    "assets/fonts/Manrope-VariableFont_wght.ttf",
    import.meta.url,
);
const TERMINAL_SYMBOL_FONT_URL = new URL(
    "assets/fonts/ricewm-nerd-symbols.woff2",
    import.meta.url,
);
const EMOJI_FONT_URL = new URL("assets/fonts/NotoEmoji.ttf", import.meta.url);
const WELCOME_JOURNEY_URL = new URL(
    "assets/img/ricewm-logo.svg",
    import.meta.url,
);

function intersectRects(left, right) {
    const x = Math.max(left.x, right.x);
    const y = Math.max(left.y, right.y);
    const edgeX = Math.min(left.x + left.width, right.x + right.width);
    const edgeY = Math.min(left.y + left.height, right.y + right.height);
    return {
        x,
        y,
        width: Math.max(0, edgeX - x),
        height: Math.max(0, edgeY - y),
    };
}

function overlaps(rect, clip) {
    return rect.width > 0 && rect.height > 0 && clip.width > 0 &&
        clip.height > 0 && rect.x < clip.x + clip.width &&
        rect.x + rect.width > clip.x && rect.y < clip.y + clip.height &&
        rect.y + rect.height > clip.y;
}

// An image underlay is captured before the rest of the command list paints.
// Controls in that same list can cover it without separate popup/dialog
// metadata. Replaying only the image would erase those foreground pixels.
function terminalImageHasForeground(context, commands, start, image, origin) {
    const t = context.getTransform();
    for (let index = start; index < commands.length; index++) {
        const command = commands[index], rect = command.rect;
        if (!rect || command.type === 'push-clip' || command.type === 'pop-clip') continue;
        if (overlaps({
            x: rect.x * t.a + t.e + (origin?.x || 0),
            y: rect.y * t.d + t.f + (origin?.y || 0),
            width: rect.width * t.a, height: rect.height * t.d,
        }, image.bounds)) return true;
    }
    return false;
}

export function paintStructuralGlass(
    context,
    source,
    rect,
    sampleRect,
    materialTint = [23, 33, 31],
) {
    if (
        !context || !source || !rect || !sampleRect ||
        rect.width < 1 || rect.height < 1 ||
        sampleRect.width < 1 || sampleRect.height < 1
    ) return false;
    context.save();
    context.beginPath();
    context.rect(rect.x, rect.y, rect.width, rect.height);
    context.clip();
    const blurBleed = 24;
    const sourceX = Math.max(0, sampleRect.x - blurBleed);
    const sourceY = Math.max(0, sampleRect.y - blurBleed);
    const sourceRight = Math.min(
        Number(source.width) || sampleRect.x + sampleRect.width,
        sampleRect.x + sampleRect.width + blurBleed,
    );
    const sourceBottom = Math.min(
        Number(source.height) || sampleRect.y + sampleRect.height,
        sampleRect.y + sampleRect.height + blurBleed,
    );
    const sourceWidth = Math.max(1, sourceRight - sourceX);
    const sourceHeight = Math.max(1, sourceBottom - sourceY);
    context.filter = "blur(10px) saturate(1.16) contrast(1.025)";
    context.drawImage(
        source,
        sourceX,
        sourceY,
        sourceWidth,
        sourceHeight,
        rect.x - (sampleRect.x - sourceX),
        rect.y - (sampleRect.y - sourceY),
        sourceWidth,
        sourceHeight,
    );
    context.filter = "none";
    const tintLuminance = materialTint[0] * .299 +
        materialTint[1] * .587 + materialTint[2] * .114;
    const tintAlpha = materialTint.length >= 4
        ? Math.max(0, Math.min(1, Number(materialTint[3]) / 255))
        : tintLuminance >= 180 ? .9 : .24;
    // Structural theme commands carry their intended material density in the
    // tint alpha. Keeping that alpha lets the wallpaper remain authoritative
    // enough for the adaptive foreground pass to switch between light and
    // dark ink instead of turning every pale sheet into opaque white.
    const inheritedAlpha = Number.isFinite(context.globalAlpha)
        ? context.globalAlpha
        : 1;
    context.globalAlpha = inheritedAlpha * tintAlpha;
    context.fillStyle = `rgb(${materialTint[0]},${materialTint[1]},${materialTint[2]})`;
    context.fillRect(rect.x, rect.y, rect.width, rect.height);
    context.restore();
    return true;
}

function wedgedPath(context, rect) {
    const { x, y, width, height } = rect;
    const cut = Math.max(2, Math.min(16, height * .28, width * .16));
    context.moveTo(x + cut, y);
    context.lineTo(x + width, y);
    context.lineTo(x + width, y + height - cut);
    context.lineTo(x + width - cut, y + height);
    context.lineTo(x, y + height);
    context.lineTo(x, y + cut);
    context.closePath();
}

function balloonPath(context, rect, radius, caretX, caretAtTop) {
    const { x, y, width, height } = rect;
    const right = x + width;
    const bottom = y + height;
    const corner = Math.max(0, Math.min(Number(radius) || 0, width / 2, height / 2));
    const caretHeight = 8;
    const caretHalfWidth = 9;
    const caretInset = Math.min(width / 2, Math.max(12, corner + caretHalfWidth));
    const minimumCaretX = x + caretInset;
    const maximumCaretX = right - caretInset;
    const pointerX = Math.max(minimumCaretX, Math.min(maximumCaretX, Number(caretX)));

    context.beginPath();
    context.moveTo(x + corner, y);
    if (caretAtTop) {
        context.lineTo(pointerX - caretHalfWidth, y);
        context.lineTo(pointerX, y - caretHeight);
        context.lineTo(pointerX + caretHalfWidth, y);
    }
    context.lineTo(right - corner, y);
    context.quadraticCurveTo(right, y, right, y + corner);
    context.lineTo(right, bottom - corner);
    context.quadraticCurveTo(right, bottom, right - corner, bottom);
    if (!caretAtTop) {
        context.lineTo(pointerX + caretHalfWidth, bottom);
        context.lineTo(pointerX, bottom + caretHeight);
        context.lineTo(pointerX - caretHalfWidth, bottom);
    }
    context.lineTo(x + corner, bottom);
    context.quadraticCurveTo(x, bottom, x, bottom - corner);
    context.lineTo(x, y + corner);
    context.quadraticCurveTo(x, y, x + corner, y);
    context.closePath();
}

export class RiceNativeCommandRenderer {
    constructor(context, {
        editor,
        icons,
        renderWebScene,
        mediaSurface = () => null,
        hostSurface = () => null,
        mediaStatus = () => null,
        now = () => globalThis.performance?.now?.() || Date.now(),
        onDamage = () => {},
        glassMaterial = null,
    }) {
        this.context = context;
        this.editor = editor;
        this.icons = icons;
        this.renderWebScene = renderWebScene;
        this.mediaSurface = mediaSurface;
        this.hostSurface = typeof hostSurface === "function" ? hostSurface : () => null;
        this.mediaStatus = mediaStatus;
        this.now = now;
        this.onDamage = onDamage;
        this.inlineImages = new Map();
        this.documentFontWork = new WeakSet();
        this.animatedDamagePending = false;
        this.welcomeJourneyAnimationStarted = null;
        this.structuralGlassSource = null;
        this.terminalSymbolFontRequested = false;
        this.emojiFontRequested = false;
        this.glass = glassMaterial || new FrostedIceMaterial({
            surface: "native-command",
        });
        this.#loadBrandFont(onDamage);
        this.#loadWelcomeJourney(onDamage);
    }

    setStructuralGlassSource(source) {
        this.structuralGlassSource = source || null;
    }

    #loadBrandFont(onDamage=this.onDamage) {
        if (typeof globalThis.FontFace !== "function") return;
        const font = new globalThis.FontFace(
            "Manrope",
            `url("${MANROPE_FONT_URL.href}") format("truetype")`,
            { style: "normal", weight: "200 800" },
        );
        Promise.resolve(font.load()).then(loaded => {
            globalThis.document?.fonts?.add?.(loaded);
            onDamage();
        }).catch(() => {});
    }

    #loadTerminalSymbolFont() {
        if (this.terminalSymbolFontRequested ||
            typeof globalThis.FontFace !== "function") return;
        this.terminalSymbolFontRequested = true;
        const font = new globalThis.FontFace(
            "Symbols Nerd Font Mono",
            `url("${TERMINAL_SYMBOL_FONT_URL.href}") format("woff2")`,
            { style: "normal", weight: "normal" },
        );
        Promise.resolve(font.load()).then(loaded => {
            globalThis.document?.fonts?.add?.(loaded);
            clearTerminalRowCache();
            this.onDamage();
        }).catch(() => {});
    }

    #loadEmojiFont() {
        if (this.emojiFontRequested || typeof globalThis.FontFace !== "function") return;
        this.emojiFontRequested = true;
        const font = new globalThis.FontFace("RiceWM Emoji",
            `url("${EMOJI_FONT_URL.href}") format("truetype")`);
        Promise.resolve(font.load()).then(loaded => {
            globalThis.document?.fonts?.add?.(loaded);
            this.onDamage();
        }).catch(() => {});
    }

    #loadWelcomeJourney(onDamage) {
        this.welcomeJourney = null;
        if (typeof globalThis.Image !== "function") return;
        const image = new globalThis.Image();
        image.addEventListener("load", () => onDamage(), { once: true });
        image.addEventListener("error", () => onDamage(), { once: true });
        image.src = WELCOME_JOURNEY_URL.href;
        this.welcomeJourney = image;
    }

    render(commands, chrome) {
        if ((chrome?.redmond98||chrome?.typography?.family==='classic'||chrome?.typography?.titleFamily==='classic') && !this.classicFontsRequested) {
            this.classicFontsRequested = true;
            ensureClassicUiFonts()?.then(() => this.onDamage()).catch(() => {});
        }
        if(chrome?.typography?.family==='manrope'||chrome?.typography?.titleFamily==='manrope')this.#loadBrandFont();
        const context = this.context;
        this.skinRenderer ||= new RiceThemeSkinRenderer(()=>this.onDamage());
        const skippedFaces=new Set();
        for(let i=0;i<commands.length;i++)if(commands[i].type==='theme-surface'){
            const tag=commands[i],skin=this.skinRenderer.style(chrome,tag.surfaceRole,tag.surfaceState);
            for(let j=i-1;j>=0&&['fill','border','shaped-fill','shaped-border','glass','stroke-line'].includes(commands[j].type);j--){
                const command=commands[j];if(!command.rect||!Object.keys(tag.rect).every(key=>command.rect[key]===tag.rect[key]))continue;
                const face=['fill','shaped-fill','glass'].includes(command.type),border=['border','shaped-border'].includes(command.type);
                if(face&&(skin.background!==undefined||skin.gradient||skin.artwork&&this.skinRenderer.image(skin.artwork))||border&&skin.border!==undefined)skippedFaces.add(j);
            }
        }
        let surface=null,iconOverride=null;
        let clipDepth = 0;
        const clipStack = [];
        try {
            for (let commandIndex = 0; commandIndex < commands.length; commandIndex++) {
                if(skippedFaces.has(commandIndex))continue;
                let command = commands[commandIndex];
                if(command.type==='icon-override'){iconOverride=command;continue;}
                if(command.type==='theme-surface'){surface=command;this.skinRenderer.paint(context,command.rect,chrome,command.surfaceRole,command.surfaceState);continue;}
                if(surface&&command.type==='text'&&!command.font&&command.rect.x>=surface.rect.x&&command.rect.y>=surface.rect.y&&command.rect.x+command.rect.width<=surface.rect.x+surface.rect.width&&command.rect.y+command.rect.height<=surface.rect.y+surface.rect.height){const foreground=this.skinRenderer.style(chrome,surface.surfaceRole,surface.surfaceState).foreground;if(foreground)command={...command,color:themeColor(foreground,command.color)};}
                const rect = command.rect;
                if (command.type === "pop-clip") {
                    if (clipDepth > 0) {
                        context.restore();
                        clipDepth -= 1;
                        clipStack.pop();
                    }
                    continue;
                }
                if (
                    !rect ||
                    ![rect.x, rect.y, rect.width, rect.height].every(
                        Number.isInteger,
                    )
                ) {
                    throw new TypeError("invalid native UI command geometry");
                }
                if (command.type === "push-clip") {
                    context.save();
                    context.beginPath();
                    context.rect(rect.x, rect.y, rect.width, rect.height);
                    context.clip();
                    clipDepth += 1;
                    const parent = clipStack.at(-1);
                    clipStack.push(parent ? intersectRects(parent, rect) : rect);
                } else if (
                    clipStack.length > 0 &&
                    !overlaps(rect, clipStack.at(-1))
                ) {
                    // Canvas clipping prevents pixels from escaping, but it
                    // does not prevent expensive shaders, image decoding, or
                    // animation scheduling. Cull fully hidden retained
                    // commands before any renderer-specific work begins.
                    continue;
                } else if (command.type === "fill") {
                    context.fillStyle = rgbaColor(
                        this.#materialFillColor(command),
                    );
                    context.beginPath();
                    if (command.shape === "wedged") {
                        wedgedPath(context, rect);
                    } else {
                        roundedPath(context, rect, command.radius);
                    }
                    context.fill();
                } else if (command.type === "border") {
                    this.#renderBorder(command);
                } else if (command.type === "stroke-line") {
                    context.save();context.strokeStyle=rgbaColor(command.color);context.lineWidth=command.pattern===6?.5:command.width;
                    const w=command.width;context.setLineDash([[],[w,w],[4*w,2*w],[4*w,2*w,w,2*w],[4*w,2*w,w,2*w,w,2*w],[5*w,2*w,w,2*w],[]][command.pattern]);
                    context.beginPath();
                    if(rect.width<=w){context.moveTo(rect.x+w/2,rect.y);context.lineTo(rect.x+w/2,rect.y+rect.height);}
                    else {context.moveTo(rect.x,rect.y+w/2);context.lineTo(rect.x+rect.width,rect.y+w/2);}
                    context.stroke();context.restore();
                } else if (command.type === "dotted-line") {
                    context.save();
                    context.strokeStyle = rgbaColor(command.color);
                    context.lineWidth = 1;
                    context.setLineDash([1, 1]);
                    context.beginPath();
                    if (rect.width === 1) {
                        context.moveTo(rect.x + .5, rect.y);
                        context.lineTo(rect.x + .5, rect.y + rect.height);
                    } else {
                        context.moveTo(rect.x, rect.y + .5);
                        context.lineTo(rect.x + rect.width, rect.y + .5);
                    }
                    context.stroke();
                    context.restore();
                } else if (command.type === "color-picker") {
                    renderNativeColorPicker(context, command);
                } else if (command.type === "terminal-grid") {
                    context.save();
                    if (command.backdrop) {
                        context.beginPath(); context.rect(rect.x, rect.y, rect.width, rect.height); context.clip();
                        context.globalCompositeOperation = "copy";
                        context.drawImage(command.backdrop, rect.x, rect.y, rect.width, rect.height);
                        context.globalCompositeOperation = "source-over";
                    }
                    context.globalAlpha *= command.backdrop || command.directBackdrop ? 1 - (command.transparency || 0) / 100 : 1;
                    context.fillStyle = `#${(this.terminalTheme?.background ?? 0x10141c).toString(16).padStart(6,"0")}`;
                    context.fillRect(rect.x, rect.y, rect.width, rect.height);
                    context.restore();
                } else if (command.type === "terminal-row") {
                    let end = commandIndex + 1;
                    while (end < commands.length && commands[end].type === 'terminal-row' &&
                        commands[end].rect && ['x','y','width','height'].every(key => Number.isInteger(commands[end].rect[key])) &&
                        (!clipStack.length || overlaps(commands[end].rect, clipStack.at(-1)))) end++;
                    if (commands.slice(commandIndex, end).some(c => c.row?.icon_glyphs !== false)) {
                        this.#loadTerminalSymbolFont();
                    }
                    const capture = this.terminalImageCapture ? (target, image) =>
                        this.terminalImageCapture(target, {...image, replaySafe:
                            !terminalImageHasForeground(target, commands, end, image, this.paintOrigin)}) : null;
                    renderTerminalRows(context, commands.slice(commandIndex, end), this.terminalTheme, this.onDamage,
                        this.motionRasterScale, this.paintOrigin, this.terminalImageVisible, capture);
                    commandIndex = end - 1;
                } else if (command.type === "group-box-frame") {
                    this.#renderGroupBoxFrame(command);
                } else if (command.type === "balloon-frame") {
                    this.#renderBalloonFrame(command);
                } else if (command.type === "text-selection") {
                    this.#renderTextSelection(command, chrome);
                } else if (command.type === "text") {
                    if (isDocumentEditor(command)) {
                        const pending=ensureDocumentTextFonts(command,context.canvas?.ownerDocument);
                        if(pending&&!this.documentFontWork.has(pending)){this.documentFontWork.add(pending);pending.then(()=>this.onDamage()).catch(()=>{});}
                        paintDocumentEditor(context,command);continue;
                    }
                    if (command.role === "editor") {
                        this.editor.render(command, chrome);
                        continue;
                    }
                    if (command.role === "markdown") {
                        this.#renderMarkdown(command);
                    } else {
                        context.save();
                        try { this.#renderText(command, chrome); } finally { context.restore(); }
                    }
                } else if (command.type === "icon") {
                    if(iconOverride){const overridden=this.icons.renderOverride(iconOverride.selector,command);iconOverride=null;if(overridden){this.icons.renderShortcutOverlay(command);continue;}}
                    const animated = command.animated === true;
                    if (animated) {
                        const phase = this.now() / 1100;
                        const cx = command.rect.x + command.rect.width / 2;
                        const cy = command.rect.y + command.rect.height / 2;
                        const scale = 1 + Math.sin(phase) * 0.018;
                        context.save();
                        context.translate(cx, cy + Math.sin(phase * 0.72) * 2);
                        context.scale(scale, scale);
                        context.translate(-cx, -cy);
                    }
                    if (
                        !command.namedIcon ||
                        !this.icons.renderNamed({
                            ...command,
                            icon: command.namedIcon,
                        })
                    ) this.icons.render(command);
                    this.icons.renderShortcutOverlay(command);
                    if (animated) context.restore();
                } else if (command.type === "spinner") {
                    this.#renderSpinner(command);
                } else if (command.type === "activity") {
                    this.#renderActivity(command);
                } else if (command.type === "scenic-backdrop") {
                    this.#renderScenicBackdrop(command);
                } else if (command.type === "glass") {
                    this.#renderGlass(command, chrome);
                } else if (command.type === "shaped-fill") {
                    this.#renderShapedControl(command, false);
                } else if (command.type === "shaped-border") {
                    this.#renderShapedControl(command, true);
                } else if (command.type === "welcome-journey") {
                    this.#renderWelcomeJourney(command);
                } else if (command.type === "media") {
                    this.#renderMedia(command);
                } else if (command.type === "inline-image") {
                    this.#renderInlineImage(command);
                } else if (command.type === "web-scene") {
                    this.renderWebScene(command);
                } else if (command.type === "host-portal") {
                    const surface = this.hostSurface(command.identity);
                    if (surface) {
                        // Retained native buffers follow the same window clip,
                        // opacity and stacking as other RiceWM draw commands.
                        this.context.drawImage(surface, command.rect.x, command.rect.y,
                            command.rect.width, command.rect.height);
                    } else {
                        this.context.clearRect(command.rect.x, command.rect.y,
                            command.rect.width, command.rect.height);
                    }
                } else if (command.type === "terminal-portal") {
                    this.context.clearRect(
                        command.rect.x,
                        command.rect.y,
                        command.rect.width,
                        command.rect.height,
                    );
                } else {
                    throw new RangeError("unknown native UI draw command");
                }
            }
        } finally {
            while (clipDepth > 0) {
                context.restore();
                clipDepth -= 1;
                clipStack.pop();
            }
        }
    }

    #renderSpinner(command) {
        const { context } = this;
        const { rect } = command;
        const cx = rect.x + rect.width / 2;
        const cy = rect.y + rect.height / 2;
        const outer = Math.max(2, Math.min(rect.width, rect.height) / 2 - 1);
        const inner = outer * 0.48;
        const phase = Math.floor(this.now() / 90) % 10;
        context.save();
        context.strokeStyle = rgbaColor(command.color);
        context.lineWidth = Math.max(1.5, outer * 0.22);
        context.lineCap = "round";
        for (let index = 0; index < 10; index += 1) {
            const angle = index * Math.PI * 2 / 10 - Math.PI / 2;
            const distance = (index - phase + 10) % 10;
            context.globalAlpha = 1 - distance * 0.085;
            context.beginPath();
            context.moveTo(
                cx + Math.cos(angle) * inner,
                cy + Math.sin(angle) * inner,
            );
            context.lineTo(
                cx + Math.cos(angle) * outer,
                cy + Math.sin(angle) * outer,
            );
            context.stroke();
        }
        context.restore();
    }

    #renderActivity(command) {
        const { context } = this;
        const { rect } = command;
        const segmentWidth = Math.max(12, rect.width * 0.34);
        const travel = Math.max(0, rect.width - segmentWidth);
        const cycle = (this.now() % 1400) / 1400;
        const eased = 0.5 - Math.cos(cycle * Math.PI * 2) / 2;
        const x = rect.x + travel * eased;
        context.save();
        context.globalAlpha = 0.72 + 0.28 * Math.sin(cycle * Math.PI);
        context.fillStyle = rgbaColor(command.color);
        roundedPath(context, {
            x,
            y: rect.y,
            width: segmentWidth,
            height: rect.height,
        }, Math.min(rect.height / 2, 999));
        context.fill();
        context.restore();
    }

    #materialFillColor(command) {
        // The GPU material owns backdrop translucency. Preserve the theme's
        // intended panel density so dialogs and menus remain readable.
        return command.color;
    }

    #renderGroupBoxFrame(command) {
        const { context } = this;
        const { rect } = command;
        const width = Math.max(1, Number(command.width) || 1);
        const sizePx = Math.max(8, Number(command.sizePx) || 12);
        const top = Number.isFinite(Number(command.legendY))
            ? Number(command.legendY)
            : rect.y + sizePx / 2;
        const left = rect.x + width / 2;
        const right = rect.x + rect.width - width / 2;
        const bottom = rect.y + rect.height - width / 2;
        const legendStart = Math.max(left, Number(command.legendX) || left);
        context.save();
        context.font = `400 ${sizePx}px "RiceWM Sans", "RiceWM CJK", sans-serif`;
        const legendWidth = context.measureText(String(command.legend || "")).width;
        // Two physical pixels are the intentional breathing room. The glyph
        // run itself is measured by the loaded browser font; no Rust-side
        // average-character estimate participates in this cutout.
        const gapStart = Math.max(left, legendStart - 2);
        const gapEnd = Math.min(right, legendStart + legendWidth + 2);
        context.strokeStyle = rgbaColor(command.color);
        context.lineWidth = width;
        context.beginPath();
        context.moveTo(left, top);
        context.lineTo(gapStart, top);
        context.moveTo(gapEnd, top);
        context.lineTo(right, top);
        context.lineTo(right, bottom);
        context.lineTo(left, bottom);
        context.lineTo(left, top);
        context.stroke();
        context.restore();
    }

    #renderBalloonFrame(command) {
        const { context } = this;
        const { rect } = command;
        context.save();
        context.fillStyle = rgbaColor(command.fill);
        context.strokeStyle = rgbaColor(command.border);
        context.lineWidth = 1;
        context.shadowColor = "rgba(0, 0, 0, .28)";
        context.shadowBlur = 10;
        context.shadowOffsetY = 4;
        balloonPath(context, rect, command.radius, command.caretX, command.caretAtTop);
        context.fill();
        context.shadowColor = "transparent";
        context.stroke();
        context.restore();
    }

    #renderText(command, chrome) {
        const { context } = this;
        const { rect } = command;
        if(command.font) {
            const pending=ensureDocumentTextFonts(command,context.canvas?.ownerDocument);
            if(pending&&!this.documentFontWork.has(pending)) {
                this.documentFontWork.add(pending);
                pending.then(()=>this.onDamage()).catch(()=>{});
            }
        }
        if (command.font || command.documentText) {
            context.beginPath?.(); context.rect?.(rect.x, rect.y, rect.width, rect.height); context.clip?.();
        }
        // Labels use explicit wire direction; editable text retains LTR until
        // input and paint share shaped caret/selection geometry.
        const rtl = command.direction === "rtl" && !command.caret &&
            command.caretOffset == null && !command.brand;
        context.direction = rtl ? "rtl" : "ltr";
        const emojiOnlyFont = /[\p{Extended_Pictographic}\p{Regional_Indicator}]/u.test(
            String(command.text || ""),
        );
        if (emojiOnlyFont) this.#loadEmojiFont();
        context.fillStyle = command.gradient && context.createLinearGradient
            ? this.#brandGradient(command.rect)
            : rgbaColor(command.color);
        context.font = command.font
            ? documentCanvasFont(command)
            : `${command.brand ? 700 : 400} ${Math.max(8, Number(command.sizePx) || 12)}px ` +
            (command.brand
                ? '"Manrope", "RiceWM Sans", "RiceWM CJK", sans-serif'
                : emojiOnlyFont
                    ? '"RiceWM Sans", "RiceWM Emoji", "Noto Color Emoji", "Apple Color Emoji", "RiceWM CJK", sans-serif'
                    : chrome?.redmond98 ? classicUiFont
                    : '"RiceWM Sans", "RiceWM CJK", sans-serif');
        if(!command.font&&!command.brand&&chrome?.typography&&Object.keys(chrome.typography).length)context.font=desktopThemeFont(chrome.typography,'body',Math.max(8,Number(command.sizePx)||12));
        if(command.font) context.textRendering='geometricPrecision';
        if (command.brand && command.gradient && command.text === "RiceWM") {
            context.textAlign = "left";
            context.textBaseline = "middle";
            const rice = "Rice";
            const startX = rect.x + 2;
            const baseline = rect.y + rect.height / 2;
            context.fillStyle = rgbaColor(command.color);
            context.fillText(rice, startX, baseline);
            const riceWidth = context.measureText(rice).width;
            context.fillStyle = this.#brandGradient({
                x: startX + riceWidth,
                y: rect.y,
                width: Math.max(1, rect.width - riceWidth - 2),
                height: rect.height,
            });
            context.fillText("WM", startX + riceWidth, baseline);
            return;
        }
        const logicalAlign = ["start", "center", "end"].includes(command.align)
            ? command.align
            : "start";
        const align = rtl ? {start:"end", center:"center", end:"start"}[logicalAlign] : logicalAlign;
        context.textAlign = {
            start: "left",
            center: "center",
            end: "right",
        }[align];
        context.textBaseline = "middle";
        const inset = 2;
        const available = Math.max(1, rect.width - inset * 2);
        const {lineHeight} = documentTextBlockGeometry(chrome?.typography?.lineHeightPercent&&!command.font?{...command,documentText:true,lineHeightPx:Math.round((Number(command.sizePx)||12)*chrome.typography.lineHeightPercent/100)}:command);
        const focusedSingleLine = command.caret === true && align === "start" &&
            !String(command.text || "").includes("\n");
        if (focusedSingleLine) {
            const {text,characters,advance:caretAdvance,textX,baseline,top} = singleLineGeometry(context,command);
            context.save();
            context.beginPath?.();
            context.rect?.(rect.x, rect.y, rect.width, rect.height);
            context.clip?.();
            if (command.selection && command.selection.end > command.selection.start) {
                const start = context.measureText(characters.slice(0, command.selection.start).join("")).width;
                const end = context.measureText(characters.slice(0, command.selection.end).join("")).width;
                context.save(); context.fillStyle = rgbaColor(command.selection.color);
                // Chrome fields retain full-height selection. Document editors
                // use the caret line even when center is encoded by no flag.
                const aligned = command.documentEditor === true;
                context.fillRect(textX + start, aligned ? top : rect.y,
                    end - start, aligned ? lineHeight : rect.height); context.restore();
            }
            context.fillText(text, textX, baseline);
            if (command.font?.underline && text) {
                const metrics = context.measureText(text), descent = Number.isFinite(metrics.actualBoundingBoxDescent) ? metrics.actualBoundingBoxDescent : command.sizePx * .35;
                context.fillRect(textX, baseline + descent + 1, metrics.width, Math.max(1, Math.round(command.sizePx / 14)));
            }
            context.fillRect(
                Math.round(textX + caretAdvance + 1),
                Math.round(baseline - lineHeight / 2),
                1,
                lineHeight,
            );
            context.restore();
            return;
        }
        const lines = String(command.text || "")
            .split(/\r?\n/)
            .flatMap(line => command.wrap === false ? [line] : this.#wrappedText(line, available));
        const visibleCount = Math.max(
            1,
            Math.min(lines.length, Math.floor(rect.height / lineHeight) || 1),
        );
        const visible = lines.slice(0, visibleCount);
        if (visibleCount < lines.length && visible.length > 0) {
            visible[visible.length - 1] = this.#ellipsizedText(
                `${visible.at(-1)}…`,
                available,
            );
        }
        const {baseline: startY} = documentTextBlockGeometry(command, visible.length);
        visible.forEach((line, index) => {
            const rendered = this.#ellipsizedText(line, available);
            const x = align === "center" ? rect.x + rect.width / 2
                : align === "end" ? rect.x + rect.width - inset : rect.x + inset;
            const y = startY + index * lineHeight;
            context.fillText(rendered, x, y);
            if (command.strike && rendered) {
                const width=Math.min(available,context.measureText(rendered).width),start=align === "center" ? x-width/2 : align === "end" ? x-width : x;
                context.fillRect(start,y,width,Math.max(1,Math.round((Number(command.sizePx)||12)/14)));
            }
            if (command.font?.underline && rendered) {
                const metrics = context.measureText(rendered), width = Math.min(available, metrics.width);
                const start = align === "center" ? x - width / 2 : align === "end" ? x - width : x;
                const descent = Number.isFinite(metrics.actualBoundingBoxDescent)
                    ? metrics.actualBoundingBoxDescent : (Number(command.sizePx) || 12) * .35;
                context.fillRect(start, y + descent + 1, width, Math.max(1, Math.round((Number(command.sizePx) || 12) / 14)));
            }
        });
        if (command.caret && visible.length === 1 && align === "start") {
            const rendered = this.#ellipsizedText(visible[0], available);
            const characters = Array.from(rendered);
            const caretOffset = Number.isInteger(command.caretOffset)
                ? Math.max(0, Math.min(characters.length, command.caretOffset))
                : characters.length;
            const prefix = characters.slice(0, caretOffset).join("");
            const caretX = Math.min(
                rect.x + rect.width - inset,
                rect.x + inset + context.measureText(prefix).width + 1,
            );
            context.fillRect(
                Math.round(caretX),
                Math.round(startY - lineHeight / 2),
                1,
                lineHeight,
            );
        }
    }

    #renderTextSelection(command, chrome) {
        const { context } = this;
        const { rect } = command;
        const characters = Array.from(String(command.text || ""));
        const start = Math.max(0, Math.min(characters.length, command.start | 0));
        const end = Math.max(start, Math.min(characters.length, command.end | 0));
        if (start === end) return;

        context.save();
        context.direction = "ltr";
        context.font = `400 ${Math.max(8, Number(command.sizePx) || 12)}px ` +
            (chrome?.redmond98 ? classicUiFont : '"RiceWM Sans", "RiceWM CJK", sans-serif');
        const inset = 2;
        const available = Math.max(1, rect.width - inset * 2);
        const fullWidth = context.measureText(characters.join("")).width;
        const prefixWidth = context.measureText(
            characters.slice(0, start).join(""),
        ).width;
        // Selection boundaries belong to the shaped full run. Measuring the
        // selected substring in isolation loses kerning at both edges and can
        // paint a glyph past the user's actual caret. Prefix advances keep the
        // highlight endpoints on the same run as the rendered text.
        const selectedWidth = Math.max(0, context.measureText(
            characters.slice(0, end).join(""),
        ).width - prefixWidth);
        const align = ["start", "center", "end"].includes(command.align)
            ? command.align
            : "start";
        const textX = align === "center"
            ? rect.x + rect.width / 2 - fullWidth / 2
            : align === "end"
                ? rect.x + rect.width - inset - fullWidth
                : rect.x + inset;
        const selectionX = textX + prefixWidth;
        const lineHeight = Math.max(command.font || command.documentText ? 1 : 10, Math.round(
            (Number(command.sizePx) || 12) * 1.25,
        ));
        const selectionY = rect.y + (rect.height - lineHeight) / 2;

        context.beginPath();
        context.rect(rect.x, rect.y, rect.width, rect.height);
        context.clip();
        context.fillStyle = rgbaColor(command.color);
        const clippedX = Math.max(rect.x + inset, selectionX);
        const clippedWidth = Math.max(0, Math.min(
            selectedWidth,
            rect.x + inset + available - clippedX,
        ));
        if (clippedWidth > 0) context.fillRect(
            clippedX,
            selectionY,
            clippedWidth,
            lineHeight,
        );
        context.restore();
    }

    #brandGradient(rect) {
        const gradient = this.context.createLinearGradient(
            rect.x,
            rect.y,
            rect.x + Math.max(1, rect.width),
            rect.y,
        );
        gradient.addColorStop(0, "#4f7ee8");
        gradient.addColorStop(.46, "#7287de");
        gradient.addColorStop(.72, "#d47f72");
        gradient.addColorStop(1, "#ff853d");
        return gradient;
    }

    #renderScenicBackdrop(command) {
        const { context } = this;
        const { rect } = command;
        context.save();
        roundedPath(context, rect, Math.min(14, rect.width / 18, rect.height / 18));
        context.clip();
        context.fillStyle = "#0a1726";
        context.fillRect(rect.x, rect.y, rect.width, rect.height);
        context.restore();
    }

    #renderWelcomeJourney(command) {
        const transform = this.context.getTransform?.();
        const visibleRect = {
            x: command.rect.x + (Number(transform?.e) || 0),
            y: command.rect.y + (Number(transform?.f) || 0),
            width: command.rect.width,
            height: command.rect.height,
        };
        const canvas = this.context.canvas;
        const visible = !canvas || (
            visibleRect.x < canvas.width &&
            visibleRect.y < canvas.height &&
            visibleRect.x + visibleRect.width > 0 &&
            visibleRect.y + visibleRect.height > 0
        );
        if (!visible) {
            this.welcomeJourneyAnimationStarted = null;
            return;
        }
        const image = this.welcomeJourney;
        if (image?.complete && image.naturalWidth > 0 && image.naturalHeight > 0) {
            const inset = 10;
            this.context.drawImage(
                image,
                command.rect.x + inset,
                command.rect.y + inset,
                Math.max(1, command.rect.width - inset * 2),
                Math.max(1, command.rect.height - inset * 2),
            );
        }
        const now = this.now();
        if (this.welcomeJourneyAnimationStarted === null) {
            this.welcomeJourneyAnimationStarted = now;
        }
        // SMIL animation in the SVG only needs occasional canvas refreshes.
        // Run a single eight-second introduction at 10 fps while the diagram
        // is actually visible; never invalidate the full Welcome document in
        // an unbounded requestAnimationFrame loop.
        if (
            now - this.welcomeJourneyAnimationStarted < 8_000 &&
            !this.animatedDamagePending &&
            typeof globalThis.setTimeout === "function"
        ) {
            this.animatedDamagePending = true;
            globalThis.setTimeout(() => {
                this.animatedDamagePending = false;
                this.onDamage();
            }, 100);
        }
    }

    #renderGlass(command, chrome) {
        const { context } = this;
        const legacyTone = [
            [125, 168, 225],
            [255, 137, 70],
            [85, 195, 151],
            [106, 163, 255],
        ][Math.max(0, Math.min(3, Number(command.tone) || 0))];
        const tint = Array.isArray(command.tint) && command.tint.length === 4
            ? command.tint.slice(0, 3)
            : legacyTone;
        const control = command.control === true;
        const active = command.active === true;
        const structural = command.structural === true;
        const popup = command.popup === true;
        const transform = context.getTransform?.();
        const sampleRect = {
            x: command.rect.x + (Number(transform?.e) || 0),
            y: command.rect.y + (Number(transform?.f) || 0),
            width: command.rect.width,
            height: command.rect.height,
        };
        if (structural) {
            const source = this.structuralGlassSource;
            if (!source) return;
            const materialTint = chrome?.frostedIce?.enabled === true &&
                Array.isArray(chrome.frostedIce.tint)
                ? [...chrome.frostedIce.tint.slice(0, 3), command.tint?.[3] ?? 112]
                : Array.isArray(command.tint) ? command.tint : [...tint, 112];
            // A structural sidebar is a sheet cut from the window, not a
            // floating glass card. Blur an expanded, registered backdrop so
            // detail bends softly through the whole pane without producing
            // the lens shader's gray fill, rim, or inset silhouette.
            paintStructuralGlass(
                context,
                source,
                command.rect,
                sampleRect,
                materialTint,
            );
            return;
        }
        const widgetBorder = Math.max(
            0,
            Math.min(100, Number(chrome?.frostedIce?.widgetBorder) || 0),
        );
        const profile = control ? {
            ...(chrome?.frostedIce || {}),
            tint,
            rim: false,
            rimWidth: 0,
        } : popup ? {
            model: chrome?.frostedIce?.model,
            thickness: 54,
            opacity: 43,
            blur: 58,
            refraction: 34,
            chroma: 8,
            tint,
            rim: false,
            rimWidth: 0,
            sheet: true,
        } : {
            thickness: command.hovered ? 82 : 66,
            opacity: command.hovered ? 54 : 43,
            blur: 58,
            refraction: command.hovered ? 64 : 48,
            chroma: 24,
            tint,
            rim: true,
        };
        const radius = control || popup
            ? Math.min(
                command.rect.height / 2,
                Number.isFinite(Number(command.radius))
                    ? Math.max(0, Number(command.radius))
                    : Number(chrome?.radius) || 6,
            )
            : Math.min(18, command.rect.height / 5);
        if (!control && !popup) {
            context.save();
            context.shadowColor = "rgba(0, 4, 14, .58)";
            context.shadowBlur = command.hovered ? 30 : 22;
            context.shadowOffsetY = command.hovered ? 14 : 10;
            context.fillStyle = "rgba(7, 15, 28, .16)";
            roundedPath(context, command.rect, radius);
            context.fill();
            context.restore();
        }
        if (popup) {
            // The WebGL material owns an entire rectangular backing canvas.
            // Constrain its composition as well as its shader mask so no
            // antialiased/refraction pixels can escape a popup's corners.
            context.save();
            roundedPath(context, command.rect, radius);
            context.clip();
        }
        const painted = this.glass.draw(
            context,
            context.canvas,
            command.rect,
            radius,
            profile,
            this.now(),
            sampleRect,
        );
        if (!painted) {
            context.save();
            const fallback = context.createLinearGradient(
                command.rect.x,
                command.rect.y,
                command.rect.x + command.rect.width,
                command.rect.y + command.rect.height,
            );
            const tintAlpha = control && active ? .28 : .18;
            fallback.addColorStop(
                0,
                `rgba(${tint[0]},${tint[1]},${tint[2]},${tintAlpha})`,
            );
            fallback.addColorStop(
                .46,
                `rgba(190, 218, 245, ${control ? .11 : .10})`,
            );
            fallback.addColorStop(
                1,
                `rgba(4, 11, 24, ${control ? .22 : .30})`,
            );
            context.fillStyle = fallback;
            roundedPath(context, command.rect, radius);
            context.fill();
            context.restore();
        }
        if (control) {
            // Preserve the semantic control color under its text. A nearly
            // transparent lens over a dark wallpaper cannot carry dark ink
            // chosen for a light accent color.
            context.save();roundedPath(context,command.rect,radius);context.clip();
            const orchard = chrome?.chromeProfile === "orchard";
            const lens = orchard && command.rect.width === command.rect.height &&
                command.rect.width <= 32 && tint.every(channel => channel >= 230);
            context.fillStyle = `rgba(${tint.join(',')},${lens ? .64 : .96})`;context.fill();
            if (orchard) {
                // Preserve semantic blue/neutral fills while giving the lens
                // a bright upper lip and a restrained blue-gray lower edge.
                const sheen = context.createLinearGradient(0, command.rect.y, 0,
                    command.rect.y + command.rect.height);
                sheen.addColorStop(0, `rgba(255,255,255,${lens ? .76 : .26})`);
                sheen.addColorStop(.46, "rgba(255,255,255,.03)");
                sheen.addColorStop(1, `rgba(69,115,151,${lens ? .12 : .04})`);
                context.fillStyle = sheen;context.fill();
                if (widgetBorder > 0) {
                    const rim = context.createLinearGradient(0, command.rect.y, 0,
                        command.rect.y + command.rect.height);
                    rim.addColorStop(0, "rgba(255,255,255,.92)");
                    rim.addColorStop(.5, "rgba(172,202,225,.20)");
                    rim.addColorStop(1, "rgba(92,132,164,.42)");
                    context.strokeStyle = rim;
                    context.lineWidth = widgetBorder / 100;context.stroke();
                }
            }
            if (command.hovered) {
                const light = tint[0]*.2126+tint[1]*.7152+tint[2]*.0722 > 145;
                context.fillStyle = light ? "rgba(0,0,0,.10)" : "rgba(255,255,255,.16)";context.fill();
                context.strokeStyle = light ? "rgba(0,0,0,.55)" : "rgba(255,255,255,.75)";
                context.lineWidth=2;context.stroke();
            }
            context.restore();return;
        }
        if (popup) context.restore();
        // Popup depth comes from the blurred sheet plus its following theme
        // coat/border. Decorative spectral/caustic overlays are intentionally
        // reserved for cards and controls.
        if (popup) return;
        context.save();
        roundedPath(context, command.rect, radius);
        context.clip();
        const spectral = context.createRadialGradient(
            command.rect.x + command.rect.width * .08,
            command.rect.y + command.rect.height * .06,
            0,
            command.rect.x + command.rect.width * .08,
            command.rect.y + command.rect.height * .06,
            command.rect.width * .72,
        );
        const spectralAlpha = control ? active ? .12 : .07 : .20;
        spectral.addColorStop(
            0,
            `rgba(${tint[0]},${tint[1]},${tint[2]},${spectralAlpha})`,
        );
        spectral.addColorStop(.36, "rgba(255,255,255,.035)");
        spectral.addColorStop(1, "rgba(255,255,255,0)");
        context.fillStyle = spectral;
        context.fillRect(command.rect.x, command.rect.y, command.rect.width, command.rect.height);
        if (!control && !popup) {
            const caustic = context.createRadialGradient(
                command.rect.x + command.rect.width * .92,
                command.rect.y + command.rect.height * 1.08,
                0,
                command.rect.x + command.rect.width * .92,
                command.rect.y + command.rect.height * 1.08,
                command.rect.width * .46,
            );
            caustic.addColorStop(0, "rgba(153,214,255,.24)");
            caustic.addColorStop(.34, `rgba(${tint[0]},${tint[1]},${tint[2]},.08)`);
            caustic.addColorStop(1, "rgba(255,255,255,0)");
            context.fillStyle = caustic;
            context.fillRect(
                command.rect.x,
                command.rect.y,
                command.rect.width,
                command.rect.height,
            );
        }
        context.restore();
        context.save();
        const sheen = context.createLinearGradient(
            command.rect.x,
            command.rect.y,
            command.rect.x,
            command.rect.y + command.rect.height,
        );
        sheen.addColorStop(0, `rgba(255,255,255,${control ? .055 : .085})`);
        sheen.addColorStop(
            .42,
            `rgba(${tint[0]},${tint[1]},${tint[2]},${control ? .02 : .035})`,
        );
        sheen.addColorStop(1, `rgba(3,9,18,${control ? .045 : .13})`);
        context.fillStyle = sheen;
        context.shadowColor = control || popup
            ? "transparent"
            : "rgba(0,0,0,.42)";
        context.shadowBlur = control || popup ? 0 : 24;
        context.shadowOffsetY = control || popup ? 0 : 12;
        roundedPath(context, command.rect, radius);
        context.fill();
        context.restore();
        // Popup borders are rendered by the theme palette immediately after
        // this material. Do not add any external spectral rim/shadow around
        // them: browser shadow filters leak crescent pixels at rounded corners
        // on some accelerated Canvas implementations.
        if (popup || (control && widgetBorder === 0)) return;
        context.save();
        const rim = context.createLinearGradient(
            command.rect.x,
            command.rect.y,
            command.rect.x + command.rect.width,
            command.rect.y + command.rect.height,
        );
        rim.addColorStop(0, `rgba(${tint[0]},${tint[1]},${tint[2]},${control ? .42 : command.hovered || active ? .95 : .68})`);
        rim.addColorStop(.38, `rgba(220,238,255,${control ? .08 : .18})`);
        rim.addColorStop(1, `rgba(145,198,255,${control ? .34 : .72})`);
        context.strokeStyle = rim;
        context.lineWidth = control
            ? Math.max(.15, widgetBorder * .01)
            : command.hovered || active ? 1.5 : 1;
        context.shadowColor = `rgba(${tint[0]},${tint[1]},${tint[2]},.5)`;
        context.shadowBlur = control
            ? 0
            : command.hovered ? 18 : 9;
        roundedPath(context, command.rect, radius);
        context.stroke();
        if (control) {
            context.restore();
            return;
        }
        const inner = {
            x: command.rect.x + 2,
            y: command.rect.y + 2,
            width: Math.max(0, command.rect.width - 4),
            height: Math.max(0, command.rect.height - 4),
        };
        context.strokeStyle = "rgba(255,255,255,.08)";
        context.lineWidth = 1;
        context.shadowBlur = 0;
        roundedPath(context, inner, Math.max(0, radius - 2));
        context.stroke();
        context.restore();
    }

    #renderShapedControl(command, border) {
        if(border&&command.width===0)return;
        const { context } = this;
        const { x, y, width, height } = command.rect;
        const cut = Math.max(2, Math.min(10, height * .28, width * .16));
        context.save();
        context.beginPath();
        if (command.shape === "pill") {
            roundedPath(context, command.rect, height / 2);
        } else if (command.shape === "wedged") {
            wedgedPath(context, command.rect);
        } else {
            context.moveTo(x + cut, y);
            context.lineTo(x + width - cut, y);
            context.lineTo(x + width, y + cut);
            context.lineTo(x + width, y + height - cut);
            context.lineTo(x + width - cut, y + height);
            context.lineTo(x + cut, y + height);
            context.lineTo(x, y + height - cut);
            context.lineTo(x, y + cut);
            context.closePath();
        }
        if (border) {
            context.strokeStyle = rgbaColor(command.color);
            context.lineWidth = Math.max(1, Number(command.width) || 1);
            context.stroke();
        } else {
            context.fillStyle = rgbaColor(command.color);
            context.fill();
        }
        context.restore();
    }

    #renderMarkdown(command) {
        const { context } = this;
        const { rect } = command;
        const baseSize = Math.max(8, Number(command.sizePx) || 12);
        const inset = 2;
        const available = Math.max(1, rect.width - inset * 2);
        const plain = value => String(value)
            .replaceAll(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
            .replaceAll(/\[([^\]]+)\]\(([^)]+)\)/g, "$1 ($2)")
            .replaceAll(/(\*\*|__|~~|`)/g, "");
        const rows = [];
        const blocks = [];
        let fenced = false;
        let pendingListItem = null;
        const flushListItem = () => {
            if (pendingListItem) blocks.push(pendingListItem);
            pendingListItem = null;
        };
        for (const source of String(command.text || "").split(/\r?\n/)) {
            if (source.trim().startsWith("```")) {
                flushListItem();
                fenced = !fenced;
                continue;
            }
            const unordered = !fenced && source.trim().match(/^[-*+]\s+(.+)$/);
            if (unordered) {
                flushListItem();
                pendingListItem = { type: "list", text: unordered[1] };
                continue;
            }
            if (
                pendingListItem && source.trim() &&
                /^\s+/.test(source) && !/^\s{0,3}#{1,6}\s+/.test(source)
            ) {
                pendingListItem.text += ` ${source.trim()}`;
                continue;
            }
            flushListItem();
            blocks.push({ type: fenced ? "fence" : "line", text: source });
        }
        flushListItem();
        let previousWasList = false;
        for (const block of blocks) {
            const source = block.text;
            let text = source.trim();
            let size = baseSize;
            let weight = 400;
            let family = '"RiceWM Sans", "RiceWM CJK", sans-serif';
            let marker = "";
            let x = rect.x + inset;
            let lineAvailable = available;
            if (block.type === "fence") {
                family = '"RiceWM Mono", "RiceWM CJK", monospace';
                text = source;
            } else if (block.type === "list") {
                marker = "•";
                context.font = `${weight} ${size}px ${family}`;
                const markerWidth = context.measureText(marker).width;
                const gap = Math.max(6, Math.round(baseSize * .65));
                x += markerWidth + gap;
                lineAvailable = Math.max(1, available - markerWidth - gap);
            } else if (/^###\s+/.test(text)) {
                text = text.replace(/^###\s+/, "");
                size = Math.round(baseSize * 1.05);
                weight = 650;
            } else if (/^##\s+/.test(text)) {
                text = text.replace(/^##\s+/, "");
                size = Math.round(baseSize * 1.15);
                weight = 650;
            } else if (/^#\s+/.test(text)) {
                text = text.replace(/^#\s+/, "");
                size = Math.round(baseSize * 1.28);
                weight = 700;
            } else if (/^>\s+/.test(text)) {
                text = `│ ${text.replace(/^>\s+/, "")}`;
            }
            if (/\*\*[^*]+\*\*|__[^_]+__/.test(text)) weight = Math.max(weight, 650);
            if (/`[^`]+`/.test(text)) family = '"RiceWM Mono", "RiceWM CJK", monospace';
            text = plain(text);
            if (!text) {
                rows.push({ text: "", size, weight, family, x, marker, marginTop: 0 });
                previousWasList = false;
                continue;
            }
            context.font = `${weight} ${size}px ${family}`;
            for (const [index, line] of this.#wrappedText(text, lineAvailable).entries()) {
                rows.push({
                    text: line,
                    size,
                    weight,
                    family,
                    x,
                    marker: index === 0 ? marker : "",
                    marginTop: index === 0 && block.type === "list" && previousWasList
                        ? Math.max(2, Math.round(baseSize * .25))
                        : 0,
                });
            }
            previousWasList = block.type === "list";
        }
        let y = rect.y + inset;
        context.fillStyle = rgbaColor(command.color);
        context.textAlign = "left";
        context.textBaseline = "top";
        for (const row of rows) {
            y += row.marginTop;
            const height = Math.max(10, Math.round(row.size * 1.35));
            if (y + height > rect.y + rect.height) break;
            context.font = `${row.weight} ${row.size}px ${row.family}`;
            if (row.marker) context.fillText(row.marker, rect.x + inset, y);
            if (row.text) context.fillText(row.text, row.x, y);
            y += height;
        }
    }

    #wrappedText(text, available) {
        const normalized = String(text).replaceAll(/\s+/g, " ").trim();
        if (!normalized || this.context.measureText(normalized).width <= available) {
            return [normalized];
        }
        // Paths are one presentation value even when a directory or filename
        // contains spaces. Wrapping them would retain the beginning line and
        // discard the basename; leave them intact for tail ellipsis below.
        if (this.#preservesTail(normalized)) return [normalized];
        const lines = [];
        let current = "";
        const words = normalized.split(/( )/).flatMap(word =>
            wordSegmenter && /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Hangul}\p{Script=Thai}]/u.test(word)
                ? Array.from(wordSegmenter.segment(word), part=>part.segment) : [word]);
        for (const word of words) {
            if (!current && !word.trim()) continue;
            if (current && this.context.measureText(current + word).width > available) {
                lines.push(current.trimEnd());
                current = "";
            }
            if (!word.trim()) { if (current) current += word; continue; }
            // Segment boundaries permit Thai/CJK wrapping without inserting
            // spaces. Long words may break only between complete graphemes.
            if (this.context.measureText(word).width > available) {
                for (const part of graphemes(word)) {
                    if (current && this.context.measureText(current + part).width > available) {
                        lines.push(current.trimEnd());current = "";
                    }
                    current += part;
                }
            } else {
                current += word;
            }
        }
        if (current) lines.push(current.trimEnd());
        return lines.length > 0 ? lines : [""];
    }

    #renderMedia(command) {
        // Embedded sessions are painted at their guest display-list position.
        // This command retains their frame subscription and damage rectangle.
        if (command.embedded) return;
        const source = command.surface || this.mediaSurface(command.request);
        if (!source) {
            this.#renderMediaStatus(command);
            return;
        }
        const sourceWidth = Number(
            source.displayWidth || source.videoWidth || source.naturalWidth || source.width,
        );
        const sourceHeight = Number(
            source.displayHeight || source.videoHeight || source.naturalHeight || source.height,
        );
        if (!Number.isFinite(sourceWidth) || sourceWidth <= 0 ||
            !Number.isFinite(sourceHeight) || sourceHeight <= 0) {
            this.#renderMediaFallback(command, "Preview unavailable");
            return;
        }
        const turns = Number(command.quarterTurns) || 0;
        const [sx, sy, sw, sh, outputWidth, outputHeight] = command.imageEdit || [0, 0, sourceWidth, sourceHeight,
            turns % 2 === 0 ? sourceWidth : sourceHeight, turns % 2 === 0 ? sourceHeight : sourceWidth];
        const drawWidth = turns % 2 === 0 ? outputWidth : outputHeight;
        const drawHeight = turns % 2 === 0 ? outputHeight : outputWidth;
        const rotatedWidth = outputWidth;
        const rotatedHeight = outputHeight;
        const baseScale = command.scaleMode === "actual"
            ? 1
            : Math.min(
                command.rect.width / rotatedWidth,
                command.rect.height / rotatedHeight,
            );
        const requestedScale = Number(command.scale);
        const zoom = Number.isFinite(requestedScale) && requestedScale > 0
            ? Math.min(requestedScale, 64)
            : 1;
        const scale = baseScale * zoom;
        if (!Number.isFinite(scale) || scale <= 0) {
            this.#renderMediaFallback(command, "Preview unavailable");
            return;
        }
        const maximumPanX = Math.max(
            0,
            (rotatedWidth * scale - command.rect.width) / 2,
        );
        const maximumPanY = Math.max(
            0,
            (rotatedHeight * scale - command.rect.height) / 2,
        );
        const panX = Math.max(
            -maximumPanX,
            Math.min(maximumPanX, Number(command.panX || 0)),
        );
        const panY = Math.max(
            -maximumPanY,
            Math.min(maximumPanY, Number(command.panY || 0)),
        );
        const context = this.context;
        context.save();
        context.beginPath();
        context.rect(
            command.rect.x,
            command.rect.y,
            command.rect.width,
            command.rect.height,
        );
        context.clip();
        context.translate(
            command.rect.x + command.rect.width / 2 +
                panX,
            command.rect.y + command.rect.height / 2 +
                panY,
        );
        context.rotate?.(turns * Math.PI / 2);
        if (command.imageEdit) {
            context.drawImage(source, sx, sy, sw, sh,
                -drawWidth * scale / 2, -drawHeight * scale / 2, drawWidth * scale, drawHeight * scale);
        } else {
            context.drawImage(source, -drawWidth * scale / 2, -drawHeight * scale / 2, drawWidth * scale, drawHeight * scale);
        }
        context.restore();
        this.#renderMediaStatus(command);
    }

    #renderInlineImage(command) {
        const owner=String(command.cacheNamespace??'');
        const key=`${owner}\0${command.dataUrl}`;
        let record = this.inlineImages.get(key);
        if (!record && typeof globalThis.Image === "function") {
            const image = new globalThis.Image();
            const changed=()=>{if(this.inlineImages.get(key)===record)this.onDamage();};
            record={image,owner,changed};
            this.inlineImages.set(key, record);
            image.addEventListener("load", changed, { once: true });
            image.addEventListener("error", changed, { once: true });
            image.src = command.dataUrl;
        }
        const image=record?.image;
        if (!image?.complete || !(image.naturalWidth > 0) || !(image.naturalHeight > 0)) {
            return;
        }
        const scale = Math.min(
            command.rect.width / image.naturalWidth,
            command.rect.height / image.naturalHeight,
        );
        const width = image.naturalWidth * scale;
        const height = image.naturalHeight * scale;
        this.context.drawImage(
            image,
            command.rect.x + (command.rect.width - width) / 2,
            command.rect.y + (command.rect.height - height) / 2,
            width,
            height,
        );
    }

    releaseInlineImages(owner=null) {
        for(const [key,record] of this.inlineImages) {
            if(owner!==null&&record.owner!==String(owner))continue;
            this.inlineImages.delete(key);
            record.image.removeEventListener?.('load',record.changed);
            record.image.removeEventListener?.('error',record.changed);
            record.image.src='';
        }
    }

    #renderMediaFallback(command, message) {
        const { context, mediaStatus } = this;
        const { rect } = command;
        context.save();
        context.fillStyle = "rgba(20, 31, 46, 0.92)";
        context.fillRect(rect.x, rect.y, Math.max(0, rect.width), Math.max(0, rect.height));
        context.fillStyle = "rgba(255, 255, 255, 0.82)";
        context.font = '500 12px "RiceWM Sans", "RiceWM CJK", sans-serif';
        context.textAlign = "center";
        context.textBaseline = "middle";
        context.fillText(
            message,
            rect.x + rect.width / 2,
            rect.y + rect.height / 2,
            Math.max(1, rect.width - 16),
        );
        context.restore();
        mediaStatus(command.request);
    }

    #renderMediaStatus(command) {
        const status = this.mediaStatus(command.request);
        if (status?.state !== "seeking") return;
        const context = this.context;
        const width = Math.max(180, Math.min(340, command.rect.width - 24));
        const height = 64;
        const x = command.rect.x + (command.rect.width - width) / 2;
        const y = command.rect.y + (command.rect.height - height) / 2;
        const parsed = formatByteCount(status.bytesParsed);
        const source = formatByteCount(status.sourceBytes);
        const detail = source
            ? `${parsed} parsed · ${source} source`
            : `${parsed} parsed`;
        context.save();
        context.fillStyle = "rgba(8, 12, 18, 0.82)";
        roundedPath(context, { x, y, width, height }, 10);
        context.fill();
        context.globalAlpha = status.blinkVisible === false ? 0.42 : 1;
        context.fillStyle = "#ffffff";
        context.font = '600 17px "RiceWM Sans", "RiceWM CJK", sans-serif';
        context.textAlign = "center";
        context.textBaseline = "middle";
        context.fillText(t("Seeking…"), x + width / 2, y + 23);
        context.globalAlpha = 0.78;
        context.font = '400 10px "RiceWM Sans", "RiceWM CJK", sans-serif';
        context.fillText(detail, x + width / 2, y + 45);
        context.restore();
    }

    #renderBorder(command) {
        if(command.width===0)return;
        const { rect } = command;
        const width = Math.max(1, Math.min(
            Number(command.width) || 1,
            Math.floor(Math.min(rect.width, rect.height) / 2),
        ));
        this.context.strokeStyle = rgbaColor(command.color);
        this.context.lineWidth = width;
        this.context.beginPath();
        if (command.shape === "wedged") {
            wedgedPath(this.context, rect);
        } else {
            // Canvas strokes straddle their path. Drawing on the declared
            // outer edge leaks half a stroke into transparent popup corners,
            // which accelerated compositors expose as dark/cyan crescents.
            // Keep the complete rounded stroke inside the command bounds.
            const inset = width / 2;
            const bounded = {
                x: rect.x + inset,
                y: rect.y + inset,
                width: Math.max(0, rect.width - width),
                height: Math.max(0, rect.height - width),
            };
            roundedPath(
                this.context,
                bounded,
                Math.max(0, (Number(command.radius) || 0) - inset),
            );
        }
        this.context.stroke();
    }

    #ellipsizedText(text, available) {
        if (this.context.measureText(text).width <= available) return text;
        const ellipsis = "…";
        if (this.context.measureText(ellipsis).width > available) return "";
        const characters = graphemes(text);
        let low = 0;
        let high = characters.length;
        const preserveTail = this.#preservesTail(text);
        while (low < high) {
            const middle = Math.ceil((low + high) / 2);
            const candidate = preserveTail
                ? `${ellipsis}${characters.slice(-middle).join("")}`
                : `${characters.slice(0, middle).join("")}${ellipsis}`;
            if (this.context.measureText(candidate).width <= available) {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        if (low === 0) return ellipsis;
        return preserveTail
            ? `${ellipsis}${characters.slice(-low).join("")}`
            : `${characters.slice(0, low).join("")}${ellipsis}`;
    }

    #preservesTail(text) {
        const value = String(text);
        return /^[/\\]/u.test(value) ||
            /^[a-z][a-z\d+.-]*:\/\/\S+$/iu.test(value) ||
            /^[a-z]:[/\\]/iu.test(value) ||
            /^\S+[/\\]\S+$/u.test(value) ||
            /^[^/\\\n]+\.[^\s./\\]{1,16}$/u.test(value);
    }
}

function formatByteCount(value) {
    const bytes = Number(value);
    if (!Number.isFinite(bytes) || bytes < 0) return "0 B";
    if (bytes < 1024) return `${Math.floor(bytes)} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

function renderNativeColorPicker(context, {rect, color, hue, hueDegrees}) {
    const [r,g,b] = color.slice(0,3).map(c=>c/255);
    const max=Math.max(r,g,b), min=Math.min(r,g,b), d=max-min;
    let h=d===0?0:max===r?((g-b)/d+6)%6:max===g?(b-r)/d+2:(r-g)/d+4;
    h = Number.isFinite(hueDegrees) ? hueDegrees/360 : h/6; const saturation=max===0?0:d/max;
    context.save();
    roundedPath(context,rect,hue?rect.height/2:10);context.clip();
    if(hue){
        const rainbow=context.createLinearGradient(rect.x,0,rect.x+rect.width,0);
        ['#ff0000','#ffff00','#00ff00','#00ffff','#0000ff','#ff00ff','#ff0000'].forEach((c,i)=>rainbow.addColorStop(i/6,c));
        context.fillStyle=rainbow;context.fillRect(rect.x,rect.y,rect.width,rect.height);
    }else{
        context.fillStyle=`hsl(${h*360} 100% 50%)`;context.fillRect(rect.x,rect.y,rect.width,rect.height);
        const white=context.createLinearGradient(rect.x,0,rect.x+rect.width,0);white.addColorStop(0,'#ffffff');white.addColorStop(1,'#ffffff00');
        context.fillStyle=white;context.fillRect(rect.x,rect.y,rect.width,rect.height);
        const black=context.createLinearGradient(0,rect.y,0,rect.y+rect.height);black.addColorStop(0,'#00000000');black.addColorStop(1,'#000000');
        context.fillStyle=black;context.fillRect(rect.x,rect.y,rect.width,rect.height);
    }
    context.restore();context.save();
    const x=rect.x+(hue?h:saturation)*(rect.width-1),y=rect.y+(hue?.5:1-max)*(rect.height-1);
    context.beginPath();context.arc(Math.max(rect.x+6,Math.min(rect.x+rect.width-6,x)),Math.max(rect.y+6,Math.min(rect.y+rect.height-6,y)),hue?8:6,0,2*Math.PI);
    context.strokeStyle='#00000070';context.lineWidth=4;context.stroke();context.strokeStyle='#ffffff';context.lineWidth=2;context.stroke();context.restore();
}
