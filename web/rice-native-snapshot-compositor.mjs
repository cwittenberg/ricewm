import { t } from "./rice-i18n.mjs";
import {
    riceColorChannels,
} from "./public-theme.mjs";
import {
    nativeChromeTheme,
    nativeResizeHandles,
} from "./rice-native-window-stack.mjs";

const WINDOW_ID = 1n;

// These actions name physical directions; RTL must not swap their meaning.
function tileActionLabel(action) {
    switch (action) {
        case "expand": return t("window.expand-restore");
        case "grow": return t("window.grow-width");
        case "shrink": return t("window.shrink-width");
        case "left": return t("window.move-left");
        case "right": return t("window.move-right");
        case "up": return t("window.move-up");
        case "down": return t("window.move-down");
        default: return null;
    }
}


export function composeNativeWindowStack({
    windows,
    bridge,
    mediaProvider,
    theme,
    desktopSettings,
    shellSettings,
    dockRevealed,
    dragVisual = null,
    snapPreview = null,
    hoveredTitleControl = null,
    shortcutLabels = {},
    tileControlAvailability = null,
    dialogWorkArea = null,
    pressedTitleControl = null,
    foregroundActive = true,
    includeHidden = false,
    allocateTaskbarId = null,
    activeWindow = windows.findLast(window => !window.minimized) || null,
}) {
    const snapshotWindows = includeHidden
        ? windows
        : windows.filter(window => !window.minimized);
    const snapshots = snapshotWindows.map(window => {
        const { frame } = window;
        if (dialogWorkArea) bridge.setDialogWorkArea?.(window.app, window.window || WINDOW_ID, dialogWorkArea(window));
        const snapshot = bridge.render(
            window.app,
            window.window || WINDOW_ID,
            frame.width,
            frame.contentHeight,
        );
        window.popup = snapshot.popup;
        if (
            snapshot.dialog &&
            !window.dialogTaskbarId &&
            typeof allocateTaskbarId === "function"
        ) {
            window.dialogTaskbarId = allocateTaskbarId();
        }
        const commandStart = snapshot.popup?.commandStart;
        const baseCommands = Number.isInteger(commandStart)
            ? snapshot.commands.slice(0, commandStart)
            : snapshot.commands;
        const popupCommands = Number.isInteger(commandStart)
            ? snapshot.commands.slice(commandStart)
            : [];
        const adaptCommand = command => {
            if (command.type === "text" && shellSettings.largeText) {
                return Object.freeze({
                    ...command,
                    sizePx: Math.min(
                        64,
                        Math.max(8, Math.round(command.sizePx * 1.25)),
                    ),
                });
            }
            return command.type === "media"
                ? Object.freeze({
                    ...command,
                    surface: mediaProvider?.surface(command.request) || null,
                })
                : command;
        };
        const commands = baseCommands.map(adaptCommand);
        const chrome = theme ? nativeChromeTheme(theme) : null;
        const tiled = !!window.autoTileRestoreFrame;
        const tileControls = tiled ? tileControlAvailability?.(window) || {} : {};
        const showTileControls = tiled && (tileControlAvailability ? Object.values(tileControls).some(Boolean) : true);
        const hovered = hoveredTitleControl?.app === window.app ? hoveredTitleControl.action : null;
        const operation = hovered?.replace(/^tile-/, '');
        const command = operation==='expand'?'window.resize.full-width':operation==='grow'?'window.resize.wider':operation==='shrink'?'window.resize.narrower':`window.swap.${operation}`;
        const shortcut = shortcutLabels[command]?.split(' · ').at(-1) || '';
        const actionLabel = tileActionLabel(operation);
        const shortcutLabel = shortcut || t("window.unassigned");
        const availability = tileControls[operation] === false
            ? t("window.unavailable") : `“${shortcutLabel}”`;
        const titleControlTooltip = !tiled || !hovered ? null
            : hovered === "close" ? `${t('icons.window-close')} · “Alt+F4”`
            : actionLabel === null ? null : `${actionLabel} · ${availability}`;
        return Object.freeze({
            ...snapshot,
            commands: Object.freeze(commands),
            popupCommands: Object.freeze(popupCommands.map(adaptCommand)),
            app: snapshot.app.toString(),
            window: snapshot.window.toString(),
            title: window.title,
            entrypoint: window.entrypoint,
            applicationId: window.applicationId,
            iconName: window.iconName || "",
            role: window.role,
            dialogOnly: Boolean(window.dialogOnly),
            titleTools: Object.freeze([...(window.titleTools || [])]),
            active: foregroundActive && window === activeWindow,
            workspace: window.workspace,
            taskbarId: window.taskbarId,
            dialogTaskbarId: snapshot.dialog
                ? window.dialogTaskbarId || 0
                : 0,
            maximized: window.maximized,
            fullscreen: Boolean(window.fullscreen),
            minimized: Boolean(window.minimized),
            layerVisible: includeHidden
                ? !window.minimized
                : true,
            frame: Object.freeze({ ...frame }),
            layoutTransition: window.layoutTransition || null,
            tileClip: window.tileClip || null,
            resizeHandles: window.resizable !== false && window.role !== "desktop" && !window.dialogOnly && !window.maximized && !window.fullscreen
                ? nativeResizeHandles(frame) : [],
            chrome: chrome ? {...chrome,windowDecorations:{...chrome.windowDecorations,tiled,showTileControls,tileControls,maximizable:window.maximizable!==false}} : null,
            tiled, titleControlTooltip,
            hoveredControl: hoveredTitleControl?.app === window.app &&
                    hoveredTitleControl.action
                ? hoveredTitleControl.action
                : null,
            pressedControl: pressedTitleControl?.app === window.app &&
                    pressedTitleControl.action
                ? pressedTitleControl.action
                : null,
            desktopBackground: window.role === "desktop"
                ? Object.freeze({
                    mode: window.desktopMode || desktopSettings.mode,
                    ...(desktopSettings.scene ? {scene:{...desktopSettings.scene,request:String(desktopSettings.scene.request)}} : {}),
                    type: desktopSettings.scene ? (desktopSettings.scene.type==='video'?'Video':'Image') : desktopSettings.type || "Gradient",
                    ...(desktopSettings.type === "Video" ? {
                        videoSession:
                            desktopSettings.videoSession?.toString?.() || "0",
                    } : {}),
                    color: riceColorChannels(desktopSettings.color),
                    style: desktopSettings.style || "Solid",
                    gradient: Object.freeze({
                        start: riceColorChannels(
                            desktopSettings.gradient?.start || desktopSettings.color,
                        ),
                        middle: riceColorChannels(
                            desktopSettings.gradient?.middle || desktopSettings.color,
                        ),
                        end: riceColorChannels(
                            desktopSettings.gradient?.end || desktopSettings.color,
                        ),
                        accent: riceColorChannels(
                            desktopSettings.gradient?.accent || desktopSettings.color,
                        ),
                        enabled: Object.freeze({
                            start: desktopSettings.gradientEnabled?.start !== false,
                            middle: desktopSettings.gradientEnabled?.middle !== false,
                            end: desktopSettings.gradientEnabled?.end !== false,
                            accent: desktopSettings.gradientEnabled?.accent !== false,
                        }),
                    }),
                })
                : null,
            shell: Object.freeze({
                ...shellSettings,
                dockRevealed,
                dockEntries: Object.freeze(windows
                    .filter(candidate => candidate.role !== "desktop")
                    .map(candidate => ({
                        app: candidate.app.toString(),
                        title: candidate.title,
                        role: candidate.role,
                        minimized: candidate.minimized,
                    }))),
                motionEnabled:
                    shellSettings.animations &&
                    !shellSettings.reduceMotion,
            }),
        });
    });
    const active = snapshots.find(snapshot =>
        snapshot.app === activeWindow?.app?.toString()
    ) || snapshots.at(-1);
    if (!active) return null;
    const hovered = snapshots.findLast(snapshot =>
        snapshot.layerVisible !== false &&
        snapshot.cursor && snapshot.cursor !== "default"
    );
    return Object.freeze({
        ...active,
        // X11 and host-native applications share one desktop. Foreground
        // ownership controls which retained cohort is painted on top and
        // receives overlapping input; it must never make the other cohort
        // disappear from the workspace.
        foregroundActive: Boolean(foregroundActive),
        cursor: hoveredTitleControl?.cursor || (hoveredTitleControl?.action
            ? "pointer"
            : hovered?.cursor || active.cursor),
        schema: "rice-native-window-stack-v1",
        activeApp: active.app,
        dragVisual: dragVisual ? Object.freeze({ ...dragVisual }) : null,
        snapPreview: snapPreview ? Object.freeze({ ...snapPreview }) : null,
        windows: Object.freeze(snapshots),
    });
}
