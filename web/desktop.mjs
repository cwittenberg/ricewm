import { riceSdkThemePalette } from "./sdk-theme.mjs";
import { createRiceNativeWindow } from "./rice-native-window-factory.mjs";
import { composeNativeWindowStack } from "./rice-native-snapshot-compositor.mjs";
import { themeBackgroundGradient } from "./public-theme.mjs";
import "./rice-input-boundary.js";
import {
  RiceNativeAppWireCodec,
  decodeRiceNativeCloseResponse,
} from "./rice-native-app-wire.mjs";
import { openNativeApplications } from "./native-apps.mjs";
import { desktopNativeFrame } from "./rice-native-window-stack.mjs";
import { RiceTopbarController } from "./rice-topbar-controller.mjs";
import {
  RiceJavaScriptAppCore,
  Message,
} from "../sdk/versions/1.0.0/runtime/sdk/js/index.mjs";
import { decodeRappMessage } from "../sdk/versions/1.0.0/runtime/sdk/js/rapp.mjs";
import { RiceNativeUiBridge } from "./rice-native-ui-bridge.mjs";
import { RiceCanvasRenderer } from "./rice-canvas-renderer.mjs";
import { RiceDisplayCore } from "./rice-display-core.mjs";
import {
  constrainNativeFrame,
  nativeTitleControl,
  nativeResizeEdges,
  nativeResizeCursor,
  toggleNativeWindowMaximize,
} from "./rice-native-window-stack.mjs";
import { nativeChromeTheme } from "./rice-native-window-stack.mjs";
import { createRiceNativeTextInput } from "./rice-native-text-input.mjs";
import { themes } from "./theme.mjs";
const canvas = document.querySelector("#screen"),
  status = document.querySelector("#status");
const windows = [],
  cores = new Map();
let nextApp = 1n,
  theme = themes.ricewm,
  snapshot = null,
  scheduled = false,
  drag = null,
  hover = null,
  queue = Promise.resolve();
