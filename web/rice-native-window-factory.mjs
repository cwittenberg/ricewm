import {riceCompactFormFactor} from './rice-form-factor.mjs';
import {t} from './rice-i18n.mjs';
import {
    nativeEntrypointTitle,
} from "./rice-native-launch-policy.mjs";
import {
    riceWallpaperModeArgument,
} from "./wallpaper-mode.mjs";
import {
    resizeNativeWindow,
    calculatorNativeFrame,
    constrainNativeFrame,
    defaultNativeFrame,
    desktopNativeFrame,
    welcomeNativeFrame,
} from "./rice-native-window-stack.mjs";

export function createRiceNativeWindow({
    app,
    entrypoint,
    arguments_,
    viewport,
    workArea = viewport,
    index,
    title = null,
    theme = null,
    preferredSize = null,
}) {
    const role = entrypoint === "rice-desktop" ? "desktop" : "window";
    const welcome = entrypoint === "rice-welcome";
    const compactClassicCalculator = entrypoint === "rice-calc" &&
        theme?.chromeProfile === "redmond-98";
    const requestedHostFrame = entrypoint === "rice-host-surface"
        ? hostSurfaceFrame(arguments_, viewport, workArea, index)
        : null;
    const ordinaryFrame = entrypoint === "rice-calc"
        ? calculatorNativeFrame(
            viewport,
            index,
            compactClassicCalculator,
        )
        : requestedHostFrame || defaultNativeFrame(viewport, index);
    for(const [dimension,minimum] of [['width',160],['height',96]]){
        const value=preferredSize?.[dimension];
        if(value!==undefined&&(!Number.isInteger(value)||value<minimum||value>4096))
            throw new RangeError(`Preferred window ${dimension} is invalid`);
    }
    const minimumWidth=preferredSize?.width!==undefined?Math.min(320,preferredSize.width):compactClassicCalculator?240:undefined;
    const minimumHeight=preferredSize?.height!==undefined?Math.min(268,preferredSize.height+ordinaryFrame.titleHeight):compactClassicCalculator?230:undefined;
    const preferredFrame=preferredSize && (preferredSize.width!==undefined||preferredSize.height!==undefined) ? {
        ...ordinaryFrame,
        width:preferredSize.width??ordinaryFrame.width,
        height:(preferredSize.height??ordinaryFrame.contentHeight)+ordinaryFrame.titleHeight,
        x:(workArea.x||0)+Math.floor((workArea.width-(preferredSize.width??ordinaryFrame.width))/2),
        y:(workArea.y||0)+Math.floor((workArea.height-((preferredSize.height??ordinaryFrame.contentHeight)+ordinaryFrame.titleHeight))/2),
    } : ordinaryFrame;
    const fittedOrdinaryFrame = constrainNativeFrame(
        entrypoint === "rice-player" && arguments_?.includes?.("--media-kind=audio")
            ? {...ordinaryFrame, width: 480, height: 360}
            : preferredFrame,
        viewport,
        "",
        workArea,
        minimumWidth,
        minimumHeight,
    );
    const initialTitle = initialNativeTitle(entrypoint, arguments_);
    const composeTitle = entrypoint === "rice-box" ? [
        ["--settings", "mail.window.settings"],
        ["--compose=reply", "mail.compose.reply"],
        ["--compose=forward", "mail.compose.forward"],
        ["--compose=attendees", "mail.compose.attendees"],
        ["--compose=new", "mail.compose.new"],
        ["--reader", "mail.window.reader"],
    ].find(([argument]) => arguments_?.includes?.(argument))?.[1] : null;
    const titleMessage = !title ? composeTitle ||
        (initialTitle === nativeEntrypointTitle(entrypoint) ? nativeEntrypointTitle(entrypoint) : null) : null;
    const window = {
        ...(minimumWidth!==undefined?{minimumWidth}:{}),
        ...(minimumHeight!==undefined?{minimumHeight}:{}),
        app,
        entrypoint,
        arguments: arguments_,
        applicationId: entrypoint === "rice-host-surface"
            ? arguments_.find(value => value.startsWith("--dock-application-id="))?.slice("--dock-application-id=".length) || arguments_.find(value => value.startsWith("--application-id="))?.slice(17) || `host:${arguments_.find(value => value.startsWith("--identity="))?.slice(11) || app}`
            : `native:${entrypoint === "rice-terminal" ? "rice-term" : entrypoint}`,
        iconName: entrypoint === "rice-host-surface"
            ? arguments_.find(value => /^--icon-name=guest-app-[a-z0-9]+(?:-[a-z0-9]+)*-[a-f0-9]{16}$/u.test(value))?.slice("--icon-name=".length) || ""
            : "",
        titleMessage,
        title: title || (theme?.chromeProfile === "redmond-98" && entrypoint === "rice-calc"
            ? "Calculator"
            : theme?.chromeProfile === "redmond-98" && entrypoint === "rice-files"
            ? "Exploring - My Computer"
            : (titleMessage ? t(titleMessage) : initialTitle)),
        role,
        frame: role === "desktop"
            ? desktopNativeFrame(viewport, workArea)
            : welcome
                ? welcomeNativeFrame(viewport, workArea)
                : fittedOrdinaryFrame,
        maximized: false,
        automaticFrame: welcome ? "welcome" : null,
        snapTarget: null,
        minimized: entrypoint === "rice-host-surface" &&
            arguments_?.includes?.("--visible=false"),
        restoreFrame: null,
        fullscreen: false,
        fullscreenRestore: null,
        ...(compactClassicCalculator
            ? { minimumWidth: 240, minimumHeight: 230 }
            : {}),
        desktopMode: role === "desktop"
            ? riceWallpaperModeArgument(arguments_)
            : null,
    };
    if (riceCompactFormFactor(viewport)) resizeNativeWindow(window, viewport, workArea);
    return window;
}

