# rice-desktop

`rice-desktop` is the pure, dependency-free policy core for RiceWM's native
desktop. It deliberately contains no filesystem calls, image decoder, Wasm
ABI, renderer, or window-manager code. Those adapters consume
`DesktopIntent`; the same commands and results can therefore be tested on the
host and reused by the eventual Wasm application.

The design keeps responsibilities small:

- `model`: Aggregate and Command dispatcher; semantic items and adapter
  intents.
- `wallpaper`: Strategy-backed wallpaper geometry and per-workspace settings.
- `grid`: deterministic placement plus collision-safe resize reconciliation.
- `selection`: click/range/rubber-band/keyboard selection policy.
- `menu`: context-menu policy for background and item selections.
- `events`: bounded filesystem and mount event coalescing.

The reusable seams are intentional rather than pattern decoration:
`DesktopCommand` is the Command boundary for all state changes;
`WallpaperLayoutStrategy`, `GridPlacementStrategy`, and
`ContextMenuPolicy` isolate replaceable policy; `DesktopModel` is the
aggregate that protects invariants; and `DesktopIntent` is the ports/adapters
boundary. Event coalescing acts as a bounded unit of work. None of these
interfaces owns browser, filesystem, renderer, or window-manager machinery.

No production source file may exceed 600 lines. `tests/architecture.rs`
enforces that boundary and also rejects dependency additions unless the
manifest allowlist is deliberately changed.

## Parity scenario map

| Contract | Implemented policy |
| --- | --- |
| DESK-001 | Solid color plus center, tile, stretch, fit, and crop geometry |
| DESK-002 | Common wallpaper and stable per-workspace overrides |
| DESK-003 | Home/Documents/Pictures/Trash/mount/file/folder/application semantics and actions |
| DESK-004 | Deterministic grid, persisted positions, collision resolution, range/rubber-band/keyboard selection, rename intent |
| DESK-006 | Background/single/multiple/read-only context-menu policy |
| DESK-007 | Bounded create/delete/rename/metadata event coalescing |
| DESK-008 | Mount arrival/removal coalescing and adapter notifications |

This is the model foundation, not a claim that rendering or host integration
for these scenarios is complete.
