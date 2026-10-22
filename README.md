# RiceWM

RiceWM is a desktop written in Rust that runs in your browser. Inspired by
[IceWM](https://ice-wm.org/), it brings windows, files and applications together
in a familiar desktop workspace. The desktop is compiled to WebAssembly, with a
JavaScript SDK for building apps.

This repository contains the desktop core and SDK, along with a standalone
browser build you can run locally. It includes the window manager, X11 server,
renderer, and Desktop, Files and Calc applications.

See [ricewm.org](https://ricewm.org/) for an introduction to RiceWM and the
full browser workspace.

## Run the web desktop

You'll need Rust 1.93 or newer and Node.js 18 or newer. Clone this repository,
then run:

```sh
rustup target add wasm32-unknown-unknown
npm run build
npm start
```

Open **http://127.0.0.1:8080**. The first build downloads the Rust dependencies
and compiles the desktop into `dist/`. You don't need `npm install` to build or
serve it.

Open Files or Calc from the Applications menu, or try one of the five SDK apps:
Calc SDK, To Do, Date & time, Charts and Table report. Files, desktop items and
app settings are saved in your browser. They survive a reload; clearing the
browser's site data removes them.

To host it elsewhere, serve `dist/` over HTTP with `.wasm` files served as
`application/wasm`.

The standalone build uses the default theme and local browser storage. The
full workspace shown on ricewm.org also has services this build doesn't provide,
including terminal execution, package installation, remote storage and office
engines. Some SDK examples need those services.

## Build an app

The [SDK guide](sdk/README.md) covers writing and packaging JavaScript apps.
Start with one of the twelve [example projects](sdk/examples/), such as
[To Do](sdk/examples/todo/) or [the calculator](sdk/examples/calc-sdk/).
The supported SDK version is 1.0.0.

## Source layout

| Directory | What's there |
| --- | --- |
| [rice-x11-core](crates/rice-x11-core/src/lib.rs) | X11 server, window management, workspaces and shell menus |
| [rice-display](crates/rice-display/src/lib.rs) | Display rendering and input |
| [rice-ui](crates/rice-ui/src/lib.rs) | Widgets, layout and themes |
| [rice-apps](crates/rice-apps/src/lib.rs) | Desktop, Files and Calc |
| [web/](web/) | Canvas compositor and browser host |
| [sdk/](sdk/) | JavaScript SDK and examples |

## Tests

```sh
cargo test --workspace --locked
npm test
```

To run the browser tests, install Playwright and Chromium first:

```sh
npm install
npx playwright install chromium
npm run build
npm run test:web
```

These exercise the desktop in Chromium, including window controls, Files,
Calc and saving app state across reloads.

## License

RiceWM's own source is [MIT licensed](LICENSE). Fonts, the icon decoder and
other dependencies have their own notices under [licenses/](licenses/).
The editor's font advance data comes from DejaVu Sans; its
[license](licenses/DEJAVU-LICENSE.txt) is included. Apache ECharts and its
dependency notices are included with the SDK vendor source. Keep these notices
when redistributing the project.
