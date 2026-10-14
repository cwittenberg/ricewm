import {riceCompactFormFactor} from './rice-form-factor.mjs';
import { isFrostedIceTheme } from "./rice-frosted-ice.mjs";

const TITLE_HEIGHT = 28;
const MIN_CONTENT_WIDTH = 320;
const MIN_CONTENT_HEIGHT = 240;
const DEFAULT_CONTENT_WIDTH = 960;
const DEFAULT_CONTENT_HEIGHT = 640;
// Keep resize targeting generous on the client side while allowing only a
// narrow reach beyond the pixels that actually paint the window frame. A
// symmetric outside band made the desktop cursor appear detached from every
// edge, especially at corners and with a scaled hardware cursor plane.
const RESIZE_EDGE_INSIDE = 7;
const RESIZE_EDGE_OUTSIDE = 2;
const TITLE_CONTROL_COUNT = 3;
const TITLE_CONTROL_BUTTON_SIZE = 20;
const TITLE_CONTROL_GLYPH_SIZE = 8;
const SNAP_EDGE_THRESHOLD = 8;

function rgbChannels(value, alpha = 255) {
    return [
        (value >>> 16) & 0xff,
        (value >>> 8) & 0xff,
        value & 0xff,
        alpha,
    ];
}

function contrastingChannels(value) {
    const red = (value >>> 16) & 0xff;
    const green = (value >>> 8) & 0xff;
    const blue = value & 0xff;
    return red * 299 + green * 587 + blue * 114 >= 150_000
        ? [10, 8, 13, 255]
        : [255, 255, 255, 255];
}

function mixChannels(channels, target, amount) {
    return channels.map((channel, index) => index === 3
        ? channel
        : Math.round(channel + (target - channel) * amount));
}

