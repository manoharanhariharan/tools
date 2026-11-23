// Generates a JSON manifest for the in-app update checker (P9-10/P11-12).
//
//   node scripts/update-manifest.mjs \
//     --version 26.11.29.1 \
//     --url https://example.invalid/h-remote-v26.11.29.1.exe \
//     --notes "Fixes" \
//     --minimum-version 26.10.1.1 \
//     [--prerelease] [--out dist/latest.json]

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const args = process.argv.slice(2);
const value = (name, fallback) => {
  const index = args.indexOf(`--${name}`);
  return index >= 0 && index + 1 < args.length ? args[index + 1] : fallback;
};

const version = value("version", process.env.HREMOTE_BUILD_VERSION ?? "").replace(/^v/, "");
if (!version) {
  console.error("--version is required");
  process.exit(1);
}
const url = value("url", "");
if (!url) {
  console.error("--url is required");
  process.exit(1);
}
const out = resolve(value("out", "dist/latest.json"));
const manifest = {
  version,
  notes: value("notes", ""),
  url,
  publishedAt: Math.floor(Date.now() / 1000),
  prerelease: args.includes("--prerelease"),
};
const minimum = value("minimum-version", "");
if (minimum) {
  manifest.minimumVersion = minimum;
}

mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
console.log(`update manifest written to ${out}`);
