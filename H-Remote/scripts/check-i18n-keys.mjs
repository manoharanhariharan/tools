// Verifies that every literal translation key used by Rust and Svelte exists in
// locales/en-US.ftl. Dynamic keys are skipped; they still fall back at runtime.
import { readFile, readdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const catalog = await readFile(join(root, "locales", "en-US.ftl"), "utf8");
const defined = new Set(
  catalog
    .split(/\r?\n/)
    .map((line) => line.match(/^([A-Za-z][A-Za-z0-9_-]*)\s*=/)?.[1])
    .filter(Boolean),
);

const sources = [];
async function walk(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      if (["node_modules", "target", "dist", "test-results"].includes(entry.name)) {
        continue;
      }
      await walk(path);
    } else if (/\.(rs|svelte|ts)$/.test(entry.name) && !/\.test\.ts$/.test(entry.name)) {
      sources.push(path);
    }
  }
}

await walk(join(root, "ui", "src"));
await walk(join(root, "crates"));
await walk(join(root, "src-tauri", "src"));

const usages = new Map();
// `has` is only a translation lookup when called on the i18n store; a plain
// `set.has("key")` must not be treated as a translation key.
const pattern =
  /(?:\bi18n\.has|\b(?:t|t_args))\(\s*["']([A-Za-z][A-Za-z0-9_.-]*)["']/g;
for (const path of sources) {
  const source = await readFile(path, "utf8");
  for (const match of source.matchAll(pattern)) {
    usages.set(match[1], path);
  }
}

const missing = [];
const ignored = new Set(["does.not.exist"]);
for (const [key, path] of usages) {
  if (ignored.has(key)) {
    continue;
  }
  const id = key.replaceAll(".", "-");
  if (!defined.has(id)) {
    missing.push(`${key} (${path.replace(root, "")})`);
  }
}

if (missing.length > 0) {
  console.error("Missing translation keys in locales/en-US.ftl:");
  for (const item of missing.sort()) {
    console.error(`  - ${item}`);
  }
  process.exit(1);
}

console.log(`i18n keys OK (${usages.size} literal keys checked).`);