export function nativeChromeTheme(theme) {
    const frame = theme.frame;
    const frostedIce = isFrostedIceTheme(theme);
    const familiar = theme.file === "classic.theme" ||
        theme.chromeProfile === "familiar";
    const redmond98 = theme.chromeProfile === "redmond-98";
    const configuredRadius = theme.windowCornerRadius ?? theme.cornerRadius ??
        (familiar || redmond98 ? 0 : frostedIce ? 18 : 6);
    const radius = Math.max(
        0,
        Math.min(32, Math.floor(Number(configuredRadius) || 0)),
    );
    const chromeProfile = theme.chromeProfile || "standard";
    const decorations = {...theme.windowDecorations,...theme.bundle?.windowDecorations};
    const artworkEnabled = decorations.decorationArtwork === "On" ||
        (decorations.decorationArtwork !== "Off" && theme.decorationEnabled !== false);
    const themedButtonStyle = ["default", "wedged", "pill", "chamfered"].includes(
        theme.buttonStyle,
    ) ? theme.buttonStyle[0].toUpperCase() + theme.buttonStyle.slice(1) : null;
    const buttonStyle = ["Default", "Wedged", "Pill", "Chamfered", "Borderless"].includes(
        decorations.buttonStyle,
    ) ? decorations.buttonStyle : themedButtonStyle ||
        (chromeProfile === "neon-grid" ? "Wedged" :
            chromeProfile === "command-deck" ? "Pill" : "Default");
    const themedWindowShape = theme.windowShape === "wedged"
        ? "Wedged"
        : "Default";
    const windowShape = ["Default", "Wedged"].includes(
        decorations.windowShape,
    ) ? decorations.windowShape : themedWindowShape;
    return Object.freeze({
        active: rgbChannels(frame[0], frostedIce ? 160 : 255),
        inactive: rgbChannels(frame[1], frostedIce ? 140 : 255),
        border: rgbChannels(frame[2], frostedIce ? 110 : 255),
        body: rgbChannels(frame[3], frostedIce ? 150 : 255),
        text: rgbChannels(frame[4]),
        control: rgbChannels(frame[5], frostedIce ? 120 : 255),
        close: rgbChannels(frame[6]),
        pressed: rgbChannels(frame[7], frostedIce ? 180 : 255),
        icon: rgbChannels(Number.isInteger(theme.icon) ? theme.icon : frame[4]),
        iconHover: rgbChannels(
            Number.isInteger(theme.iconHover)
                ? theme.iconHover
                : Number.isInteger(theme.icon)
                    ? theme.icon
                    : frame[4],
        ),
        iconVariant: theme.iconVariant === "color" ? "color" : "monochrome",
        iconOverrides: Object.freeze({ ...(theme.iconOverrides || {}) }),
        iconThemeName: String(theme.iconThemeName ?? theme.name ?? ""),
        selection: rgbChannels(
            Array.isArray(theme.menu) && Number.isInteger(theme.menu[3])
                ? theme.menu[3]
                : frame[0],
        ),
        selectionText: contrastingChannels(
            Array.isArray(theme.menu) && Number.isInteger(theme.menu[3])
                ? theme.menu[3]
                : frame[0],
        ),
        radius,
        ...(decorations.frameBorderWidth!==undefined?{frameBorderWidth:decorations.frameBorderWidth}:{}),
        ...Object.fromEntries(['typography','metrics','controlColors','surfaceSkins'].filter(key=>theme.bundle?.[key]!==undefined).map(key=>[key,theme.bundle[key]])),
        ...(theme.bundle?.icons?{themeIcons:theme.bundle.icons}:{}),
        chromeProfile,
        ...(theme.sidebarMaterial === "glass" ? {
            glassTitlebar: true,
            glassTint: rgbChannels(
                Array.isArray(theme.menu) && Number.isInteger(theme.menu[0])
                    ? theme.menu[0]
                    : frame[0],
            ),
            glassText: rgbChannels(
                Array.isArray(theme.menu) && Number.isInteger(theme.menu[5])
                    ? theme.menu[5]
                    : frame[4],
            ),
        } : {}),
        ...(
            artworkEnabled &&
            (/^[a-z0-9]+(?:-[a-z0-9]+)*\.png$/.test(theme.decorationAsset || "") ||
                (theme.decorationAsset?.startsWith("/") && theme.decorationSource?.startsWith("data:image/png;base64,"))) &&
            theme.decorationLayout === "hud-frame"
                ? { decoration: Object.freeze({
                    asset: theme.decorationAsset,
                    ...(theme.decorationSource ? { source: theme.decorationSource } : {}),
                    layout: "hud-frame",
                    opacity: Math.max(0, Math.min(
                        1,
                        (Number(theme.decorationOpacity) || 0) / 100,
                    )),
                }) }
                : {}
        ),
        material: frostedIce ? "frosted-ice" : "solid",
        buttonStyle,
        windowShape,
        ...(familiar ? {
            familiar: true,
            controlText: rgbChannels(0x000000),
            controlLight: mixChannels(rgbChannels(frame[5]), 255, .62),
            controlShadow: mixChannels(rgbChannels(frame[5]), 0, .48),
        } : {}),
        ...(redmond98 ? {
            redmond98: true,
            controlText: rgbChannels(0x000000),
            controlLight: rgbChannels(0xffffff),
            controlHighlight: rgbChannels(0xdfdfdf),
            controlShadow: rgbChannels(0x808080),
            controlDarkShadow: rgbChannels(0x000000),
            desktop: rgbChannels(0x008080),
            activeTitleEnd: rgbChannels(0x1084d0),
            inactiveTitleEnd: rgbChannels(0xb5b5b5),
        } : {}),
        windowDecorations: Object.freeze({
            ...decorations,
            compactTitlebar:
                decorations.compactTitlebar === true ||
                chromeProfile === "yellow-jacket",
            compactTitlebarFullWidthBackdrop:
                decorations.compactTitlebarFullWidthBackdrop === true,
            controlBoxPosition: ["Left", "Right"].includes(
                decorations.controlBoxPosition,
            ) ? decorations.controlBoxPosition : "Theme Default",
            controlLayout: decorations.controlLayout === "split" ||
                theme.controlLayout === "split" ? "split" : "grouped",
            ricewmMenuBorder: Number.isFinite(
                Number(decorations.ricewmMenuBorder),
            )
                ? Math.max(0, Math.min(
                    100,
                    Number(decorations.ricewmMenuBorder),
                ))
                : 50,
            titlebarEnd: ["Straight", "Inclined", "Rounded"].includes(
                decorations.titlebarEnd,
            )
                ? decorations.titlebarEnd
                : "Straight",
            decorationArtwork: decorations.decorationArtwork || "Theme Default",
            buttonStyle,
            windowShape,
            controlBoxEdgeInset:
                chromeProfile === "neon-grid" && artworkEnabled
                    ? 72
                    : redmond98 ? 3 : 0,
            chromeProfile: decorations.chromeProfile || chromeProfile,
        }),
        ...(frostedIce ? {
            frostedIce: Object.freeze({
                thickness: 58,
                opacity: 42,
                blur: 62,
                refraction: 52,
                chroma: 18,
                widgetBorder: 0,
                windowBorder: 100,
                windowContentPointerLight: true,
                ...(theme.frostedIce || {}),
            }),
        } : {}),
    });
}

function integer(value) {
    return Math.floor(Number(value));
}

