import { openWasmSheetBackend } from "./workbook/wasm-engine.mjs";
import { createFontMetricsResolver } from "./workbook/font-metrics.mjs";
import { openTypographyEngine } from "./typography-engine.mjs";

/** Source-backed workbook state/formulas/import/export, independent from UI. */
export function openWorkbookEngine({
  services,
  path = "workbook.wasm",
  typography = {},
  resolveFontMetrics,
  ...options
} = {}) {
  const resolve =
    resolveFontMetrics ??
    createFontMetricsResolver(() =>
      openTypographyEngine({ services, ...typography }),
    );
  return openWasmSheetBackend({
    ...options,
    services,
    path,
    resolveFontMetrics: resolve,
  });
}
openWorkbookEngine.controlFactory = Object.freeze({
  topics: ["documents", "tables", "workbook", "media"],
  signature:
    'await openWorkbookEngine({services,path:"workbook.wasm",create:{},signal,typography:{fontManifest:"fonts-manifest.json"}})',
  behavior:
    'The existing source-backed workbook engine owns cells, formulas, selection, undo/redo, typed commands, queries and XLSX/ODS/CSV/TSV import/export. Render engine projections with VirtualGrid; never reimplement formulas in UI. Inspect returned capabilities and fileFormats. sdk.resources ["workbook","typography"] bundles the real engine and licensed fonts. Each open worker owns one workbook. close() releases it; use ResourcePool for many documents.',
});
