import {readableInk, rgbaColor} from './rice-native-canvas-primitives.mjs';

// Panel controls share ink and hover colors, independently of window toolbars.
export function riceTopbarAppearance(chrome = {}, paper = [36,36,36,255]) {
    const foreground = readableInk(chrome.icon || chrome.text || [242,242,242,255], paper, 3);
    const hoverBackground = chrome.selection || chrome.control || [65,65,65,255];
    return {foreground, hoverBackground,
        hoverForeground: readableInk(chrome.selectionText || foreground, hoverBackground, 3),
        radius: chrome.buttonStyle === 'Pill' ? 16 : Math.min(8, Math.max(0, Number(chrome.radius) || 0))};
}

export function riceTopbarPanelStyles(chrome, paper) {
    const {foreground, hoverBackground, hoverForeground, radius} = riceTopbarAppearance(chrome, paper);
    return `
.panel-button { color:${rgbaColor(foreground)}; padding:0px 8px; margin:2px 0px; border-radius:${radius}px; }
.panel-button:hover, .panel-button:active, .panel-button:focus { background-color:${rgbaColor(hoverBackground)}; color:${rgbaColor(hoverForeground)}; }
.panel-button StIcon { icon-size:16px; }
`;
}