function hostSurfaceFrame(arguments_, viewport, workArea, index) {
    const requested = name => {
        const raw = arguments_?.find?.(argument => String(argument).startsWith(`--${name}=`));
        if (!raw) return null;
        const value = Number(String(raw).slice(name.length + 3));
        return Number.isFinite(value) ? Math.round(value) : null;
    };
    const fallback = defaultNativeFrame(viewport, index);
    const width = Math.max(1, Math.min(8192, requested("width") ?? fallback.width));
    const height = Math.max(29, Math.min(8192, requested("height") ?? fallback.height));
    return constrainNativeFrame({
        x: requested("x") ?? Math.round((workArea.x || 0) + (workArea.width - Math.min(width, workArea.width)) / 2),
        y: requested("y") ?? Math.round((workArea.y || 0) + (workArea.height - Math.min(height, workArea.height)) / 2),
        width,
        height,
    }, viewport, "", workArea);
}

function initialNativeTitle(entrypoint, arguments_) {
    if (entrypoint === "rice-host-surface") {
        const requested = arguments_?.find?.(argument =>
            String(argument).startsWith("--title=")
        );
        const title = String(requested || "").slice(8).trim();
        if (title && !/[\0\r\n]/.test(title)) return title.slice(0, 255);
    }
    if (entrypoint === "rice-box") {
        if (arguments_?.includes?.("--settings")) return "Rice Box settings";
        if (arguments_?.includes?.("--compose=reply")) return "Compose mail — Reply";
        if (arguments_?.includes?.("--compose=forward")) return "Compose mail — Forward";
        if (arguments_?.includes?.("--compose=attendees")) return "Compose mail — Attendees";
        if (arguments_?.includes?.("--compose=new")) return "Compose mail";
        if (arguments_?.includes?.("--reader")) return "Mail message";
    }
    if (entrypoint === "rice-player") {
        const path = String(arguments_?.[1] || "");
        const filename = path.split("/").filter(Boolean).at(-1);
        if (filename) return filename.slice(0, 96);
    }
    if (entrypoint === "rice-paper") {
        const path = arguments_?.find?.(argument =>
            String(argument).startsWith("/")
        );
        const filename = String(path || "").split("/").filter(Boolean).at(-1);
        if (filename) return filename.slice(0, 96);
        const title = arguments_?.find?.(argument =>
            String(argument).startsWith("--title=")
        );
        if (title) return String(title).slice(8, 104) || "Untitled";
    }
    return nativeEntrypointTitle(entrypoint);
}
