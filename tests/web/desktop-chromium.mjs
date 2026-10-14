import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
const { chromium } = await import(
  process.env.RICEWM_PLAYWRIGHT_MODULE || "playwright"
);
const port = Number(process.env.RICEWM_TEST_PORT || 8841),
  url = `http://127.0.0.1:${port}`;
const server = spawn(
  process.execPath,
  ["tools/serve-web.mjs", "--port", String(port)],
  { stdio: ["ignore", "pipe", "pipe"] },
);
let browser, startTimer, livePage;
try {
  await Promise.race([
    once(server.stdout, "data"),
    new Promise((_, reject) => {
      startTimer = setTimeout(
        () => reject(Error("Static server did not start")),
        10000,
      );
    }),
  ]);
  clearTimeout(startTimer);
  browser = await chromium.launch({
    headless: true,
    ...(process.env.RICEWM_CHROMIUM_EXECUTABLE
      ? { executablePath: process.env.RICEWM_CHROMIUM_EXECUTABLE }
      : {}),
    args: ["--no-sandbox"],
  });
  const context = await browser.newContext({
      viewport: { width: 1440, height: 960 },
      reducedMotion: "reduce",
    }),
    page = await context.newPage(),
    faults = [],
    missing = [],
    external = [];
  livePage = page;
  page.on("pageerror", (error) => faults.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") faults.push(message.text());
  });
  page.on("response", (response) => {
    if (response.status() >= 400) missing.push(response.url());
  });
  page.on("request", (request) => {
    if (
      !request.url().startsWith(url + "/") &&
      !request.url().startsWith("data:") &&
      !request.url().startsWith("blob:")
    )
      external.push(request.url());
  });
  await page.goto(url);
  await page.waitForFunction(() => window.ricewm?.ready);
  const shellPoint = async (text) => {
    const result = await page.waitForFunction((text) => {
      const item = ricewm.display.shellTexts().find(c=>c.text===text&&ricewm.renderer.windows.get(c.id)?.viewable);
      if(!item)return false;
      const w=ricewm.renderer.windows.get(item.id);
      return {x:w.x+item.x+Math.min(item.width/2,50),y:w.y+item.y+item.height/2};
    },text);
    return result.jsonValue();
  };
  const shellClick = async (text) => {
    const p = await shellPoint(text);
    assert(p, "Missing shell item " + text);
    await page.mouse.click(p.x, p.y);
  };
  const open = async (title) => {
    const count = await page.evaluate(() => ricewm.windows.length);
    await shellClick("RiceWM");
    const point = await shellPoint("Applications");
    assert(point, "Applications menu");
    await page.mouse.move(point.x, point.y);
    await page.waitForTimeout(160);
    await shellClick(title);
    await page.waitForFunction((count) => ricewm.windows.length > count, count);
    await page.waitForTimeout(180);
  };
  const point = async (text, emptyAfter = false) =>
    page.evaluate(
      ({ text, emptyAfter }) => {
        const s = ricewm.snapshot.windows.at(-1),
          commands = [...s.commands, ...s.popupCommands],
          index = commands.findIndex(
            (c) => c.type === "text" && c.text === text,
          );
        if (index < 0) throw Error("Missing visible control: " + text);
        const c = emptyAfter
          ? commands
              .slice(index + 1)
              .find((c) => c.type === "text" && c.text === "")
          : commands[index];
        if (!c) throw Error("Missing text field");
        return {
          x: s.frame.x + c.rect.x + c.rect.width / 2,
          y: s.frame.y + s.frame.titleHeight + c.rect.y + c.rect.height / 2,
        };
      },
      { text, emptyAfter },
    );
  const click = async (text, emptyAfter = false) => {
    const p = await point(text, emptyAfter);
    await page.mouse.click(p.x, p.y);
    await page.waitForTimeout(90);
  };
  const hasText = async (text) =>
    page.waitForFunction(
      (text) =>
        ricewm.snapshot.windows
          .at(-1)
          .commands.some((c) => c.type === "text" && c.text === text),
      text,
    );
  const titleControl = async (action) => {
    const p = await page.evaluate(async (action) => {
      const { nativeTitleControlLayout } = await import(
          "/web/rice-native-window-stack.mjs"
        ),
        w = ricewm.windows.at(-1),
        s = ricewm.snapshot.windows.find((s) => s.uiApp === w.app),
        c = nativeTitleControlLayout(
          w.frame,
          w.title,
          [],
          s.chrome.windowDecorations || {},
        ).find((c) => c.action === action);
      if (!c) throw Error("Missing title control " + action);
      return {
        x: w.frame.x + c.cell.x + c.cell.width / 2,
        y: w.frame.y + c.cell.y + c.cell.height / 2,
      };
    }, action);
    await page.mouse.click(p.x, p.y);
    await page.waitForTimeout(160);
  };
  // Both the desktop and Calc are the original Rust actors, with their actual shell menu.
  await open("Calc");
  assert.equal(
    await page.evaluate(() => ricewm.windows.at(-1).entrypoint),
    "rice-calc",
  );
  for (const text of ["7", "×", "8", "="]) await click(text);
  await hasText("56");
  const before = await page.evaluate(() => ({
    ...ricewm.windows.at(-1).frame,
  }));
  await page.mouse.move(before.x + 140, before.y + 12);
  await page.mouse.down();
  await page.mouse.move(before.x + 190, before.y + 42, { steps: 8 });
  await page.mouse.up();
  const after = await page.evaluate(() => ({ ...ricewm.windows.at(-1).frame }));
  assert.notEqual(after.x, before.x, "title-bar drag");
  await page.mouse.move(after.x + after.width - 2, after.y + after.height - 2);
  await page.mouse.down();
  await page.mouse.move(
    after.x + after.width + 38,
    after.y + after.height + 28,
    { steps: 6 },
  );
  await page.mouse.up();
  assert.ok(
    await page.evaluate(
      (w) => ricewm.windows.at(-1).frame.width > w,
      after.width,
    ),
    "window resize",
  );
  await titleControl("maximize");
  assert.equal(
    await page.evaluate(() => ricewm.windows.at(-1).maximized),
    true,
  );
  await titleControl("maximize");
  assert.equal(
    await page.evaluate(() => ricewm.windows.at(-1).maximized),
    false,
  );
  await titleControl("minimize");
  assert.equal(
    await page.evaluate(() => ricewm.windows.at(-1).minimized),
    true,
  );
  await shellClick("Calc");
  await page.waitForTimeout(150);
  assert.equal(
    await page.evaluate(() => ricewm.windows.at(-1).minimized),
    false,
  );
  await titleControl("close");
  await page.waitForFunction(() => ricewm.windows.length === 1);
  // Create and rename through the original desktop context menu and native dialog.
  await page.mouse.click(1200, 500, { button: "right" });
  await page.waitForTimeout(120);
  await click("New File");
  await page.waitForTimeout(200);
  await page.keyboard.press("Control+a");
  await page.keyboard.type("public-note.txt");
  await click("Rename");
  await page.waitForFunction(
    () =>
      !ricewm.snapshot.windows[0].dialog &&
      ricewm.snapshot.windows[0].commands.some(
        (c) => c.type === "text" && c.text === "public-note.txt",
      ),
  );
  await page.reload();
  await page.waitForFunction(() => window.ricewm?.ready);
  await hasText("public-note.txt");
  // A real desktop icon opens the original Files actor.
  const home = await point("Home");
  await page.mouse.dblclick(home.x, home.y);
  await page.waitForFunction(() =>
    ricewm.windows.some((w) => w.entrypoint === "rice-files"),
  );
  await hasText("Desktop");
  await titleControl("close");
  await page.waitForFunction(() => ricewm.windows.length === 1);
  // SDK app UI and durable settings use the same original native controls.
  await open("To Do");
  await click("Add a task", true);
  await page.keyboard.type("Read the public RiceWM source");
  await page.keyboard.press("Enter");
  await page.waitForFunction(() =>
    Object.keys(localStorage).some(
      (k) =>
        k.endsWith(".tasks") &&
        localStorage.getItem(k).includes("Read the public RiceWM source"),
    ),
  );
  await page.reload();
  await page.waitForFunction(() => window.ricewm?.ready);
  await open("To Do");
  await hasText("Read the public RiceWM source");
  await titleControl("close");
  await page.waitForFunction(() => ricewm.windows.length === 1);
  await open("Calc SDK");
  for (const text of ["7", "×", "8", "="]) await click(text);
  await hasText("56");
  await open("Charts");
  await page.waitForFunction(() =>
    ricewm.snapshot.windows.at(-1).commands.some((c) => c.type === "web-scene"),
  );
  for (const title of ["Date & time", "Table report"]) await open(title);
  await page.waitForTimeout(500);
  if (process.env.RICEWM_WEB_SCREENSHOT)
    await page.screenshot({ path: process.env.RICEWM_WEB_SCREENSHOT });
  assert.deepEqual(missing, [], "missing served assets");
  assert.deepEqual(faults, [], "browser errors");
  assert.deepEqual(
    external,
    [],
    "runtime requests must stay within the static build",
  );
  console.log(
    "PASS: original desktop, Files and Calc; real menus, drag/resize/maximize/minimize/close; native file creation and reload; persistent SDK To Do and five SDK apps; no missing assets or external runtime requests",
  );
} catch (error) {
  if (livePage) {
    await livePage
      .screenshot({
        path:
          process.env.RICEWM_WEB_SCREENSHOT ||
          "/tmp/ricewm-browser-failure.png",
      })
      .catch(() => {});
    console.error(
      JSON.stringify(
        await livePage
          .evaluate(() => ({
            status: document.querySelector("#status")?.textContent,
            windows: window.ricewm?.snapshot?.windows?.map((w) => ({
              entry: w.entrypoint,
              dialog: w.dialog,
              texts: w.commands
                .filter((c) => c.type === "text")
                .map((c) => c.text),
            })),
          }))
          .catch(() => null),
      ),
    );
  }
  throw error;
} finally {
  clearTimeout(startTimer);
  await browser?.close();
  server.kill("SIGTERM");
}
