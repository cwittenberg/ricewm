function packedRiceColor(value) {
    if (!Number.isInteger(value) || value < 0 || value > 0x00ff_ffff) {
        throw new TypeError("invalid RiceWM theme background color");
    }
    return `#${value.toString(16).padStart(6, "0")}`;
}

function packedRiceChannels(value) {
    const packed = Number.isInteger(value) ? value : 0x091317;
    return [
        (packed >>> 16) & 0xff,
        (packed >>> 8) & 0xff,
        packed & 0xff,
    ];
}

export function themeBackgroundGradient(theme) {
    if (theme?.chromeProfile === "redmond-98") {
        return Object.freeze({
            start: "#008080",
            middle: "#008080",
            end: "#008080",
            accent: "#008080",
        });
    }
    return Object.freeze({
        start: packedRiceColor(theme?.frame?.[3]),
        middle: packedRiceColor(theme?.menu?.[0]),
        end: packedRiceColor(theme?.taskbar?.[4]),
        accent: packedRiceColor(theme?.taskbar?.[6]),
    });
}

export function parseRiceWorkspaceNames(value) {
    const names = String(value || "").split(",").map(name => name.trim());
    return names.length === 4 && names.every(name =>
        name.length > 0 && [...name].length <= 16
    ) ? names : ["1", "2", "3", "4"];
}

export function riceColorChannels(value) {
    const match = /^#([0-9a-f]{6})$/i.exec(String(value));
    if (!match) throw new TypeError("invalid desktop background color");
    const packed = Number.parseInt(match[1], 16);
    return [
        (packed >>> 16) & 0xff,
        (packed >>> 8) & 0xff,
        packed & 0xff,
        255,
    ];
}