const viewport = () => ({
  x: 0,
  y: 32,
  width: innerWidth,
  height: innerHeight - 64,
});
const fail = (error) => {
  console.error(error);
  status.textContent = error.message || String(error);
};
const serial = (work) => {
  queue = queue.then(work).catch(fail);
  return queue;
};
const display = await RiceDisplayCore.instantiate(
  await (
    await fetch(new URL("./rice_display.wasm", import.meta.url))
  ).arrayBuffer(),
);
display.initialize(innerWidth, innerHeight);
const api = display.exports;
const bridge = new RiceNativeUiBridge(display);
bridge.setTheme(theme);
bridge.onRenderEvents = deliver;
display.setThemes({ schema: "ricewm-theme-catalog-v1", themes: [theme] });
display.selectTheme(0);
display.setGnomePanel({ profile: 50, indicators: [] });
display.configureTaskbar({
  visible: true,
  position: "Bottom",
  autoHide: false,
  quickLaunchVisible: false,
  trayItems: [],
});
function schedule() {
  if (!scheduled) {
    scheduled = true;
    requestAnimationFrame(render);
  }
}
const renderer = new RiceCanvasRenderer(canvas, {
  onNativeFault: fail,
  onFault: fail,
});
renderer.topbarController = new RiceTopbarController({
  renderer,
  activate: (item) => {
    if (item.key === "@clock") {
      renderer.calendarActive = !renderer.calendarActive;
      renderer.calendarFromTopBar = true;
      renderer.damaged = true;
      renderer.schedule();
    }
  },
});
function state(window) {
  const f = window.frame;
  const core = cores.get(window.app);
  if (core?.isNative) {
    core.state(
      window.app,
      window.window,
      f,
      window === windows.at(-1),
      !window.minimized,
      !!window.maximized,
    );
    return;
  }
  const app = cores.get(window.app)?.instances.get(window.app),
    native = app?.windows.get(window.window);
  native?.presentationState.observe({
    contentWidth: f.width,
    contentHeight: f.contentHeight,
    fullscreen: false,
    maximized: !!window.maximized,
    focused: window === windows.at(-1),
    visible: !window.minimized,
  });
}
function collectWindows(core) {
  if (core.isNative) return;
  const app = core.instances.values().next().value;
  if (!app) return;
  for (const native of app.windows.values()) {
    let window = windows.find(
      (w) => w.app === app.id && w.window === native.id,
    );
    if (!window) {
      window = {
        ...createRiceNativeWindow({
          app: app.id,
          entrypoint: "rice-sdk-app",
          arguments_: [],
          viewport: { width: innerWidth, height: innerHeight },
          workArea: viewport(),
          index: windows.length,
          title: native.title,
          theme,
          preferredSize: app.manifest.window,
        }),
        window: native.id,
        applicationId: app.manifest.id,
        iconName: app.manifest.iconName || "program",
        workspace: 0,
      };
      windows.push(window);
    }
    window.title = native.title;
    state(window);
  }
  tasks();
}
function apply(frames) {
  for (const bytes of frames) {
    const kind = new DataView(
      bytes.buffer,
      bytes.byteOffset,
      bytes.byteLength,
    ).getUint16(6, true);
    if ([20, 22, 23, 24].includes(kind)) {
      deliver(bridge.apply(bytes));
    } else if (kind === 6) {
      const reply = decodeRiceNativeCloseResponse(bytes),
        pending = pendingCloses.get(reply.request);
      if (
        pending &&
        pending.app === reply.app &&
        reply.decision !== "pending"
      ) {
        pendingCloses.delete(reply.request);
        if (reply.decision === "approved")
          void serial(() => destroyWindow(pending));
      }
    } else if (kind === 70) {
      const view = new DataView(
          bytes.buffer,
          bytes.byteOffset,
          bytes.byteLength,
        ),
        app = view.getBigUint64(12, true),
        win = view.getBigUint64(20, true),
        tag = bytes[28],
        window = windows.find((w) => w.app === app && w.window === win);
      if (tag === 10 && window) {
        const n = view.getUint32(29, true);
        window.title = new TextDecoder().decode(bytes.slice(33, 33 + n));
        tasks();
      }
      if (tag === 43 && window) {
        const token = view.getBigUint64(29, true),
          width = view.getUint32(37, true),
          height = view.getUint32(41, true);
        window.frame = constrainNativeFrame(
          { ...window.frame, width, height: height + 28 },
          viewport(),
        );
        cores
          .get(app)
          .instances.get(app)
          .windows.get(win)
          .contentSizing.receive({
            token,
            width: window.frame.width,
            height: window.frame.contentHeight,
            error: "",
          });
      }
      if (tag === 3 && window) void closeWindow(window);
    }
  }
  for (const core of cores.values()) collectWindows(core);
  schedule();
}
async function events() {
  for (let count = 0; count < 200; count++) {
    const size = api.rice_native_ui_event_pending();
    if (!size) return;
    if (size === 0xffffffff || size > 8 * 1024 * 1024)
      throw Error("Invalid UI event queue");
    const pointer = api.rice_io_reserve(size);
    if (api.rice_native_ui_event_drain(size) !== size)
      throw Error("Cannot read UI event");
    const bytes = new Uint8Array(api.memory.buffer, pointer, size).slice(),
      event = decodeRappMessage(bytes),
      core = cores.get(event.app);
    if (core) apply(await core.dispatch(bytes));
  }
  throw Error("Application event queue did not settle");
}
async function dispatchEvents(frames) {
  for (const bytes of frames) {
    const event = decodeRappMessage(bytes),
      core = cores.get(event.app);
    if (core) apply(await core.dispatch(bytes));
  }
}
function deliver(frames) {
  if (frames?.length) void serial(() => dispatchEvents(frames));
}
function nativeSnapshot(window) {
  return bridge.render(
    window.app,
    window.window,
    Math.max(1, window.frame.width),
    Math.max(1, window.frame.contentHeight),
  );
}
function render() {
  scheduled = false;
  try {
    const root = { width: innerWidth, height: innerHeight };
    const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
    const shellSettings = {
      dockVisible: true,
      dockPosition: "Bottom",
      dockAutoHide: false,
      gnomePanelVisible: true,
      animations: !reduced,
      reduceMotion: reduced,
      activeWindowShadow: true,
      inactiveWindowsTranslucent: true,
      menuShadow: true,
      icons: "RiceWM",
      iconVariant: "monochrome",
      dockStyle: "Standard",
    };
    const composed = composeNativeWindowStack({
      windows: windows.filter((w) => !w.minimized),
      bridge,
      theme,
      desktopSettings: {
        type: "Gradient",
        mode: "Crop",
        style: "Layered",
        color: "#364f6b",
        gradient: themeBackgroundGradient(theme),
      },
      shellSettings,
      dockRevealed: true,
      hoveredTitleControl: hover
        ? { app: hover.window.app, action: hover.action }
        : null,
      activeWindow: windows.at(-1),
      dialogWorkArea: () => viewport(),
    });
    snapshot = composed
      ? {
          ...composed,
          windows: composed.windows.map((w) => ({
            ...w,
            uiApp: BigInt(w.app),
          })),
        }
      : null;
    renderer.applyNative(snapshot);
    const commands = display.drainScene();
    renderer.apply(
      commands,
      display.surfaceUploads(commands),
      display.shellTexts(),
    );
    renderer.damaged = true;
    renderer.flush();
    void serial(events);
    textInput?.sync({ scene: true });
  } catch (error) {
    fail(error);
  }
}
function tasks() {
  display.replaceNativeTasks(
    windows
      .filter((w) => w.role !== "desktop")
      .map((window) => ({
        id: Number(window.app),
        workspace: 0,
        title: window.title,
        applicationId: window.applicationId,
        icon: 0,
        minimized: window.minimized,
        maximized: !!window.maximized,
        active: window === windows.at(-1),
      })),
  );
  schedule();
}
function raise(window) {
  if (window.role === "desktop") return;
  windows.splice(windows.indexOf(window), 1);
  windows.push(window);
  for (const w of windows) state(w);
  tasks();
  schedule();
}
const pendingCloses = new Map();
let closeSequence = 1n;
async function closeWindow(window) {
  if (window.role === "desktop" || [...pendingCloses.values()].includes(window))
    return;
  const request = closeSequence++;
  pendingCloses.set(request, window);
  const core = cores.get(window.app);
  apply(
    await core.dispatch(
      RiceNativeAppWireCodec.closeRequest(
        request,
        window.app,
        "requested",
        8 * 1024 * 1024,
      ),
    ),
  );
}
async function destroyWindow(window) {
  const core = cores.get(window.app);
  if (core?.isNative) {
    await core.closeApp(window.app);
    windows.splice(windows.indexOf(window), 1);
    cores.delete(window.app);
    bridge.removeApp(window.app);
    tasks();
    return;
  }
  const app = core?.instances.get(window.app),
    native = app?.windows.get(window.window);
  native?.close();
  windows.splice(windows.indexOf(window), 1);
  if (app && !app.windows.size) {
    bridge.removeApp(window.app);
    await core.close();
    cores.delete(window.app);
  }
  tasks();
  schedule();
}
function target(x, y) {
  const popup = [...windows]
    .reverse()
    .find(
      (w) =>
        w.popup &&
        x >= w.frame.x + w.popup.rect.x &&
        x < w.frame.x + w.popup.rect.x + w.popup.rect.width &&
        y >= w.frame.y + w.frame.titleHeight + w.popup.rect.y &&
        y <
          w.frame.y +
            w.frame.titleHeight +
            w.popup.rect.y +
            w.popup.rect.height,
    );
  if (popup) return popup;
  return [...windows]
    .reverse()
    .find(
      (w) =>
        !w.minimized &&
        x >= w.frame.x &&
        x < w.frame.x + w.frame.width &&
        y >= w.frame.y &&
        y < w.frame.y + w.frame.height,
    );
}
function pointer(window, kind, x, y, button = 0) {
  void serial(async () => {
    if (!windows.includes(window)) return;
    nativeSnapshot(window);
    await bridge.prepareEditorPointer(window.app, window.window);
    await dispatchEvents(
      bridge.pointer(
        window.app,
        window.window,
        kind,
        Math.round(x - window.frame.x),
        Math.round(y - window.frame.y - window.frame.titleHeight),
        button,
        0,
      ),
    );
    schedule();
  });
}
canvas.addEventListener("pointerdown", (event) => {
  canvas.focus();
  const window = target(event.clientX, event.clientY);
  if (!window || shellAt(event.clientX, event.clientY)) {
    display.pointerButton(
      true,
      event.button + 1,
      event.clientX,
      event.clientY,
      0,
    );
    shellActions();
    schedule();
    return;
  }
  raise(window);
  canvas.setPointerCapture(event.pointerId);
  const f = window.frame,
    edges =
      window.role === "desktop"
        ? null
        : nativeResizeEdges(f, event.clientX, event.clientY),
    action =
      window.role === "desktop"
        ? null
        : nativeTitleControl(
            f,
            event.clientX,
            event.clientY,
            window.title,
            [],
            nativeChromeTheme(theme).windowDecorations || {},
          );
  if (action) {
    if (action === "close") void closeWindow(window);
    else if (action === "minimize") {
      window.minimized = true;
      tasks();
      schedule();
    } else if (action === "maximize") {
      toggleNativeWindowMaximize(window, viewport(), viewport());
      state(window);
      schedule();
    }
    return;
  }
  if (
    window.role !== "desktop" &&
    (edges || event.clientY < f.y + f.titleHeight)
  ) {
    drag = {
      window,
      x: event.clientX,
      y: event.clientY,
      frame: { ...f },
      edges,
    };
    return;
  }
  pointer(window, 1, event.clientX, event.clientY, event.button + 1);
});
canvas.addEventListener("pointermove", (event) => {
  if (drag) {
    const dx = event.clientX - drag.x,
      dy = event.clientY - drag.y;
    if (drag.edges) {
      const f = { ...drag.frame };
      if (drag.edges.includes("r")) f.width += dx;
      if (drag.edges.includes("b")) f.height += dy;
      if (drag.edges.includes("l")) {
        f.x += dx;
        f.width -= dx;
      }
      if (drag.edges.includes("t")) {
        f.y += dy;
        f.height -= dy;
      }
      drag.window.frame = constrainNativeFrame(f, viewport());
    } else
      drag.window.frame = constrainNativeFrame(
        { ...drag.frame, x: drag.frame.x + dx, y: drag.frame.y + dy },
        viewport(),
      );
    state(drag.window);
    schedule();
    return;
  }
  display.pointerMotion(event.clientX, event.clientY, 0);
  const window = target(event.clientX, event.clientY);
  hover = window
    ? {
        window,
        action: nativeTitleControl(
          window.frame,
          event.clientX,
          event.clientY,
          window.title,
        ),
      }
    : null;
  canvas.style.cursor = window
    ? nativeResizeCursor(
        nativeResizeEdges(window.frame, event.clientX, event.clientY),
      ) || "default"
    : "default";
  if (window && event.clientY >= window.frame.y + window.frame.titleHeight)
    pointer(window, 0, event.clientX, event.clientY);
  schedule();
});
canvas.addEventListener("pointerup", (event) => {
  if (drag) {
    drag = null;
    schedule();
    return;
  }
  if (
    shellAt(event.clientX, event.clientY) ||
    !target(event.clientX, event.clientY)
  ) {
    display.pointerButton(
      false,
      event.button + 1,
      event.clientX,
      event.clientY,
      0,
    );
    shellActions();
    schedule();
    return;
  }
  const window = target(event.clientX, event.clientY);
  if (window && event.clientY >= window.frame.y + window.frame.titleHeight)
    pointer(window, 2, event.clientX, event.clientY, event.button + 1);
});
canvas.addEventListener(
  "wheel",
  (event) => {
    const window = target(event.clientX, event.clientY);
    if (!window) return;
    event.preventDefault();
    pointer(window, 5, event.clientX, event.clientY, event.deltaY > 0 ? 5 : 4);
  },
  { passive: false },
);
function key(text, code = 0, modifiers = 0, pressed = true) {
  void serial(async () => {
    const window = [...windows].reverse().find((w) => !w.minimized);
    if (!window) return;
    nativeSnapshot(window);
    await dispatchEvents(
      bridge.key(window.app, window.window, pressed, code, modifiers, text),
    );
    schedule();
  });
}
for (const type of ["keydown", "keyup"])
  document.addEventListener(type, (event) => {
    if (
      event.target !== canvas &&
      event.target?.dataset?.riceNativeTextInput !== "true"
    )
      return;
    if (event.isComposing) return;
    const pressed = type === "keydown",
      textTarget = event.target !== canvas;
    // The IME textarea owns ordinary committed text; native owns navigation and shortcuts.
    if (
      textTarget &&
      event.key.length === 1 &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.altKey
    )
      return;
    const normalized = RiceInputBoundary.keyEvents(event, pressed);
    if (!normalized.length) return;
    event.preventDefault();
    for (const input of normalized) {
      if (input.keycode === 70 && input.state & 8 && pressed) {
        const current = windows.at(-1);
        if (current?.role !== "desktop") void closeWindow(current);
        continue;
      }
      key(input.text || "", input.keycode, input.state, pressed);
    }
  });
