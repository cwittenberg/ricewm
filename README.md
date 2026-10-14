# RiceWM

RiceWM's window manager, X11 display server, compositor, native UI renderer and
JavaScript application SDK. The first-party Rust code is MIT licensed.

## The desktop core

| Source | Responsibility |
| --- | --- |
| [rice-x11-core](crates/rice-x11-core/src/lib.rs) | X11 protocol handling, managed windows, focus, stacking, frames, shell chrome, workspaces, tiling, pointer and keyboard input, retained surfaces and composition |
| [DisplayServer](crates/rice-x11-core/src/server.rs) | Host-independent display-server entry point and bounded client transport |
| [rice-display](crates/rice-display/src/lib.rs) | Embeddable native/Wasm display ABI, X11 clients, input, scene updates and surface access |
| [NativeUiHost](crates/rice-display/src/native_ui.rs) | Native control layout, rendering, hit testing, menus, dialogs, text editing and retained scenes |
| [rice-ui](crates/rice-ui/src/lib.rs) | Complete retained widget model, controls, geometry, themes, editor state and virtual grids |
| [rice-native-app-protocol](crates/rice-native-app-protocol/src/lib.rs) | Bounded application messages, widget patches and host capability interfaces |
| [rice-web-render-protocol](crates/rice-web-render-protocol/src/lib.rs) | Validated retained scenes, textures and paint transactions |

The core accepts X11 protocol bytes and produces retained scene updates and
raster surfaces. Host adapters supply event delivery, presentation, application
execution and optional services. The display ABI builds as `rice_display.wasm`;
it can be embedded without an account or hosted service.

## Build and exercise the core

Install Rust 1.93 or newer and Node.js 18 or newer. From this repository root:

```sh
cargo test --workspace --locked
rustup target add wasm32-unknown-unknown
cargo build --locked -p rice-display --target wasm32-unknown-unknown
node examples/display-smoke.mjs target/wasm32-unknown-unknown/debug/rice_display.wasm /tmp/ricewm-display.png
npm test
```

The display smoke test instantiates the actual Wasm core, completes an X11
connection, creates two managed windows, drags a window by its title bar, reads
the core's raster surfaces and writes a PNG. Choose a new output path; existing
files are preserved. This is an executable core demonstration, not a replacement
for a desktop host. Cargo fetches normal public dependencies on the first build.

A second example exercises the reusable models and local file save/reopen:

```sh
cargo run --locked -p ricewm-core-demo -- /tmp/ricewm-demo.txt
```

Core tests cover X11 requests, multi-monitor windows, focus and input, shell
menus, taskbar geometry, notification area, scrolling layouts, retained controls,
localization, scene transport and native widget rendering. Tests which require
the separate first-party application bundle are outside this source edition.

## Application SDK and examples

The sole supported SDK is **1.0.0**, in
[sdk/versions/1.0.0/runtime/sdk/js/](sdk/versions/1.0.0/runtime/sdk/js/).
The [version selector](sdk/rice-sdk-versions.mjs) reflects that implementation.
Compatible fixes update this version in place.

See [the SDK guide](sdk/README.md) and [example projects](sdk/examples/).
Apps import `@ricewm/app` and declare their host capabilities. For example:

```sh
node tools/riceapp-pack.mjs sdk/examples/calc-sdk /tmp/calc-sdk.riceapp
node tools/riceapp-pack.mjs sdk/examples/todo /tmp/todo.riceapp
```

The SDK includes controls, application lifecycle, messages, scene rendering,
charts, desktop events and capability-checked service calls. The public packager
uses the selected SDK's reflected service metadata. Applications run in a RiceWM
host which provides the services they request. Sheet and Deck include application
sources; their separate compiled engines, fonts and assets are not bundled.

## Reusable application components

Additional crates provide desktop icon policy (`rice-desktop`), file-manager
models (`rice-files`), viewer state (`rice-view`), shared image contracts,
calculator arithmetic, contacts, calendar, mailbox state, tray layouts and spatial
scene geometry. Their host intents keep policy independently testable.

## Themes and licensing

[Theme presets](assets/themes/) supply appearance tokens. The core uses original
MIT-licensed fallback raster icons and accepts host-supplied application icons.
Editor font advance data is derived from DejaVu Sans; preserve its
[license](licenses/DEJAVU-LICENSE.txt). The SDK includes Apache ECharts with its
Apache-2.0 and dependency notices in the vendor directory. Public translation
catalogs contain only messages used by the exported core.

Source: [cwittenberg/ricewm](https://github.com/cwittenberg/ricewm).
The inventory in `public-files.json` records this reviewed source snapshot.
Preserve public contributions when reconciling later updates.
