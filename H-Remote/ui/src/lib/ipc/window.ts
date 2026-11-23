import { isTauriRuntime } from "./runtime";

/** Minimizes the webview window that owns the calling UI. */
export async function minimizeCurrentWindow(): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().minimize();
}
