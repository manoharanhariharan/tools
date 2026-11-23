// Writes BUILD.txt + SHA256SUMS.txt for a staged release directory.
//
//   node scripts/build-info.mjs --tag v26.11.22.1 --dir release/v26.11.22.1 \
//        --signed yes|no|unknown [--notes "..."]

import { resolve } from "node:path";
import { writeBuildInfo } from "./release-lib.mjs";

const args = process.argv.slice(2);
const value = (name, fallback) => {
  const index = args.indexOf(`--${name}`);
  return index >= 0 && index + 1 < args.length ? args[index + 1] : fallback;
};

const root = resolve(import.meta.dirname, "..");
const tag = value("tag", "local");
const dir = resolve(value("dir", resolve(root, "release", tag)));
const signed = value("signed", "unknown");
const notes = value("notes", "");

try {
  const artifacts = writeBuildInfo({ root, dir, tag, signed, notes });
  console.log(`BUILD.txt written for ${artifacts.length} artifact(s) in ${dir}`);
  for (const artifact of artifacts) {
    console.log(`${artifact.sha256}  ${artifact.file}`);
  }
} catch (error) {
  console.error(`build-info failed: ${error.message}`);
  process.exitCode = 1;
}