export function boundedViewport(width, height, fallback = {}) {
    const bounded = (value, fallbackValue) => {
        const number = integer(value);
        return Number.isFinite(number)
            ? Math.max(64, Math.min(16_384, number))
            : fallbackValue;
    };
    return {
        width: bounded(width, fallback.width || 1024),
        height: bounded(height, fallback.height || 768),
        ...(typeof fallback.touch === "boolean" ? {touch: fallback.touch} : {}),
    };
}

export function defaultNativeFrame(viewport, cascade = 0) {
    const width = Math.min(
        DEFAULT_CONTENT_WIDTH,
        Math.max(MIN_CONTENT_WIDTH, viewport.width - 64),
    );
    const contentHeight = Math.min(
        DEFAULT_CONTENT_HEIGHT,
        Math.max(MIN_CONTENT_HEIGHT, viewport.height - 112),
    );
    const centered = {
        x: Math.max(0, Math.floor((viewport.width - width) / 2)),
        y: Math.max(0, Math.floor(
            (viewport.height - contentHeight - TITLE_HEIGHT) / 2,
        )),
        width,
        height: contentHeight + TITLE_HEIGHT,
    };
    const offset = (cascade % 8) * 24;
    centered.x += offset;
    centered.y += offset;
    return constrainNativeFrame(centered, viewport);
}

export function welcomeNativeFrame(viewport, workArea = viewport) {
    const areaX = integer(workArea.x ?? 0);
    const areaY = integer(workArea.y ?? 0);
    const areaWidth = Math.max(1, integer(workArea.width ?? viewport.width));
    const areaHeight = Math.max(1, integer(workArea.height ?? viewport.height));
    const width = Math.min(1_400, Math.max(MIN_CONTENT_WIDTH, Math.round(areaWidth * .8)));
    const height = Math.min(900, Math.max(MIN_CONTENT_HEIGHT + TITLE_HEIGHT, Math.round(areaHeight * .8)));
    return constrainNativeFrame({
        x: areaX + Math.floor((areaWidth - width) / 2),
        y: areaY + Math.floor((areaHeight - height) / 2),
        width,
        height,
    }, viewport, "", workArea);
}

export function calculatorNativeFrame(viewport, cascade = 0, compactClassic = false) {
    const width = Math.min(compactClassic ? 260 : 440, viewport.width);
    const height = Math.min(compactClassic ? 252 : 560, viewport.height);
    const offset = (cascade % 8) * 24;
    return constrainNativeFrame({
        x: Math.max(0, Math.floor((viewport.width - width) / 2)) + offset,
        y: Math.max(0, Math.floor((viewport.height - height) / 2)) + offset,
        width,
        height,
    }, viewport, "", viewport,
        compactClassic ? 240 : MIN_CONTENT_WIDTH,
        compactClassic ? 230 : MIN_CONTENT_HEIGHT + TITLE_HEIGHT,
    );
}

export function constrainNativeFrame(
    frame,
    viewport,
    anchoredEdges = "",
    workArea = viewport,
    minimumWidth = MIN_CONTENT_WIDTH,
    minimumHeight = MIN_CONTENT_HEIGHT + TITLE_HEIGHT,
) {
    const minX = Math.max(0, integer(workArea.x ?? 0));
    const minY = Math.max(0, integer(workArea.y ?? 0));
    const maxWidth = Math.min(
        viewport.width - minX,
        Math.max(1, integer(workArea.width ?? viewport.width - minX)),
    );
    const maxHeight = Math.min(
        viewport.height - minY,
        Math.max(1, integer(workArea.height ?? viewport.height - minY)),
    );
    const width = Math.max(
        Math.min(Math.max(1, integer(minimumWidth)), maxWidth),
        Math.min(maxWidth, integer(frame.width)),
    );
    const height = Math.max(
        Math.min(Math.max(1, integer(minimumHeight)), maxHeight),
        Math.min(maxHeight, integer(frame.height)),
    );
    let x = integer(frame.x);
    let y = integer(frame.y);
    if (anchoredEdges.includes("l")) {
        x = integer(frame.x + frame.width - width);
    }
    if (anchoredEdges.includes("t")) {
        y = integer(frame.y + frame.height - height);
    }
    const maxX = minX + maxWidth - width;
    const maxY = minY + maxHeight - height;
    x = Math.max(minX, Math.min(maxX, x));
    y = Math.max(minY, Math.min(maxY, y));
    return {
        x,
        y,
        width,
        height,
        contentHeight: height - TITLE_HEIGHT,
        titleHeight: TITLE_HEIGHT,
    };
}

export function maximizedNativeFrame(workArea) {
    return {
        x: workArea.x ?? 0,
        y: workArea.y ?? 0,
        width: workArea.width,
        height: workArea.height,
        contentHeight: Math.max(0, workArea.height - TITLE_HEIGHT),
        titleHeight: TITLE_HEIGHT,
    };
}