canvas.addEventListener("contextmenu", (event) => event.preventDefault());
canvas.addEventListener("dblclick", (event) => {
  if (event.button !== 0) return;
  const window = target(event.clientX, event.clientY);
  if (window && !shellAt(event.clientX, event.clientY)) {
    pointer(window, 1, event.clientX, event.clientY, 6);
    pointer(window, 2, event.clientX, event.clientY, 6);
  }
});
const textInput = createRiceNativeTextInput({
  document,
  container: document.querySelector("#desktop"),
  canvas,
  snapshot: () => snapshot,
  enabled: () => true,
  commit: ({ text }) => key(text),
  rejected: fail,
});
const handlers = {
  "debug.log": () => null,
  "settings.get": (app, p) =>
    JSON.parse(localStorage.getItem(`ricewm.app.${app}.${p.key}`) || "null"),
  "settings.set": (app, p) => {
    localStorage.setItem(`ricewm.app.${app}.${p.key}`, JSON.stringify(p.value));
    return true;
  },
  "clipboard.writeText": async (app, p) => {
    await navigator.clipboard.writeText(
      typeof p === "string" ? p : p.text || "",
    );
    return true;
  },
  "dialogs.confirm": async (app, p) =>
    confirm(p.message || p.text || "Continue?"),
  "host.openExternal": async (app, p) => {
    const url = new URL(typeof p === "string" ? p : p.url);
    if (url.protocol !== "https:")
      throw Error("Only HTTPS links are supported");
    globalThis.open(url.href, "_blank", "noopener");
    return true;
  },
  "native-ui.measure": (app, p) =>
    bridge.measureControlTree(p.nodes, p.width, p.app),
};
function appTheme() {
  const palette = {
    ...riceSdkThemePalette(theme),
    reducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
  };
  for (const core of cores.values()) {
    if (core.isNative) continue;
    for (const app of core.instances.values()) {
      app.theme = palette;
      app.emit(Message.THEME_CHANGED, palette);
      apply(core.collect(app));
    }
  }
}
const apps = [
  ["calc-sdk", "Calc SDK", "Calculator and scientific tools", "calculator"],
  ["todo", "To Do", "Lists, tasks and notes saved locally", "selection-check"],
  ["date-time", "Date & time", "Calendar and time controls", "calendar"],
  ["chart-dashboard", "Charts", "Interactive chart dashboard", "chart"],
  ["table-report", "Table report", "Tables and data controls", "table"],
];
async function launch(name, args = []) {
  if (
    ![
      "rice-calc",
      "rice-desktop",
      "rice-files",
      ...apps.map((a) => a[0]),
    ].includes(name)
  ) {
    status.textContent = `${name} is not included in this edition`;
    return;
  }
  if (["rice-calc", "rice-desktop", "rice-files"].includes(name))
    return launchNative(name, args);
  try {
    status.textContent = "Opening application…";
    const manifest = await (
      await fetch(
        new URL(`../sdk/examples/${name}/manifest.json`, import.meta.url),
      )
    ).json();
    const { default: definition } = await import(
      `../sdk/examples/${name}/${manifest.entry}`
    );
    const core = new RiceJavaScriptAppCore(definition, { manifest });
    const id = nextApp++;
    cores.set(id, core);
    await core.initialize({
      capabilities: 0xffffffffn,
      maximumMessageBytes: 8 * 1024 * 1024,
      callHost: (group, operation, p) => {
        const handler = handlers[`${group}.${operation}`];
        if (!handler)
          throw Error(
            `This browser edition does not supply ${group}.${operation}`,
          );
        return handler(manifest.id, p);
      },
      emitResponses: (frames) => {
        apply(frames);
      },
      onFault: fail,
    });
    const frames = await core.launch({
      app: id,
      arguments: [],
      entrypoint: manifest.entry,
    });
    collectWindows(core);
    apply(frames);
    appTheme();
    canvas.focus();
    status.textContent = "";
    schedule();
  } catch (error) {
    fail(error);
  }
}
async function launchNative(entrypoint, args = []) {
  const id = nextApp++;
  const window = {
    ...createRiceNativeWindow({
      app: id,
      entrypoint,
      arguments_: args,
      viewport: { width: innerWidth, height: innerHeight },
      workArea: viewport(),
      index: windows.length,
      theme,
    }),
    window: 1n,
    workspace: 0,
  };
  const frame = window.frame;
  if (window.role === "desktop") {
    frame.titleHeight = 0;
    frame.contentHeight = frame.height;
    windows.unshift(window);
  } else windows.push(window);
  cores.set(id, nativeApps);
  apply(await nativeApps.start(id, entrypoint, args));
  state(window);
  tasks();
  return window;
}
const nativeApps = await openNativeApplications({
  send: (bytes) =>
    void serial(async () => apply(await nativeApps.dispatch(bytes))),
  requestLaunch: (entry, args) =>
    launch(entry.replace(/^native[:/]+/, ""), args),
  onError: fail,
});
const catalog = [
  {
    action: 0x2001,
    label: "Files",
    entrypoint: "rice-files",
    iconName: "files",
    icon: 11,
    category: 8,
  },
  {
    action: 0x2000,
    label: "Calc",
    entrypoint: "rice-calc",
    iconName: "calculator",
    icon: 0,
    category: 8,
  },
  ...(await Promise.all(
    apps.map(async ([id, title], index) => {
      const manifest = await (
        await fetch(
          new URL(`../sdk/examples/${id}/manifest.json`, import.meta.url),
        )
      ).json();
      return {
        action: 0x1000 + index,
        label: title,
        entrypoint: id,
        applicationId: manifest.id,
        iconName: manifest.iconName || "program",
        icon: 0,
        category: 8,
      };
    }),
  )),
];
display.setMenuApplications(catalog);
renderer.setApplicationIcons({
  menu: catalog,
  builtins: catalog,
  quick: [],
  desktop: [],
  associations: [],
});
function shellAt(x, y) {
  return [...renderer.windows.values()].some(
    (w) =>
      w.viewable &&
      w.shellLayer &&
      x >= w.x &&
      x < w.x + w.width &&
      y >= w.y &&
      y < w.y + w.height,
  );
}
function shellActions() {
  for (const action of display.drainWmActions()) {
    const app = catalog.find((row) => row.action === action);
    if (app) {
      void launch(app.entrypoint);
      continue;
    }
    const prefix = (action & 0xffff0000) >>> 0,
      id = action & 0xffff,
      window = windows.find((w) => Number(w.app) === id);
    if (prefix === 0xfffa0000 && window) {
      window.minimized = window === windows.at(-1) && !window.minimized;
      if (!window.minimized) raise(window);
      tasks();
    } else if (prefix === 0xfff80000 && window) void closeWindow(window);
    else if (prefix === 0xfff60000 && window) {
      window.minimized = true;
      tasks();
    } else if (prefix === 0xfff70000 && window) {
      toggleNativeWindowMaximize(window, viewport(), viewport());
      state(window);
      tasks();
    } else if (prefix === 0xfff50000 && window) {
      window.minimized = false;
      raise(window);
    } else if (prefix === 0xfffb0000) {
      display.switchWorkspace(id);
    } else if (action === 0xffffff07) {
      for (const w of windows) if (w.role !== "desktop") w.minimized = true;
      tasks();
    }
  }
}
window.addEventListener("resize", () => {
  display.resize(innerWidth, innerHeight);
  for (const w of windows) {
    w.frame =
      w.role === "desktop"
        ? desktopNativeFrame(
            { width: innerWidth, height: innerHeight },
            viewport(),
          )
        : constrainNativeFrame(w.frame, viewport());
    state(w);
  }
  schedule();
});
setInterval(() => {
  const now = new Date();
  display.setClock(now.getHours(), now.getMinutes());
  schedule();
}, 1000);
const now = new Date();
display.setClock(now.getHours(), now.getMinutes());
await launchNative("rice-desktop");
window.ricewm = {
  launch,
  windows,
  cores,
  renderer,
  display,
  bridge,
  get snapshot() {
    return snapshot;
  },
  get native() {
    return api;
  },
  ready: true,
};
status.textContent = "";
schedule();
