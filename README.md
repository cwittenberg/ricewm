# RiceWM core

Reusable Rust desktop components and the JavaScript application SDK.
MIT licensed. Source: [cwittenberg/ricewm](https://github.com/cwittenberg/ricewm).

This edition is a library workspace. It includes desktop icon policy, window
layout, UI geometry and themes, a retained rendering protocol, and application
models. A complete desktop executable, browser host and service implementations
are outside this edition.

## Build and try

Install Rust 1.93 or newer and Node.js 18 or newer. From this repository root:

```sh
cargo test --workspace --locked
cargo run --locked -p ricewm-core-demo -- /tmp/ricewm-demo.txt
npm test
node tools/riceapp-pack.mjs sdk/examples/calc-sdk /tmp/calc-sdk.riceapp
```

The Rust demo lays out two windows, calculates a result, writes it to a new file,
then reopens and checks it. Choose a new output path; existing files are preserved.
No account or desktop service is required. Cargo downloads its normal public
registry dependencies on the first build.

## Components

| Crate | Responsibility |
| --- | --- |
| `rice-desktop` | Desktop icons, grid, selection, wallpaper geometry and host intents |
| `rice-layout` | Split, master and scrolling window layouts; arrangement serialization |
| `rice-ui` | Geometry, theme tokens, editor values and virtual grids |
| `rice-web-render-protocol` | Validated retained scenes, textures and paint transactions |
| `rice-files` | File-manager state, navigation, associations and operation planning |
| `rice-view` | Image and PDF viewer state, transforms, animation and cache policy |
| `rice-image-contract` | Image request identities and cache invalidation |
| `rice-calc` | Exact arithmetic, expressions and unit conversions |
| `rice-card` / `rice-day` | Contacts and calendar models |
| `rice-box` | Provider-neutral mailbox state and commands |
| `rice-tray` / `rice-spatial` | Tray layout and spatial scene geometry |

The public `rice-ui` crate exposes selected independent modules, rather than
the complete desktop widget toolkit. Layout types are public so other hosts can
reuse them. These libraries emit intents; they do not implement host services.

## Application SDK

The sole supported SDK is **1.0.0**, in
`sdk/versions/1.0.0/runtime/sdk/js/`. The selector at `sdk/rice-sdk-versions.mjs`
reflects that code. Compatible fixes update this version in place.

See [the SDK guide](sdk/README.md) for packaging and examples. Apps import
`@ricewm/app` and use a RiceWM host that supplies their declared capabilities.
This source edition tests the SDK with a local host bridge; it does not include
a runnable desktop host. Compiled application engines are separate dependencies.

## Contributing

Changes should include focused tests and preserve the capability boundary.
Keep generated packages and build output outside the source tree. The file
inventory in `public-files.json` records the reviewed source snapshot. Preserve
public contributions when reconciling later source updates.
