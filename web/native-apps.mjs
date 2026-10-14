import { RiceBrowserClipboardProvider } from "./rice-browser-clipboard-provider.mjs";
import {
  decodeVfsFrame,
  encodeVfsFrame,
  VfsWireWriter,
} from "./rice-vfs-wire-core.mjs";
import { decodeRappMessage } from "../sdk/versions/1.0.0/runtime/sdk/js/rapp.mjs";
// Embed the original native application actors and their existing RAPP protocol.
import { RiceNativeAppWireCodec } from "./rice-native-app-wire.mjs";
import {
  decodeRiceHostRequest,
  encodeRiceTimerEvent,
  encodeRiceWindowStateEvent,
} from "./rice-host-wire.mjs";
import {
  decodeSettingsRequest,
  encodeSettingsResponse,
} from "./settings-wire.mjs";
import { openLocalFiles } from "./local-files.mjs";
export async function openNativeApplications({
  send,
  launch,
  requestLaunch,
  onError,
}) {
  const files = await openLocalFiles();
  const { instance } = await WebAssembly.instantiate(
      await (
        await fetch(new URL("./rice_apps.wasm", import.meta.url))
      ).arrayBuffer(),
      {},
    ),
    api = instance.exports,
    timers = new Map(),
    clipboard = new RiceBrowserClipboardProvider();
  const maximum = 8 * 1024 * 1024,
    capabilities = 1n;
  if (api.rice_apps_init(Number(capabilities), 0) !== 1)
    throw Error("Native applications could not initialize");
  function ingest(bytes) {
    const pointer = api.rice_apps_io_reserve(bytes.length);
    new Uint8Array(api.memory.buffer, pointer, bytes.length).set(bytes);
    if (api.rice_apps_handle(bytes.length) !== 1) {
      const n = api.rice_apps_last_error_len(),
        p = api.rice_apps_io_reserve(n);
      api.rice_apps_last_error(p, n);
      throw Error(
        new TextDecoder().decode(new Uint8Array(api.memory.buffer, p, n)),
      );
    }
  }
  const core = {
    isNative: true,
    files,
    clipboard,
    async dispatch(bytes) {
      ingest(bytes);
      const output = [];
      for (let count = 0; count < 2000; count++) {
        const size = api.rice_apps_pending();
        if (!size) return output;
        const pointer = api.rice_apps_io_reserve(size);
        if (api.rice_apps_drain(pointer, size) !== size)
          throw Error("Native response could not drain");
        const frame = new Uint8Array(api.memory.buffer, pointer, size).slice(),
          view = new DataView(frame.buffer),
          kind = view.getUint16(6, true);
        if (kind === 30) {
          ingest(await clipboard.handle(frame));
        } else if (kind === 10) {
          ingest(await files.provider.handle(frame));
        } else if (kind === 60) {
          const r = decodeVfsFrame(frame, 60),
            request = r.id(),
            app = r.id(),
            tag = r.u8(),
            writer = new VfsWireWriter();
          writer.id(request);
          if (tag === 2) {
            r.finish();
            writer.u8(3);
            writer.u8(0);
            writer.u32(0);
          } else if (tag === 8) {
            const mime = r.text(255);
            r.finish();
            writer.u8(5);
            writer.u16(1);
            writer.text(mime, 255);
            const available = mime === "inode/directory";
            writer.u32(available ? 1 : 0);
            if (available) {
              for (const value of ["native:rice-files", "Files", "files"])
                writer.text(value, 255);
              writer.u32(1);
              writer.text("inode/directory", 255);
              writer.u8(1);
              writer.u8(1);
              writer.u8(1);
            }
          } else if (tag === 9) {
            const application = r.text(255),
              mime = r.text(255),
              count = r.u32();
            if (count > 128) throw Error("Too many launch paths");
            const paths = Array.from({ length: count }, () => r.text(4096)),
              remember = r.u8();
            r.finish();
            if (application === "native:rice-files") {
              await requestLaunch("rice-files", paths);
              writer.u8(6);
              writer.text(application, 255);
              writer.u8(0);
            } else {
              writer.u8(2);
              writer.text(
                "This application is unavailable in the standalone edition",
                16384,
              );
            }
          } else {
            writer.u8(2);
            writer.text(
              "This service is unavailable in the standalone edition",
              16384,
            );
          }
          ingest(encodeVfsFrame(61, writer.bytes));
        } else if (kind === 50) {
          const { request, operation } = decodeSettingsRequest(frame);
          let result;
          if (operation.kind === "load")
            result = {
              kind: "values",
              values: JSON.parse(
                localStorage.getItem("ricewm.native.settings") || "{}",
              ),
            };
          else if (
            (operation.kind === "set" &&
              [
                "desktop.icon_positions",
                "desktop.icons.auto_arrange",
                "ricefiles.places",
                "ricefiles.places-with-labels",
                "ricefiles.download-tip-shown",
                "ricefiles.options",
              ].includes(operation.key)) ||
            (operation.kind === "set" &&
              /^ricefiles\.(simplified-view|folder-tree)\./.test(operation.key))
          ) {
            const values = JSON.parse(
              localStorage.getItem("ricewm.native.settings") || "{}",
            );
            values[operation.key] = operation.value;
            localStorage.setItem(
              "ricewm.native.settings",
              JSON.stringify(values),
            );
            result = {
              kind: "applied",
              key: operation.key,
              value: operation.value,
            };
          } else
            result = {
              kind: "error",
              message: "This setting is unavailable in the standalone edition",
            };
          ingest(encodeSettingsResponse(request, result));
        } else if (kind === 70) {
          const request = decodeRiceHostRequest(frame);
          if (request.operation === "timer") {
            const key = `${request.app}:${request.token}`;
            clearInterval(timers.get(key));
            timers.delete(key);
            if (request.enabled)
              timers.set(
                key,
                setInterval(
                  () => send(encodeRiceTimerEvent(request.app, request.token)),
                  request.intervalMillis,
                ),
              );
          } else output.push(frame);
        } else if (kind === 2) {
          const message = decodeRappMessage(frame);
          await requestLaunch(message.entrypoint, message.arguments);
        } else output.push(frame);
      }
      throw Error("Native application response queue did not settle");
    },
    async start(app, entrypoint, args = []) {
      return await this.dispatch(
        RiceNativeAppWireCodec.launch(app, entrypoint, args, maximum),
      );
    },
    state(app, window, frame, focused, visible, maximized) {
      send(
        encodeRiceWindowStateEvent(app, window, {
          contentWidth: frame.width,
          contentHeight: frame.contentHeight,
          focused,
          visible,
          maximized,
          fullscreen: false,
        }),
      );
    },
    async closeApp(app) {
      for (const [key, timer] of timers)
        if (key.startsWith(`${app}:`)) {
          clearInterval(timer);
          timers.delete(key);
        }
      return this.dispatch(
        RiceNativeAppWireCodec.close(app, "requested", maximum),
      );
    },
  };
  await core.dispatch(RiceNativeAppWireCodec.hostHello(capabilities, maximum));
  return core;
}
