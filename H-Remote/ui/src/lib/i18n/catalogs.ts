// Explicit catalog imports keep locales working in Vite, Vitest and Tauri
// without relying on glob paths outside the frontend root.
import enUS from "../../../../locales/en-US.ftl?raw";
import zhCN from "../../../../locales/zh-CN.ftl?raw";
import zhTW from "../../../../locales/zh-TW.ftl?raw";
import jaJP from "../../../../locales/ja-JP.ftl?raw";
import ruRU from "../../../../locales/ru-RU.ftl?raw";
import deDE from "../../../../locales/de-DE.ftl?raw";
import frFR from "../../../../locales/fr-FR.ftl?raw";
import itIT from "../../../../locales/it-IT.ftl?raw";
import esAR from "../../../../locales/es-AR.ftl?raw";
import glES from "../../../../locales/gl-ES.ftl?raw";
import plPL from "../../../../locales/pl-PL.ftl?raw";
import ptBR from "../../../../locales/pt-BR.ftl?raw";
import ptPT from "../../../../locales/pt-PT.ftl?raw";
import csCZ from "../../../../locales/cs-CZ.ftl?raw";

export const catalogs: Record<string, string> = {
  "en-US": enUS,
  "zh-CN": zhCN,
  "zh-TW": zhTW,
  "ja-JP": jaJP,
  "ru-RU": ruRU,
  "de-DE": deDE,
  "fr-FR": frFR,
  "it-IT": itIT,
  "es-AR": esAR,
  "gl-ES": glES,
  "pl-PL": plPL,
  "pt-BR": ptBR,
  "pt-PT": ptPT,
  "cs-CZ": csCZ,
};
