// Capture a PNG of the running app over the Chrome DevTools Protocol.
// usage: node ui-shot.mjs <out.png> [navLabel]
const [, , outPath, navLabel, action] = process.argv;
const BASE = "http://127.0.0.1:9222";

const targets = await (await fetch(`${BASE}/json`)).json();
const page = targets.find((t) => t.type === "page");
if (!page) {
  console.error("no page target; is the app running with --remote-debugging-port=9222?");
  process.exit(1);
}

const ws = new WebSocket(page.webSocketDebuggerUrl);
let seq = 0;
const pending = new Map();
const send = (method, params = {}) =>
  new Promise((resolve, reject) => {
    const id = ++seq;
    pending.set(id, resolve);
    ws.send(JSON.stringify({ id, method, params }));
    setTimeout(() => {
      if (pending.has(id)) {
        pending.delete(id);
        reject(new Error(`${method} timed out`));
      }
    }, 15000);
  });

ws.addEventListener("message", (event) => {
  const msg = JSON.parse(event.data);
  if (msg.id && pending.has(msg.id)) {
    pending.get(msg.id)(msg);
    pending.delete(msg.id);
  }
});

await new Promise((resolve) => ws.addEventListener("open", resolve));
await send("Page.enable");

// Wait for the instrument master before capturing, otherwise a screenshot can
// catch the app mid-load and show an empty grid.
const evalValue = async (expression) => {
  const res = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  return res.result?.result?.value;
};
for (let attempt = 0; attempt < 40; attempt++) {
  const status = await evalValue(
    'window.__TAURI_INTERNALS__ ? window.__TAURI_INTERNALS__.invoke("status").then(s => JSON.stringify(s)) : "no-ipc"',
  );
  if (typeof status === "string" && status.includes('"master_loaded":true')) break;
  await new Promise((r) => setTimeout(r, 500));
}

if (navLabel) {
  const expr = `(() => {
    const b = [...document.querySelectorAll("button")].find(x => x.textContent.trim() === ${JSON.stringify(navLabel)});
    if (!b) return "not-found";
    b.click();
    return "clicked";
  })()`;
  const res = await send("Runtime.evaluate", { expression: expr, returnByValue: true });
  const status = res.result?.result?.value;
  console.log(`nav "${navLabel}": ${status}`);
  await new Promise((r) => setTimeout(r, 900));
}

// Optional UI drive step, e.g. pressing "Load and subscribe" on the chain.
if (action) {
  const res = await send("Runtime.evaluate", {
    expression: action,
    awaitPromise: true,
    returnByValue: true,
  });
  console.log(`action: ${res.result?.result?.value ?? "no-result"}`);
  await new Promise((r) => setTimeout(r, 2500));
}

const shot = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
if (!shot.result?.data) {
  console.error("capture failed:", JSON.stringify(shot).slice(0, 300));
  process.exit(1);
}

const fs = await import("node:fs");
fs.writeFileSync(outPath, Buffer.from(shot.result.data, "base64"));
console.log(`wrote ${outPath}`);
ws.close();
process.exit(0);
