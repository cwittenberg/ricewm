All code of RiceWM will be published here for FOSDEM 2027

Try it here: https://env.now

[ricewm-promo-v2-small.webm](https://github.com/user-attachments/assets/0af2746d-f753-4a75-bf0f-9f68f907acd7)



# RiceWM

**A desktop built in Rust and WebAssembly, with a shared shell for the browser and Linux.**

RiceWM brings together a window manager, compositor, X11-compatible display
server, application toolkit, and a growing collection of everyday apps. It is
the desktop layer of [Webexe](../README.md): Webexe supplies application
execution and the surrounding environment; RiceWM gives that environment its
windows, controls, and visual identity.

The aim is a familiar, responsive desktop that stays small, is easy to
customize, and runs real applications. A terminal, a document editor, a file
manager, and an installed X11 application all participate in the same window
lifecycle rather than living in separate desktop overlays.

[Capabilities](#what-you-can-do) · [Architecture](#how-it-fits-together) ·
[Source map](#finding-your-way-around) · [Build and test](#build-and-test) ·
[App SDK](docs/RICE_APP_SDK.md)

## What you can do

### Make the desktop your own

RiceWM owns window frames, focus, stacking, movement, resizing, minimizing,
maximizing, closing, and workspace membership. The shell includes application
menus, a taskbar, a dock, Activities, and desktop launchers, with presentation
selected by the active theme and target.

Choose floating windows or tiling. [Infinite scroll](docs/scrolling-tiling.md)
arranges windows in a horizontal strip with keyboard navigation, adjustable
columns, and stacked windows. Layout preferences are saved per workspace.

Theme Studio brings appearance, backgrounds, motion, extensions, and desktop
settings together. Shared theme tokens style first-party app controls as well
as shell chrome. Themes can include image or video backgrounds, gradients,
icons, and motion settings. Localization lives in a shared Rust/JavaScript
catalog rather than separate translations for every frontend.

### Get everyday work done

The first-party app collection shares RiceWM's window controls, input, themes,
and host services.

| Application | Purpose |
| --- | --- |
| **Rice Files** | Browse files, switch views, preview media, manage folders, and connect supported cloud storage. |
| **Rice Term** | Work in real shell sessions with tabs, selection, clipboard integration, scrollback, and saved preferences. |
| **Rice Paper** | Write and edit documents with rich editing and Markdown workflows. |
| **Rice View** | View images and PDFs, navigate images, and crop, resize, and export image edits. |
| **Rice Calc** | Perform decimal and scientific calculations. |
| **Rice Sheet** | Edit spreadsheets through a JavaScript app and a Wasm calculation engine, with XLSX/ODS workflows under active development. |
| **Rice Deck** | Create and edit presentations, with PPTX/ODP workflows under active development. |
| **Rice Settings** | Configure the desktop and its visual behavior. |
| **Rice System Monitor** | Inspect runtime performance and resource use. |
| **Rice Store** | Browse application sources through shared package and launch services. |

The collection also includes calendar, contacts, mail, media, and Bowl browser
components. Availability and integration vary by build and target; document
format support and compatibility are tracked in the relevant app documentation.

Rice Files' browser integration supports **Google Drive, Nextcloud, and
S3-compatible storage** through a shared provider interface. Remote files reuse
the desktop's VFS and media services. Saved connection metadata survives
reloads; credentials remain in session memory and require reconnection. See
[cloud storage](#cloud-storage-in-rice-files) for setup and provider limits.

### Run applications through a common desktop

RiceWM supports three application paths:

- **Rust apps:** independently testable app libraries, composed into the
  first-party `rice_apps.wasm` bundle for the browser distribution.
- **JavaScript apps:** ES modules packaged as `.riceapp` files, using the
  RiceWM SDK and retained native controls.
- **Compatibility apps:** real application processes connected through the
  X11 display boundary or Webexe's direct Windows runtime.

For browser X11 clients, unchanged supported guest executables connect to
`/tmp/.X11-unix/X0`. Their protocol traffic reaches `rice_display.wasm`, which
renders to the desktop canvas. This path does not require Xorg or a VNC server.
GTK and Qt applications continue to own their application content; RiceWM owns
the surrounding desktop and implements the supported display protocol.

### Build your own apps

The [JavaScript SDK](docs/RICE_APP_SDK.md) provides themed controls, layout,
messages, system dialogs, RiceBus, packaging, and debugging. Each `.riceapp`
runs in a disposable module-worker actor. File access, networking, and compute
are requested through host-owned capabilities; application code does not receive
the browser DOM or RiceWM's Wasm memory.

Start with the [SDK guide](examples/sdk-guide/) and [examples](examples/).
Apps such as Sheet and Deck illustrate how a JavaScript UI can use a separate
Wasm engine while remaining part of the native desktop.

## Where it runs

| Target | How it works |
| --- | --- |
| **Browser / Wasm** | The browser hosts the desktop; Webexe supplies the execution runtime and browser-backed environment. |
| **Linux desktop** | A Debian-based image boots patched Chromium on DRM/KMS. Linux applications execute on the host CPU while using the shared Rice shell and display. |
| **Hosted Wayland** | The desktop runs in Chromium under an outer compositor, with windowed GNOME and Cage session options. |

The Linux target is a working prototype. Hardware qualification, mixed-DPI
multi-monitor behavior, and suspend recovery still need work. See the
[desktop target guide](../platform/desktop/README.md) for its evidence and setup.

## How it fits together

RiceWM separates **application behavior**, **desktop policy and rendering**, and
**host capabilities**. An app describes its UI and requests an operation; the
shared runtime validates and routes that request to the selected host adapter.

<img width="1536" height="1024" alt="image" src="https://github.com/user-attachments/assets/5a1a7e2a-9cdd-4300-9c4e-2509331b4473" />

**RAPP** is the retained native application protocol. It carries bounded UI,
input, and service messages between application actors and the desktop.
RiceWM owns the window frame, taskbar entry, workspace, snapshots, and previews
for every Rice app. Host adapters supply operations rather than creating
another window or focus system.

The browser's two core first-party Wasm binaries have different jobs:

| Binary | Responsibility |
| --- | --- |
| `rice_display.wasm` | Display protocol integration, rendering, input, and desktop window presentation. |
| `rice_apps.wasm` | Built-in application actors and their shared composition root. |

Optional engines and compatibility runtimes can add other payloads; these two
binaries are the core desktop split, not the entire download inventory.

App models remain separate from browser APIs and filesystem implementations.
`rice-ui` supplies shared widgets and theme tokens; the protocol crates define
the messages; adapters provide I/O. Networking uses one `RiceDataProvider` per
session so applications share the selected transport and offline policy.

For the detailed contracts, read the
[native application architecture](docs/NATIVE_APP_ARCHITECTURE.md) and
[SDK reference](docs/RICE_APP_SDK.md).

## Finding your way around

| Path | What belongs here |
| --- | --- |
| [`crates/`](crates/) | Rust workspace: app models, actor composition, widgets, protocols, rendering, and X11. |
| [`crates/rice-apps/`](crates/rice-apps/) | Built-in app actors, Wasm ABI, and composition of models with UI and host requests. |
| [`crates/rice-ui/`](crates/rice-ui/) | Shared retained widgets, layout, and theme behavior. |
| [`crates/rice-display/`](crates/rice-display/) and [`crates/rice-x11-core/`](crates/rice-x11-core/) | Display integration, compositor, X11 implementation, and shell/window policy. |
| [`crates/rice-native-app-protocol/`](crates/rice-native-app-protocol/) | Shared native app wire contracts. |
| [`web/`](web/) | Authored browser modules, host adapters, workers, and staged browser payloads. |
| [`sdk/`](sdk/) | JavaScript SDK used by `.riceapp` applications. |
| [`examples/`](examples/) | SDK examples and JavaScript apps, including Sheet and Deck. |
| [`engines/`](engines/) | Supporting app engines and adapted upstream code. |
| [`assets/`](assets/) and [`locales/`](locales/) | Authored themes, icons, fonts, desktop assets, and translations. |
| [`tests/`](tests/) | RiceWM subsystem and application tests. |
| [`docs/`](docs/) | Architecture references, feature guides, implementation plans, and evidence. |
| [`helpers/`](helpers/) and [`tools/`](tools/) | Build helpers, architecture checks, asset tools, and app packaging. |
| [`../platform/desktop/`](../platform/desktop/) | Linux images, hosted sessions, native services, and desktop integration. |
| [`../wasm-rust/`](../wasm-rust/) | Webexe's guest execution runtime, rootfs, and publication pipeline. |
| [`../site/ricewm/`](../site/ricewm/) | Published RiceWM assets. |

Source, staging, and publication have distinct ownership:

<img width="727" height="485" alt="image" src="https://github.com/user-attachments/assets/9cbb1166-3b01-45a1-be21-84f1d4a9078f" />


Edit authored inputs and use the owning build pipeline to publish them.
`site/ricewm/` is a published inventory, not a separate implementation.
Browser-runtime tests belong in [`../tests/site/`](../tests/site/), and test-only
files stay out of published assets. The
[repository ownership guide](../docs/repository-structure.md) explains the wider
source and release boundaries.

## Compatibility and project status

RiceWM is under active development. A catalog entry, a mapped window, or a
successful build does not establish that an application can do useful work,
save, reopen, and shut down correctly.

Supported X11 requests and tested GTK2/GTK3 workflows are implemented, but this
is not a claim of complete Xorg, GTK, Qt, IceWM, Windows, or POSIX compatibility.
Graphics API support, toolkit edges, document fidelity, and platform behavior
have separate acceptance requirements. Browser canvas presentation through
WebGPU does not imply guest OpenGL or Vulkan support.

Read [STATUS.md](STATUS.md) for the recorded X11 evidence,
[native application parity](NATIVE_APPS_PARITY.md) for app-specific coverage,
and the [Linux desktop status](../platform/desktop/docs/RICEWM_NATIVE_STATUS.md)
for native-target results. These ledgers and feature documents have their own
update dates; they are not a substitute for testing the current release.

## JavaScript application SDK

User-developed RiceWM applications, the `.riceapp` package format, control
inheritance/message procedures, RiceBus, system dialogs, protected VFS/network
capabilities, packaging, debugging, and the complete example are documented in
[`docs/RICE_APP_SDK.md`](docs/RICE_APP_SDK.md).

The SDK deliberately keeps application code as JavaScript. Each `.riceapp`
runs in a disposable module-worker actor with ambient browser I/O removed;
`.riceapp` packages are data assets rather than separately compiled Wasm
programs. RiceWM's host-owned capability boundary and retained RAPP compositor
remain outside that realm.

## Network capability rule

The runnable Gmail-first Rice Box slice and its local-only OAuth/App Password
test paths are documented in
[`docs/RICE_BOX_PHASE1_TESTING.md`](docs/RICE_BOX_PHASE1_TESTING.md).

`RiceDataProvider` is the single application-facing HTTP capability in
RiceWM. Every built-in native app, hosted compatibility package, and future app
service that needs HTTP(S) must receive and reuse that provider; application
code must not call browser `fetch` or construct a gateway/proxy transport.
The runtime creates one provider per desktop session, backed by the selected
offline, local-gateway, configured proxy, or cloud network strategy. Offline
mode exposes no HTTP capability and must fail closed.

Bowl navigation, Extension Manager catalog/archive requests, and network calls
made by GNOME compatibility extensions all use this same provider instance.
Transport selection and policy therefore remain outside the app and cannot
silently diverge between applications.

## Direct Windows runtime

The direct Windows/Wasm subsystem's browser module ownership, dependency
direction, and refactor rules are indexed in
[`README-win32.md`](README-win32.md).

PE32+ applications run through Webexe's direct AMD64/Wasm process loader and
the built-in KERNEL32, USER32, GDI32, COMCTL32, Shell, networking, and provider
boundaries. No Wine loader, wineserver, Wine prefix, or Wine display driver is
part of this path. `rice-wine` remains only a compatibility-facing command
alias for launching the direct runtime.

`tests/x11-elf-lifecycle.S` is compiled as a real static i386 ELF and executed
by Webexe. It interns an atom, creates/maps a window, selects events, sets a
property, and validates geometry/tree/property replies through the same
`socketcall`, AF_UNIX, cross-Worker transport, blocking read, and write path
used by Xlib clients. The test starts no X server process: its peer is
`rice-display.wasm`.

This is not yet a claim of complete IceWM or Xorg parity. GTK2 and GTK3 are
now proven across real unchanged applications, but GTK4 has not yet passed an
unchanged i386 fixture. Qt window rendering uses the toolkit-neutral X11
boundary, but matching application-content colors still requires and has not
yet proven a Qt palette/style bridge. XKEYBOARD, complete XI2/grab behavior, MIT-SHM,
clipboard/drag-and-drop/tray edges, remaining IceWM policy, multi-output
scaling, and the accelerated media/GL path remain release gates. See
[STATUS.md](STATUS.md) for the current evidence and explicit gaps.

The X11 wire implementation is pinned to YSERVER's `yserver-protocol` crate.
See [docs/YSERVER_AUDIT.md](docs/YSERVER_AUDIT.md) for the portability
decision and [PLAN.md](PLAN.md) for the complete delivery plan.

## Build and test

```sh
cargo test --manifest-path ricewm/Cargo.toml --workspace
cargo clippy --manifest-path ricewm/Cargo.toml --workspace --all-targets -- -D warnings
cargo build --manifest-path ricewm/Cargo.toml \
  --package rice-display \
  --target wasm32-unknown-unknown \
  --release
node ricewm/tests/wasm-smoke.mjs
node ricewm/tests/canvas-renderer.test.mjs
node ricewm/tests/local-host-service.test.mjs
node ricewm/tests/native-process-x11-lifecycle.test.mjs
ricewm/tests/test-x11-elf-handshake.sh
ricewm/tests/test-x11-elf-lifecycle.sh
```

Build the browser-served artifact with:

```sh
ricewm/build-web.sh
# Equivalent convenience entry point:
wasm-rust/prepare_rootfs.sh --web-only
```

Use this for normal RiceWM browser development. It incrementally compiles the
RiceWM Wasm crates, copies their Wasm output into `ricewm/web`, and refreshes
the browser asset catalogs. The development nginx configuration mounts that
directory directly, so it does not need a rootfs rebuild. Edits made directly
to the `.mjs` files already in `ricewm/web` need no build at all.

The generated browser payload remains under the dedicated RiceWM directory:

```text
ricewm/web/rice-display-core.mjs
ricewm/web/rice-display-service.mjs
ricewm/web/rice-browser-vfs-adapter.mjs
ricewm/web/rice-guest-path.mjs
ricewm/web/rice-native-app-service.mjs
ricewm/web/rice-native-canvas-window.mjs
ricewm/web/rice-native-runtime.mjs
ricewm/web/rice-wasi-preview1.mjs
ricewm/web/rice-native-window-stack.mjs
ricewm/web/rice-native-ui-bridge.mjs
ricewm/web/rice-vfs-provider.mjs
ricewm/web/rice-vfs-wire-core.mjs
ricewm/web/rice-vfs-wire.mjs
ricewm/web/rice_apps.wasm
ricewm/web/rice_display.wasm
ricewm/web/assets/fonts/
ricewm/web/assets/icons/ricewm.css
ricewm/web/assets/icons/ricewm.woff2
ricewm/web/assets/icons/new-icons/ # optional SVG fallbacks awaiting font inclusion
ricewm/web/assets/img/
ricewm/web/assets/themes/
```

`prepare_rootfs.sh` rebuilds this payload and transactionally publishes the
exact validated tree to `site/ricewm` with the rootfs, runtime Wasm, and asset
release marker. The development nginx compose configuration continues to
mount `ricewm/web` directly at `/ricewm`.

The full command caches the Ubuntu/APT rootfs by its actual build inputs and
keeps the shared-runtime Cargo target between runs. Rust runtime or browser
code changes therefore reuse the packaged rootfs instead of reinstalling its
APT packages. Run `wasm-rust/prepare_rootfs.sh --clean-rootfs` only after you
want to refresh the base Ubuntu/APT snapshot; changing a rootfs helper,
desktop payload, profile, or Docker image invalidates the cache automatically.

### Files preferences

Open **Edit → Preferences** in Files to enable thumbnails, set their maximum file
size (16 MiB–8 GiB, initially 64 MiB), show hidden files, sort folders first, or
choose the default icon size and preview-pane visibility. Changes are saved with
**Save**; **Cancel** keeps the previous preferences. **View → Ascending / Descending**
works in icon and detail views. Hold **Ctrl** while scrolling to change zoom.

The hosted desktop stores thumbnail frames in
`~/.cache/ricewm/thumbnails-v1`, bounded to 64 MiB and 1,024 entries. Reopening a
folder or restarting Files reuses cached frames; changing a file's size or
modification time invalidates its thumbnail. Cache failures fall back to decoding.
Images and movies use host URLs for thumbnail decoding, so the VFS bridge does
not copy an entire movie merely to display one frame.

### Cloud storage in Rice Files

The browser desktop exposes **File → Connect to Server… → Cloud storage**,
with Google Drive, Nextcloud and S3-compatible storage. Google authorization uses the existing central Google OAuth
broker and configured client ID. The client must have the Drive API enabled and
be allowed to request `https://www.googleapis.com/auth/drive`. That scope is
needed to browse existing files; `drive.file` only exposes files selected or
created through the application. See Google's [scope guide](https://developers.google.com/workspace/drive/api/guides/api-specific-auth).

`rice-cloud-providers.mjs` is the composition root. Register an ID, display label, optional icon and mount label,
optional connection fields/help, and factory in `RiceCloudProviderRegistry` to add a provider. The host sends the
catalog to Files over `MountCloudProviders`; Files has no vendor list. This uses
GoF Factory Method for construction, Strategy for provider operations, Adapter
for the existing VFS protocol, and the existing Composite mount host and VFS
router for storage selection. A strategy implements `connect`, `stat`,
`directory`, `read`, `write`, `mkdir`, `rename`, `remove`, `sync`, and `close`.
`connect` returns the stable account ID and display label. Provider errors use
POSIX codes; `EAUTH` requests reconnection. Cloud/local copies and moves reuse
`RiceCrossMountTransfer`.

Google Drive supports My Drive browsing, range reads, folders, uploads, edits,
renames, moves, copies, and trash. Editor replacement saves preserve the target
Drive ID and publish its new content before trashing the temporary source.
Paths escape reserved characters and disambiguate duplicate names with Drive
IDs. Google documents appear as read-only Office/PDF exports; save edits under
a new filename. Other Google-native items appear as `.url` links. Google limits
[exports to 10 MB](https://developers.google.com/workspace/drive/api/reference/rest/v3/files/export).
The initial adapter bounds read-modify-upload editing and replacement to 64 MiB
per file; ordinary binary downloads use ranges. Remote mutations are online
operations, not an offline sync queue or a POSIX atomic filesystem.

Listings never download native-document exports. File lookups share folder
metadata for up to 30 seconds; refreshing a folder fetches a fresh listing,
and mutations invalidate the cache. Image and PDF reads share a bounded 64 MiB
range cache, coalesce duplicate reads, and use at most four concurrent transfers.
Remote PDFs open through the same media filesystem as images (64 MiB document
limit). The saved Drive is presented as “My Google Drive” with its provider icon,
including bookmarks made by earlier versions that contained an email label.

Only provider/account identifiers, labels and non-secret connection settings persist in the selected environment
at `/root/.config/rice/cloud-drives.json`. OAuth access and refresh tokens stay in
session memory and are cleared on disconnect, environment restore, or shutdown.
After reload, choose the saved sidebar drive and reconnect with the same account.
Nextcloud asks for the server base URL, username and an app password generated
in the user's Nextcloud personal settings → Security. It discovers the account
through OCS and uses WebDAV for file operations. No publisher account is needed.

S3 asks for the user's endpoint, existing bucket, region and access/secret keys;
optional session tokens and a folder prefix are supported. It signs requests
with AWS Signature V4 and uses path-style bucket URLs. Compatible services must
support ListObjectsV2, ranged GET, PUT, HEAD, CopyObject and DELETE. For AWS use
the bucket's regional endpoint; for R2 use its endpoint and region `auto`.
The bucket owner bears any storage/request charges. This integration does not
create a bucket, enable billing, or supply a shared publisher storage account.
S3 folders are key prefixes; folder moves copy all objects before removing
sources and are not atomic. Listings/moves are limited to 10,000 objects,
single-object copy to 5 GiB, and editing/upload replacement to 64 MiB. Versioned
buckets can retain old versions after deletion according to bucket policy.

Both providers cache/coalesce listings and use the shared remote media reader.
Passwords and S3 keys remain session-only: saved sidebar connections ask for
credentials again after reload. Credentials are masked in the dialog, excluded
from bookmarks, and cleared after failures/disconnect. HTTPS is required except
for loopback development servers. Storage redirects are rejected. Connections
use the selected Rice network transport, avoiding server CORS configuration.

OneDrive is deferred because a Microsoft Entra app registration is not configured.
Native Linux desktop composition has not yet been wired to the cloud registry.

Validation from the repository root:

```sh
node --test ricewm/tests/cloud-drive.test.mjs ricewm/tests/cloud-storage.test.mjs ricewm/tests/rice-google-oauth-bowl.test.mjs ricewm/tests/data-provider.test.mjs ricewm/tests/rice-auth-network.test.mjs
cargo test --manifest-path ricewm/Cargo.toml -p rice-apps --lib files::
cargo test --manifest-path ricewm/Cargo.toml -p rice-native-app-protocol --lib
RICE_APPS_WASM=/path/to/rice_apps.wasm node ricewm/tests/cloud-files-chromium.mjs
```

The Chromium journey uses real Files/Paper Wasm, DOM input and durable browser
storage with a deterministic Drive API fixture. It does not prove live Google
consent, API enablement, account access, or public deployment.

The additional `cloud-storage-live.mjs` and `cloud-storage-chromium.mjs` tests
exercise disposable real Nextcloud and S3 servers, including browser connection
fields, PDF/PNG decoding, Paper edit/save, cross-provider copies and reconnects.
See [the local storage test setup](tests/cloud-storage-servers.md). These tests
use local service accounts; they do not certify every hosted S3/Nextcloud vendor.
