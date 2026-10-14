import { cp, mkdir, rm, copyFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { resolve, join } from "node:path";
const root = resolve(new URL("../", import.meta.url).pathname);
const command = spawnSync(
  "cargo",
  [
    "build",
    "--manifest-path",
    join(root, "Cargo.toml"),
    "--locked",
    "-p",
    "rice-display",
    "-p",
    "rice-apps",
    "--target",
    "wasm32-unknown-unknown",
  ],
  { cwd: root, stdio: "inherit" },
);
if (command.status !== 0) process.exit(command.status || 1);
const target = resolve(root, process.env.CARGO_TARGET_DIR || "target");
await rm(join(root, "dist"), { recursive: true, force: true });
await mkdir(join(root, "dist"), { recursive: true });
for (const name of ["web", "sdk", "index.html", "LICENSE", "licenses"])
  await cp(join(root, name), join(root, "dist", name), { recursive: true });
await copyFile(
  join(target, "wasm32-unknown-unknown/debug/rice_display.wasm"),
  join(root, "dist/web/rice_display.wasm"),
);
await copyFile(
  join(target, "wasm32-unknown-unknown/debug/rice_apps.wasm"),
  join(root, "dist/web/rice_apps.wasm"),
);
console.log(
  "Built dist/. Serve it with npm start, or upload its contents to a static host.",
);
