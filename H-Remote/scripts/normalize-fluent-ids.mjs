// Fluent message identifiers cannot contain dots. H-Remote keeps hierarchical
// keys in code (e.g. `common.cancel`) and stores them with dashes in `.ftl`.
// This script rewrites catalog identifiers in place; it is idempotent.
import { readdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "locales");
const files = (await readdir(root)).filter((name) => name.endsWith(".ftl"));

for (const file of files) {
  const path = join(root, file);
  const source = await readFile(path, "utf8");
  const rewritten = source
    .split(/\r?\n/)
    .map((line) => {
      const match = line.match(/^([A-Za-z][A-Za-z0-9_.-]*)(\s*=)/);
      if (!match) {
        return line;
      }
      return `${match[1].replaceAll(".", "-")}${line.slice(match[1].length)}`;
    })
    .join("\n");
  if (rewritten !== source) {
    await writeFile(path, rewritten, "utf8");
    console.log(`normalised ${file}`);
  }
}