export function nativeSnapTarget(x, y, viewport, workArea = viewport) {
    const pointerX = integer(x);
    const pointerY = integer(y);
    if (!Number.isFinite(pointerX) || !Number.isFinite(pointerY)) return null;
    const left = integer(workArea.x ?? 0);
    const top = integer(workArea.y ?? 0);
    const right = left + integer(workArea.width ?? viewport.width) - 1;
    if (pointerY <= top + SNAP_EDGE_THRESHOLD) return "top";
    if (pointerX <= left + SNAP_EDGE_THRESHOLD) return "left";
    if (pointerX >= right - SNAP_EDGE_THRESHOLD) return "right";
    return null;
}

export function snappedNativeFrame(target, workArea) {
    if (target === "top") return maximizedNativeFrame(workArea);
    const leftWidth = Math.floor(workArea.width / 2);
    const width = target === "left"
        ? leftWidth
        : workArea.width - leftWidth;
    return {
        x: target === "left"
            ? workArea.x ?? 0
            : (workArea.x ?? 0) + leftWidth,
        y: workArea.y ?? 0,
        width,
        height: workArea.height,
        contentHeight: Math.max(0, workArea.height - TITLE_HEIGHT),
        titleHeight: TITLE_HEIGHT,
    };
}

export function snapNativeWindow(
    window,
    target,
    workArea,
    restoreFrame = window.frame,
) {
    if (!target) return false;
    window.automaticFrame = null;
    if (!window.restoreFrame) window.restoreFrame = { ...restoreFrame };
    window.frame = snappedNativeFrame(target, workArea);
    window.maximized = target === "top";
    window.snapTarget = target === "top" ? null : target;
    applyNativeTitlebarVisibility(window);
    return true;
}

export function desktopNativeFrame(viewport, workArea = viewport) {
    const x = Math.max(0, integer(workArea.x ?? 0));
    const y = Math.max(0, integer(workArea.y ?? 0));
    const width = Math.max(0, Math.min(
        viewport.width - x,
        integer(workArea.width ?? viewport.width - x),
    ));
    const height = Math.max(0, Math.min(
        viewport.height - y,
        integer(workArea.height ?? viewport.height - y),
    ));
    return {
        x,
        y,
        width,
        height,
        contentHeight: height,
        titleHeight: 0,
    };
}

export function nativeActiveWindowSummary(active, windowId) {
    if (!active) return null;
    return Object.freeze({
        app: active.app,
        window: windowId,
        entrypoint: active.entrypoint,
        title: active.title,
        frame: Object.freeze({ ...active.frame }),
    });
}

export function nativeWindowSummaries(windows, windowId) {
    return Object.freeze(windows.map(window => Object.freeze({
        app: window.app,
        window: windowId,
        entrypoint: window.entrypoint,
        title: window.title,
        role: window.role,
        workspace: window.workspace,
        taskbarId: window.taskbarId,
        frame: Object.freeze({ ...window.frame }),
        maximized: window.maximized,
        minimized: window.minimized,
        fullscreen: window.fullscreen,
    })));
}

