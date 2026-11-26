// Capture a PNG of the running app over the Chrome DevTools Protocol.
// usage: node ui-shot.mjs <out.png> [navLabel]
const [, , outPath, navLabel] = process.argv;
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
