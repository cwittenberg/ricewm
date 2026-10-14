# RiceWM

RiceWM's window manager, browser compositor, native desktop applications and
JavaScript application SDK. First-party source is MIT licensed.

## Run the web desktop

Install Rust 1.93 or newer and Node.js 18 or newer. From this repository:

```sh
rustup target add wasm32-unknown-unknown
npm run build
npm start
```

Open **http://127.0.0.1:8080**. The build compiles the Rust source into
`rice_display.wasm` and `rice_apps.wasm` and prepares `dist/`. No npm install is
needed to build or serve. The first Cargo build downloads public dependencies.
You can also serve the contents of `dist/` from any static HTTP host that serves
`.wasm` as `application/wasm`. Opening `index.html` directly as a local file is
not supported.

The web edition uses the existing RiceWM canvas compositor, shell menus,
window decorations, fonts, icons, and original Rust **Desktop**, **Files** and
**Calc** applications. Files and desktop items are stored in the browser's
IndexedDB; SDK application settings are stored locally. Clear browser site data
to reset them. A separate browser profile has separate files.

The Applications menu also launches five SDK examples: **Calc SDK**, **To Do**,
**Date & time**, **Charts**, and **Table report**. To Do persists tasks across
reloads. The SDK and all twelve included example projects are in [sdk/](sdk/).

This edition runs the included applications and local file operations. The
browser host currently uses the RiceWM default theme. It does not include a
guest operating system, terminal execution, browser engine, package manager,
remote storage, the complete Settings app, or the separate office engines.
Those host services are not supplied by this build. Other example projects may
require such services; their source being included does not make them runnable
in this host.

## Source layout

| Source | Responsibility |
| --- | --- |
| [rice-x11-core](crates/rice-x11-core/src/lib.rs) | X11 server, window management, stacking, focus, workspaces, tiling, shell menus, retained surfaces and composition |
| [rice-display](crates/rice-display/src/lib.rs) | Native/Wasm display ABI and native control layout, rendering, input and hit testing |
| [rice-ui](crates/rice-ui/src/lib.rs) | Retained widget model, controls, themes, geometry, editor state and grids |
| [rice-apps](crates/rice-apps/src/lib.rs) | Original Desktop, Files and Calc actors with the registry restricted to those applications |
| [web/](web/) | RiceWM canvas compositor, window rendering, input, artwork and a static browser host with local storage adapters |
| [sdk/](sdk/) | SDK 1.0.0, version selection and example projects |

Additional crates contain reusable desktop, file manager, viewer, calculator,
calendar, contacts, mailbox, tray and spatial scene models. The source includes
[theme presets](assets/themes/); the initial browser host selects the default.

## Develop and verify

```sh
cargo test --workspace --locked
npm test
```

For the browser acceptance test, install Playwright and Chromium once:

```sh
npm install
npx playwright install chromium
npm run build
npm run test:web
```

The browser test operates the actual canvas UI: shell menus, native Calc,
window dragging, resizing, maximizing, minimizing and closing, desktop file
creation and reload, opening Files from a desktop icon, and SDK application
persistence. It rejects missing assets, browser errors and runtime requests to
other origins. Test files stay outside `dist/`.

A lower-level example exercises X11 clients and writes the real display
renderer's pixels to a new PNG:

```sh
node examples/display-smoke.mjs target/wasm32-unknown-unknown/debug/rice_display.wasm /tmp/ricewm-display.png
cargo run --locked -p ricewm-core-demo -- /tmp/ricewm-demo.txt
```

See [the SDK guide](sdk/README.md) for application packaging. The sole supported
SDK is [1.0.0](sdk/versions/1.0.0/runtime/sdk/js/); compatible fixes update it in
place.

## Artwork and licenses

The browser's RiceWM SVG artwork, icon font and required text fonts are
included. Preserve [font notices](licenses/fonts/) and
[icon decoder notices](licenses/icons/). The core uses original MIT fallback
raster icons. Editor advance data derives from DejaVu Sans; preserve its
[license](licenses/DEJAVU-LICENSE.txt). Apache ECharts and its dependency notices
are included with the SDK's vendor source.

[Source repository](https://github.com/cwittenberg/ricewm).
`public-files.json` records hashes for the reviewed source and media assets.
