// Stages Tauri build outputs for a release job and writes BUILD.txt.
//
//   node scripts/stage-bundles.mjs --out dist/windows --tag v26.11.22.1 \
//        [--signed unknown] [--release-dir target/release]

import { copyFileSync, existsSync, mkdirSync, readdirSync, statSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import { writeBuildInfo } from "./release-lib.mjs";

const args = process.argv.slice(2);
const value = (name, fallback) => {
  const index = args.indexOf(`--${name}`);
  return index >= 0 && index + 1 < args.length ? args[index + 1] : fallback;
};

const root = resolve(import.meta.dirname, "..");
const releaseDir = resolve(value("release-dir", resolve(root, "target", "release")));
const outDir = resolve(value("out", resolve(root, "dist")));
const tag = value("tag", "local");
const signed = value("signed", "unknown");

const extensions = [".exe", ".msi", ".dmg", ".appimage", ".deb", ".rpm", ".pkg"];
mkdirSync(outDir, { recursive: true });

let copied = 0;
const copy = (path) => {
  const target = join(outDir, basename(path));
  copyFileSync(path, target);
  copied += 1;
};

for (const name of ["hremote.exe", "hremote"]) {
  const candidate = join(releaseDir, name);
  if (existsSync(candidate) && statSync(candidate).isFile()) {
    copy(candidate);
  }
}

const bundleDir = join(releaseDir, "bundle");
if (existsSync(bundleDir)) {
  const walk = (current) => {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const path = join(current, entry.name);
      if (entry.isDirectory()) {
        walk(path);
      } else if (extensions.some((extension) => entry.name.toLowerCase().endsWith(extension))) {
        copy(path);
      }
    }
  };
  walk(bundleDir);
}

if (copied === 0) {
  console.error(`no Tauri outputs found under ${releaseDir}`);
  process.exit(1);
}

try {
  const artifacts = writeBuildInfo({ root, dir: outDir, tag, signed });
  console.log(`staged ${artifacts.length} artifact(s) in ${outDir}`);
} catch (error) {
  console.error(`staging failed: ${error.message}`);
  process.exit(1);
}
