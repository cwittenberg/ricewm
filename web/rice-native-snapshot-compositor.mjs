import {translateMenuCommands,removeApplicationMenuBand} from './rice-application-menu.mjs';
import {applyNativeTitlebarVisibility} from './rice-native-window-stack.mjs';
import {nativeClientRect} from './rice-native-window-stack.mjs';
import {popupSurfaceRects} from './rice-native-canvas-window-damage.mjs';
import { t } from "./rice-i18n.mjs";
import {
    riceColorChannels,
} from "./public-theme.mjs";
import {
    nativeChromeTheme,
    nativeResizeHandles,
    nativeWindowResizeFrame,
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
    const context=activeWindow?.transientPopup?activeWindow.applicationContext:activeWindow;
    const applicationWindow=context&&snapshotWindows.includes(context)&&!context.minimized&&!context.background&&context.role!=='desktop'?context:null;
    const snapshots = snapshotWindows.map(window => {
        applyNativeTitlebarVisibility(window);
        const { frame } = window;
        if (dialogWorkArea) bridge.setDialogWorkArea?.(window.app, window.window || WINDOW_ID, dialogWorkArea(window));
        let snapshot = bridge.render(
            window.app,
            window.window || WINDOW_ID,
            nativeClientRect(frame).width,
            frame.contentHeight + (window.menuPresentationHeight || 0),
        );
        let editorPopupTranslation=null;
        window.browserEditorPopupSize=null;
        if(window.browserEditorMenuAnchor&&snapshot.popup&&dialogWorkArea){
            const normal=snapshot,area=dialogWorkArea(window),expanded=bridge.render(window.app,WINDOW_ID,area.width,area.height);
            if(expanded.popup){
                window.browserEditorPopupSize={width:area.width,height:area.height};
                const menuCommands=expanded.commands.slice(expanded.popup.commandStart);
                const root=popupSurfaceRects(menuCommands,expanded.popup.rect)[0],anchor=window.browserEditorMenuAnchor;
                // Reserve a cascade beside the root from its first paint. The
                // root stays still as the pointer opens/closes child menus.
                const x=Math.max(0,Math.min(anchor.x-(frame.x+nativeClientRect(frame).x+area.x),area.width-Math.min(area.width,root.width*2)));
                const y=Math.max(0,Math.min(anchor.y-(frame.y+frame.titleHeight+area.y),area.height-root.height));
                editorPopupTranslation={x:area.x+x-root.x,y:area.y+y-root.y};
                const base=normal.commands.slice(0,normal.popup.commandStart);
                snapshot={...normal,popup:{...expanded.popup,commandStart:base.length},commands:[...base,...menuCommands]};
            }
        }else if(!snapshot.popup)window.browserEditorMenuAnchor=null;
        const globalMenus = theme?.bundle?.shellAppearance?.menuBar?.mode === 'application' && shellSettings.gnomePanelVisible && !window.fullscreen && window.role !== 'desktop';
        let applicationMenu = globalMenus ? snapshot.applicationMenu : null;
        let menuHeight = applicationMenu?.height || 0;
        if (menuHeight !== (window.menuPresentationHeight || 0)) {
            snapshot = bridge.render(window.app, WINDOW_ID, nativeClientRect(frame).width, frame.contentHeight + menuHeight);
            applicationMenu = globalMenus ? snapshot.applicationMenu : null;
            menuHeight = applicationMenu?.height || 0;
        }
        window.menuPresentationHeight = menuHeight;
        window.menuPresentationTop = applicationMenu?.y || 0;
        window.applicationMenu = applicationMenu;
        let popup = snapshot.popup;
        let popupTranslation = {x:0, y:!popup || popup.rect.y >= window.menuPresentationTop ? -menuHeight : 0};
        if(editorPopupTranslation)popupTranslation=editorPopupTranslation;
        if (popup && window.applicationMenuAnchor && globalMenus && window === activeWindow && foregroundActive) {
            const anchor = window.applicationMenuAnchor;
            anchor.popupOrigin ||= {x:popup.rect.x, y:popup.rect.y};
            popupTranslation = {
                x: anchor.x - frame.x - nativeClientRect(frame).x - anchor.popupOrigin.x,
                y: anchor.y + anchor.height - frame.y - frame.titleHeight - anchor.popupOrigin.y,
            };
        } else if (!popup) window.applicationMenuAnchor = null;
        window.popupPresentationTranslation = popupTranslation;
        if (popup) popup = {...popup, rect:{...popup.rect, x:popup.rect.x+popupTranslation.x, y:popup.rect.y+popupTranslation.y}};
        window.popup = popup;
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
        // Retained Paper engines omit the dark suffix from paged rich text.
        // Use the actual checked native View item (paper::FORCE_DARK_MODE_MENU),
        // retained from accepted patches, rather than duplicating preferences.
        const paperDarkItem = window.entrypoint === 'rice-paper'
            ? bridge.applicationMenus?.windows.get(`${window.app}:${WINDOW_ID}`)?.get('38') : null;
        const paperRichDark = paperDarkItem?.kind === 14 && Boolean(paperDarkItem.state & (1 << 5));
        const adaptCommand = input => {
            let command = input;
            if (paperRichDark && command.type === 'text' && command.role === 'editor' &&
                /^editor:rich(?:~|:)/u.test(command.text)) {
                const end = command.text.indexOf(':', 'editor:'.length);
                if (end > 0 && !command.text.slice(0, end).endsWith('~dark')) {
                    command = Object.freeze({...command, text: command.text.slice(0, end) + '~dark' + command.text.slice(end)});
                }
            }
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
        const commands = removeApplicationMenuBand(baseCommands, window.menuPresentationTop, menuHeight).map(adaptCommand);
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
            height: frame.contentHeight,
            popup,
            applicationMenu: applicationMenu ? {...applicationMenu, activeTarget:snapshot.popup ? window.applicationMenuAnchor?.target || null : null} : null,
            menuPresentationHeight: menuHeight,
            commands: Object.freeze(commands),
            popupCommands: Object.freeze(translateMenuCommands(popupCommands, popupTranslation.x, popupTranslation.y).map(adaptCommand)),
            app: snapshot.app.toString(),
            window: snapshot.window.toString(),
            title: window.title,
            entrypoint: window.entrypoint,
            applicationId: window.riceAppId || window.applicationId,
            riceAppId: window.riceAppId || "",
            iconName: window.iconName || "",
            role: window.role,
            dialogOnly: Boolean(window.dialogOnly),
            editorRegions:window.editorRegions||null,editorShapeRevision:window.editorShapeRevision||0,
            fileChooser: Boolean(window.fileChooser),
            actorPopup:window.transientPopup===true&&window.riceAppId==='org.ricewm.gnome.actor-popup',
            transientPopup:Boolean(window.transientPopup),
            titleTools: Object.freeze([...(window.titleTools || [])]),
            active: foregroundActive && window === activeWindow,
            applicationActive: Boolean(foregroundActive && window === applicationWindow),
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
            resizeHandles: window.resizable !== false && window.role !== "desktop" && !window.maximized && !window.fullscreen && nativeWindowResizeFrame(window, snapshot.dialog)
                ? nativeResizeHandles(nativeWindowResizeFrame(window, snapshot.dialog)) : [],
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
