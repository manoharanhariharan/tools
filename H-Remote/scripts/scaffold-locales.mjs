// Generates scaffolded locale catalogs without touching translated files.
// Run with: node scripts/scaffold-locales.mjs
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "locales");

const locales = {
  "zh-CN": "简体中文",
  "zh-TW": "繁體中文",
  "ja-JP": "日本語",
  "ru-RU": "Русский",
  "de-DE": "Deutsch",
  "fr-FR": "Français",
  "it-IT": "Italiano",
  "es-AR": "Español (Argentina)",
  "gl-ES": "Galego",
  "pl-PL": "Polski",
  "pt-BR": "Português (Brasil)",
  "pt-PT": "Português (Portugal)",
  "cs-CZ": "Čeština",
};

await mkdir(root, { recursive: true });

for (const [id, name] of Object.entries(locales)) {
  const path = join(root, `${id}.ftl`);
  const body = [
    `# H-Remote — ${name} (${id})`,
    "# Scaffolded locale: untranslated keys fall back to en-US.",
    "# Translate by adding the same message keys as locales/en-US.ftl.",
    "",
    "app.name = H-Remote",
    "",
  ].join("\n");
  await writeFile(path, body, "utf8");
  console.log(`wrote ${path}`);
}
