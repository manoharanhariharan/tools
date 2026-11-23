export const isTauriRuntime =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Playwright and plain-browser development use the in-memory backend. */
export const useMockBackend =
  import.meta.env.VITE_E2E === "true" || !isTauriRuntime;