function estimatedTitleWidth(title) {
    const width = [...String(title || "RiceWM")].reduce((total, character) => {
        if (/\s/u.test(character)) return total + 4;
        if (/[ilI1|.,'`:;]/u.test(character)) return total + 4.25;
        if (/[MW@#%&]/u.test(character)) return total + 10;
        return total + (character.codePointAt(0) > 0xff ? 12 : 7.25);
    }, 0);
    return Math.max(28, Math.min(280, Math.ceil(width)));
}

export function nativeControlBoxPosition(decorations = {}) {
    if (["Left", "Right"].includes(decorations?.controlBoxPosition)) {
        return decorations.controlBoxPosition;
    }
    return ["orchard", "yellow-jacket"].includes(decorations?.chromeProfile)
        ? "Left"
        : "Right";
}

function captionMetrics(frame, decorations) {
    const legacy = decorations.tiled ? Math.min(24,frame.titleHeight) : decorations.chromeProfile==='redmond-98'?18:frame.titleHeight;
    const width = decorations.tiled ? legacy : decorations.captionWidth ?? legacy;
    return {width,height:Math.min(frame.titleHeight,decorations.tiled?frame.titleHeight:decorations.captionHeight??frame.titleHeight),gap:decorations.tiled?0:decorations.captionGap??0};
}
function tiledControlCount(frame, title = "RiceWM", decorations = {}) {
    const titleReserve=Math.max(72,Math.min(160,estimatedTitleWidth(title)));
    const expandedControls = decorations.tileControls?.expand === true;
    const count = expandedControls ? 8 : 7;
    return frame.width >= titleReserve + 44 + count*Math.min(24,frame.titleHeight) ? count : expandedControls ? 2 : 1;
}

export function nativeTitlebarLayout(
    frame,
    title = "RiceWM",
    tools = [],
    decorations = {},
) {
    const profile = decorations?.chromeProfile || "standard";
    const controlsLeading = nativeControlBoxPosition(decorations) === "Left";
    const splitControls = !decorations.tiled && decorations?.controlLayout === "split";
    const compact = decorations?.compactTitlebar === true ||
        profile === "yellow-jacket";
    const end = ["Straight", "Inclined", "Rounded"]
        .includes(decorations?.titlebarEnd)
        ? decorations.titlebarEnd
        : "Straight";
    const toolCount = Array.isArray(tools) ? Math.min(6, tools.length) : 0;
    const controlCount = decorations.tiled ? decorations.showTileControls===false ? 1 : tiledControlCount(frame,title,decorations) : profile === "yellow-jacket" ? 1 : TITLE_CONTROL_COUNT;
    const metrics=captionMetrics(frame,decorations);
    const controlPitch = metrics.width + metrics.gap;
    const trailingControlCount = splitControls ? 2 : controlsLeading ? 0 : controlCount;
    const endWidth = compact ? end === "Straight" ? 7 : 18 : 0;
    const iconX = splitControls
        ? 4 + controlPitch + 6
        : controlsLeading
        ? 4 + controlPitch * controlCount + 6
        : 7;
    const titleX = iconX + 24;
    const naturalWidth = titleX + estimatedTitleWidth(title) + 8 +
        frame.titleHeight * toolCount + controlPitch * trailingControlCount +
        endWidth + (profile === "yellow-jacket" ? 26 : 0);
    const width = compact
        ? Math.max(1, Math.min(frame.width, naturalWidth))
        : frame.width;
    const edgeInset = controlsLeading
        ? 0
        : Math.max(0, Number(decorations?.controlBoxEdgeInset) || 0);
    const controlsX = controlsLeading && !splitControls
        ? 4
        : width - endWidth - edgeInset - controlPitch *
            (splitControls ? 2 : controlCount);
    const toolsX = controlsLeading && !splitControls
        ? width - endWidth - frame.titleHeight * toolCount
        : controlsX - frame.titleHeight * toolCount;
    return Object.freeze({
        compact,
        width,
        end,
        endWidth,
        titleX,
        titleWidth: Math.max(1, toolsX - titleX - 4),
        controlsX,
        leadingControlsX: splitControls ? 4 : controlsX,
        splitControls,
        toolsX,
    });
}

export function nativeTitlebarContains(
    frame,
    x,
    y,
    title = "RiceWM",
    tools = [],
    decorations = {},
) {
    const layout = nativeTitlebarLayout(frame, title, tools, decorations);
    return frame.titleHeight > 0 &&
        x >= frame.x && x < frame.x + layout.width &&
        y >= frame.y && y < frame.y + frame.titleHeight;
}

export function nativeTitleIconX(decorations = {}, frame = {width:1000,titleHeight:28}, title = "RiceWM") {
    return nativeTitlebarLayout(frame,title,[],decorations).titleX - 24;
}

export function nativeTitleIconContains(frame, x, y, decorations = {}, title = "RiceWM") {
    const iconX = nativeTitleIconX(decorations,frame,title);
    return frame.titleHeight > 0 &&
        x >= frame.x + iconX && x < frame.x + iconX + 26 &&
        y >= frame.y && y < frame.y + frame.titleHeight;
}

export function nativeTitleControl(
    frame,
    x,
    y,
    title = "RiceWM",
    tools = [],
    decorations = {},
) {
    return nativeTitleControlLayout(frame, title, tools, decorations)
        .find(control =>
            x >= frame.x + control.cell.x &&
            x < frame.x + control.cell.x + control.cell.width &&
            y >= frame.y + control.cell.y && y < frame.y + control.cell.y + control.cell.height
        )?.action || null;
}

export function nativeTitleControlLayout(
    frame,
    title = "RiceWM",
    tools = [],
    decorations = {},
) {
    const profile = decorations?.chromeProfile || "standard";
    if (frame.titleHeight <= 0) return [];
    const metrics=captionMetrics(frame,decorations);
    const cellSize = metrics.width + metrics.gap;
    if (cellSize <= 0) return [];
    const buttonSize = Math.min(
        TITLE_CONTROL_BUTTON_SIZE,
        Math.max(1, cellSize - 4),
    );
    const redmond98 = !decorations.tiled && profile === "redmond-98";
    const custom = !decorations.tiled && (decorations.captionWidth !== undefined || decorations.captionHeight !== undefined || decorations.buttonStyle === "Borderless");
    const buttonWidth = custom ? metrics.width : redmond98 ? 16 : buttonSize;
    const buttonHeight = custom ? metrics.height : redmond98 ? 14 : buttonSize;
    const glyphSize = Math.min(
        TITLE_CONTROL_GLYPH_SIZE,
        Math.max(1, buttonHeight - 4),
    );
    const titlebar = nativeTitlebarLayout(
        frame, title, tools, decorations,
    );
    const glyphWidth=Math.min(metrics.width,decorations.glyphWidth??glyphSize);
    const glyphHeight=Math.min(metrics.height,decorations.glyphHeight??glyphSize);
    const cellY=Math.floor((frame.titleHeight-metrics.height)/2);
    const firstX = titlebar.controlsX;
    const controlsLeading = nativeControlBoxPosition(decorations) === "Left";
    const tileCount = tiledControlCount(frame,title,decorations);
    const tileActions = tileCount === 2 ? ['expand'] : ['up','down','left','right','grow',...(decorations.tileControls?.expand === true ? ['expand'] : []),'shrink'];
    const controls = decorations.tiled
        ? (decorations.showTileControls===false || tileCount===1 ? [["close",54]] : controlsLeading ? [["close",54],...tileActions.map(id=>["tile-"+id,0])] : [...tileActions.map(id=>["tile-"+id,0]),["close",54]])
        : profile === "yellow-jacket"
        ? [["close", 54]]
        : titlebar.splitControls
        ? [["minimize", 55], ["maximize", 56], ["close", 54]]
        : controlsLeading
        ? [["close", 54], ["minimize", 55], ["maximize", 56]]
        : [["minimize", 55], ["maximize", 56], ["close", 54]];
    return controls.map(([action, icon], index) => {
        const cellX = titlebar.splitControls
            ? index === 0
                ? titlebar.leadingControlsX
                : firstX + cellSize * (index - 1)
            : firstX + cellSize * index;
        return {
            action,
            disabled: (action === 'maximize' && decorations.maximizable === false) || action.startsWith('tile-') && decorations.tileControls?.[action.slice(5)]===false,
            icon,
            cell: {
                x: cellX,
                y: cellY,
                width: metrics.width,
                height: metrics.height,
            },
            button: {
                x: cellX + Math.floor((metrics.width - buttonWidth) / 2),
                y: redmond98
                    ? 6
                    : Math.floor((frame.titleHeight - buttonHeight) / 2),
                width: buttonWidth,
                height: buttonHeight,
            },
            glyph: {
                x: cellX + Math.floor((metrics.width - glyphWidth) / 2),
                y: redmond98
                    ? 6 + Math.floor((buttonHeight - glyphSize) / 2)
                    : Math.floor((frame.titleHeight - glyphHeight) / 2),
                width: glyphWidth,
                height: glyphHeight,
            },
        };
    });
}

export function nativeTitleToolLayout(
    frame,
    tools = [],
    title = "RiceWM",
    decorations = {},
) {
    const cellSize = frame.titleHeight;
    if (cellSize <= 0 || !Array.isArray(tools)) return [];
    const bounded = tools.slice(0, 6);
    const firstX = nativeTitlebarLayout(
        frame, title, bounded, decorations,
    ).toolsX;
    return bounded.map((tool, index) => ({
        action: String(tool.action || ""),
        icon: Math.max(0, Math.min(0xffff, Number(tool.icon) || 0)),
        label: String(tool.label || tool.action || "Action").slice(0, 64),
        cell: {
            x: firstX + cellSize * index,
            y: 0,
            width: cellSize,
            height: cellSize,
        },
        button: {
            x: firstX + cellSize * index + 3,
            y: 3,
            width: Math.max(1, cellSize - 6),
            height: Math.max(1, cellSize - 6),
        },
        glyph: {
            x: firstX + cellSize * index + Math.floor((cellSize - 12) / 2),
            y: Math.floor((cellSize - 12) / 2),
            width: 12,
            height: 12,
        },
    })).filter(tool => tool.action && tool.icon > 0);
}

export function nativeTitleTool(
    frame,
    tools,
    x,
    y,
    title = "RiceWM",
    decorations = {},
) {
    return nativeTitleToolLayout(frame, tools, title, decorations).find(tool =>
        x >= frame.x + tool.cell.x &&
        x < frame.x + tool.cell.x + tool.cell.width &&
        y >= frame.y &&
        y < frame.y + tool.cell.height
    ) || null;
}

export function nativeResizeHandles(frame) {
    const left = frame.x;
    const top = frame.y;
    const right = frame.x + frame.width;
    const bottom = frame.y + frame.height;
    const inside = RESIZE_EDGE_INSIDE;
    const outside = RESIZE_EDGE_OUTSIDE;
    const band = inside + outside;
    const middleWidth = Math.max(0, frame.width - inside * 2);
    const middleHeight = Math.max(0, frame.height - inside * 2);
    return [
        { edges: "lt", rect: {
            x: left - outside, y: top - outside,
            width: band, height: band,
        } },
        { edges: "rt", rect: {
            x: right - inside, y: top - outside,
            width: band, height: band,
        } },
        { edges: "lb", rect: {
            x: left - outside, y: bottom - inside,
            width: band, height: band,
        } },
        { edges: "rb", rect: {
            x: right - inside, y: bottom - inside,
            width: band, height: band,
        } },
        { edges: "l", rect: {
            x: left - outside, y: top + inside,
            width: band, height: middleHeight,
        } },
        { edges: "r", rect: {
            x: right - inside, y: top + inside,
            width: band, height: middleHeight,
        } },
        { edges: "t", rect: {
            x: left + inside, y: top - outside,
            width: middleWidth, height: band,
        } },
        { edges: "b", rect: {
            x: left + inside, y: bottom - inside,
            width: middleWidth, height: band,
        } },
    ];
}

export function nativeResizeEdges(frame, x, y) {
    const handle = nativeResizeHandles(frame).find(({ rect }) =>
        x >= rect.x &&
        x < rect.x + rect.width &&
        y >= rect.y &&
        y < rect.y + rect.height
    );
    return handle?.edges || "";
}

export function nativeResizeCursor(edges) {
    switch (String(edges || "")) {
        case "lt":
        case "rb":
            return "nwse-resize";
        case "rt":
        case "lb":
            return "nesw-resize";
        case "l":
        case "r":
            return "ew-resize";
        case "t":
        case "b":
            return "ns-resize";
        default:
            return "";
    }
}

export function pointInNativeFrame(frame, x, y, includeResize = false) {
    const edge = includeResize ? RESIZE_EDGE_OUTSIDE : 0;
    return (
        x >= frame.x - edge &&
        x < frame.x + frame.width + edge &&
        y >= frame.y - edge &&
        y < frame.y + frame.height + edge
    );
}

export function draggedNativeFrame(
    drag,
    x,
    y,
    viewport,
    workArea = viewport,
) {
    const deltaX = x - drag.startX;
    const deltaY = y - drag.startY;
    const frame = { ...drag.frame };
    if (drag.kind === "move") {
        if (drag.fromMaximized || drag.fromSnapped) {
            frame.x = drag.startX - Math.round(
                frame.width * drag.pointerRatioX,
            ) + deltaX;
            frame.y = drag.startY - drag.pointerOffsetY + deltaY;
        } else {
            frame.x += deltaX;
            frame.y += deltaY;
        }
    } else {
        if (drag.edges.includes("l")) {
            frame.x += deltaX;
            frame.width -= deltaX;
        }
        if (drag.edges.includes("r")) frame.width += deltaX;
        if (drag.edges.includes("t")) {
            frame.y += deltaY;
            frame.height -= deltaY;
        }
        if (drag.edges.includes("b")) frame.height += deltaY;
    }
    return constrainNativeFrame(
        frame,
        viewport,
        drag.edges,
        workArea,
        drag.minimumWidth,
        drag.minimumHeight,
    );
}

export function activeNativeWindow(windows) {
    return windows.findLast(window => !window.minimized && window.focusable !== false) || null;
}

export function topNativeWindow(
    windows,
    x,
    y,
    includeResize = false,
    includeDesktop = true,
) {
    return windows.findLast(window =>
        !window.minimized &&
        window.ignoreMouseEvents !== true &&
        (includeDesktop || window.role !== "desktop") &&
        ((window.dialogOnly || window.dialog?.title?.startsWith("panel:drawer-dialog:"))
            ? Boolean(window.dialog?.rect) && pointInNativeFrame({
                ...window.dialog.rect,
                x: window.frame.x + window.dialog.rect.x,
                y: window.frame.y + window.frame.titleHeight + window.dialog.rect.y,
            }, x, y, false) || (!window.dialogOnly && pointInNativeFrame(window.frame, x, y, includeResize && window.resizable !== false))
            : pointInNativeFrame(window.frame, x, y, includeResize && window.resizable !== false))
    ) || null;
}

export function raiseNativeWindow(windows, window) {
    if (window.role === "desktop") return false;
    const index = windows.findIndex(candidate => candidate.app === window.app);
    if (index < 0) return false;
    windows.splice(index, 1);
    const insertion = window.alwaysOnTop
        ? windows.length
        : windows.findIndex(candidate => candidate.alwaysOnTop === true);
    windows.splice(insertion < 0 ? windows.length : insertion, 0, window);
    return windows.indexOf(window) !== index;
}

export function toggleNativeWindowMaximize(
    window,
    viewport,
    workArea = viewport,
) {
    window.automaticFrame = null;
    if (window.maximized) {
        window.frame = constrainNativeFrame(
            window.restoreFrame || defaultNativeFrame(viewport),
            viewport,
            "",
            workArea,
            window.minimumWidth,
            window.minimumHeight,
        );
        window.restoreFrame = null;
        window.maximized = false;
        window.snapTarget = null;
    } else {
        if (!window.restoreFrame) window.restoreFrame = { ...window.frame };
        window.frame = maximizedNativeFrame(workArea);
        window.maximized = true;
        window.snapTarget = null;
    }
    applyNativeTitlebarVisibility(window);
}

export function dragNativeWindow(
    windows,
    drag,
    x,
    y,
    viewport,
    workArea = viewport,
) {
    const window = windows.find(candidate => candidate.app === drag.app);
    if (!window) return null;
    window.automaticFrame = null;
    if ((drag.fromMaximized || drag.fromSnapped) && (window.maximized || window.snapTarget)) {
        window.maximized = false;
        window.restoreFrame = null;
    }
    window.frame = draggedNativeFrame({
        minimumWidth: window.minimumWidth,
        minimumHeight: window.minimumHeight,
        ...drag,
    }, x, y, viewport, workArea);
    window.snapTarget = null;
    applyNativeTitlebarVisibility(window);
    return window;
}

export function applyNativeTitlebarVisibility(window) {
    if (!window?.frame || window.role === "desktop") return;
    window.frame.titleHeight = window.fullscreen || window.titlebarHidden || window.dialogOnly ? 0 : window.themeTitlebarHeight??TITLE_HEIGHT;
    window.frame.contentHeight = Math.max(
        0,
        window.frame.height - window.frame.titleHeight,
    );
}

export function resizeNativeWindow(window, viewport, workArea = viewport, fullscreenArea = viewport) {
    // Compact presentation is temporary; desktop geometry and state survive rotation.
    const compact = riceCompactFormFactor(viewport);
    const eligible = window.role !== "desktop" && !window.transientPopup && !window.dialogOnly;
    if (compact && eligible && !window.compactRestore) {
        window.compactRestore = {frame: {...window.frame}, restoreFrame: window.restoreFrame && {...window.restoreFrame}};
    } else if (!compact && window.compactRestore) {
        const saved = window.compactRestore;
        window.frame = {...saved.frame};
        window.restoreFrame = saved.restoreFrame && {...saved.restoreFrame};
        if (saved.frame.x >= (workArea.x || 0) && saved.frame.y >= (workArea.y || 0) &&
            saved.frame.x + saved.frame.width <= (workArea.x || 0) + workArea.width &&
            saved.frame.y + saved.frame.height <= (workArea.y || 0) + workArea.height) {
            delete window.compactRestore;
        }
    }
    if (compact && eligible && !window.fullscreen) {
        window.frame = maximizedNativeFrame(workArea);
        applyNativeTitlebarVisibility(window);
        return;
    }

    window.frame = window.role === "desktop"
        ? desktopNativeFrame(viewport, workArea)
        : window.fullscreen
        ? {
            x: fullscreenArea.x || 0,
            y: fullscreenArea.y || 0,
            width: fullscreenArea.width,
            height: fullscreenArea.height,
            contentHeight: Math.max(0, fullscreenArea.height - TITLE_HEIGHT),
            titleHeight: TITLE_HEIGHT,
        }
        : window.maximized
        ? maximizedNativeFrame(workArea)
        : window.snapTarget
        ? snappedNativeFrame(window.snapTarget, workArea)
        : window.automaticFrame === "welcome"
        ? welcomeNativeFrame(viewport, workArea)
        : constrainNativeFrame(
            window.frame,
            viewport,
            "",
            workArea,
            window.minimumWidth,
            window.minimumHeight,
        );
    if (window.restoreFrame && !window.sceneClientFrame) {
        window.restoreFrame = constrainNativeFrame(
            window.restoreFrame,
            viewport,
            "",
            workArea,
            window.minimumWidth,
            window.minimumHeight,
        );
    }
    applyNativeTitlebarVisibility(window);
}

export { TITLE_CONTROL_COUNT, TITLE_HEIGHT };
