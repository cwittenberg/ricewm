export function overviewTheme(chrome = {}) {
    const color=(value,fallback)=>Array.isArray(value)?value.slice(0,3):fallback;
    const css=(value,alpha=1)=>`rgba(${value.join(',')},${alpha})`;
    const body=color(chrome.body,[24,26,29]),text=color(chrome.text,[235,237,239]);
    const accent=color(chrome.selection,[210,214,218]),control=color(chrome.control,[36,39,43]);
    return {
        background:alpha=>css(body,alpha),surface:css(body,.96),control:css(control,.96),
        ink:css(text),muted:css(text,.68),line:css(color(chrome.border,[72,76,81])),
        accent:css(accent),accentWash:css(accent,.16),onAccent:css(color(chrome.selectionText,[12,14,16])),
        radius:Math.max(0,Math.min(23,chrome.radius??10)),
    };
}
