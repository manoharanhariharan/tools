// Shared release-artifact helpers used by the local release script and CI.
// Kept dependency-free so `node scripts/...` works on every runner.

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join, relative } from "node:path";

const ARTIFACT_EXTENSIONS = [
  ".exe",
  ".msi",
  ".dmg",
  ".appimage",
  ".deb",
  ".rpm",
  ".zip",
  ".tar.gz",
  ".pkg",
];

export function sha256(path) {
  const hash = createHash("sha256");
  hash.update(readFileSync(path));
  return hash.digest("hex");
}

export function listArtifacts(dir) {
  if (!existsSync(dir)) {
    return [];
  }
  const results = [];
  const walk = (current) => {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const path = join(current, entry.name);
      if (entry.isDirectory()) {
        walk(path);
        continue;
      }
      const lower = entry.name.toLowerCase();
      if (
        lower.endsWith("build.txt") ||
        lower.startsWith("sha256sums") ||
        !ARTIFACT_EXTENSIONS.some((extension) => lower.endsWith(extension))
      ) {
        continue;
      }
      results.push(path);
    }
  };
  walk(dir);
  return results.sort();
}

function toolVersion(command, args) {
  try {
    return execFileSync(command, args, { encoding: "utf8" }).trim();
  } catch {
    return "unavailable";
  }
}

function packageVersion(root) {
  try {
    const manifest = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
    return manifest.version ?? "unknown";
  } catch {
    return "unknown";
  }
}

function gitCommit(root) {
  try {
    return execFileSync("git", ["rev-parse", "--short", "HEAD"], {
      cwd: root,
      encoding: "utf8",
    }).trim();
  } catch {
    return "unknown";
  }
}

function gitDirty(root) {
  try {
    return execFileSync("git", ["status", "--porcelain", "--", "."], {
      cwd: root,
      encoding: "utf8",
    }).trim().length > 0;
  } catch {
    return true;
  }
}

/**
 * Writes BUILD.txt and SHA256SUMS.txt for every recognized artifact under
 * `dir`. Returns the list of `{ file, sha256, bytes }` entries.
 */
export function writeBuildInfo({ root, dir, tag, signed = "unknown", notes = "" }) {
  mkdirSync(dir, { recursive: true });
  const artifacts = listArtifacts(dir).map((path) => ({
    path,
    file: relative(dir, path).replaceAll("\\", "/"),
    sha256: sha256(path),
    bytes: statSync(path).size,
  }));
  if (artifacts.length === 0) {
    throw new Error(`no release artifacts found in ${dir}`);
  }

  const commit = process.env.HREMOTE_BUILD_COMMIT || gitCommit(root);
  const dirty =
    process.env.HREMOTE_BUILD_DIRTY === undefined
      ? gitDirty(root)
      : process.env.HREMOTE_BUILD_DIRTY === "true";
  const lines = [
    "H-Remote release build",
    `tag: ${tag}`,
    `version: ${packageVersion(root)}`,
    `buildVersion: ${process.env.HREMOTE_BUILD_VERSION || packageVersion(root)}`,
    `commit: ${commit}`,
    `dirty: ${dirty}`,
    `built: ${new Date().toISOString()}`,
    `node: ${process.version}`,
    `rustc: ${toolVersion("rustc", ["--version"])}`,
    `cargo: ${toolVersion("cargo", ["--version"])}`,
    `signed: ${signed}`,
    ...(notes ? [`notes: ${notes}`] : []),
    "",
    "artifacts:",
    ...artifacts.map(
      (artifact) => `${artifact.sha256}  ${artifact.bytes}  ${artifact.file}`,
    ),
    "",
  ];
  writeFileSync(join(dir, "BUILD.txt"), `${lines.join("\n")}\n`, "utf8");
  writeFileSync(
    join(dir, "SHA256SUMS.txt"),
    `${artifacts.map((artifact) => `${artifact.sha256}  ${artifact.file}`).join("\n")}\n`,
    "utf8",
  );
  return artifacts;
}
