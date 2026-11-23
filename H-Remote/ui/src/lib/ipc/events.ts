import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { mockSubscribe } from "./mock";
import { useMockBackend } from "./runtime";

/** Tracks teardown callbacks so every listener is released exactly once. */
export class CleanupRegistry {
  private readonly items: Array<() => void> = [];
  private disposed = false;

  add(cleanup: () => void): void {
    if (this.disposed) {
      cleanup();
      return;
    }
    this.items.push(cleanup);
  }

  get size(): number {
    return this.items.length;
  }

  dispose(): void {
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    while (this.items.length > 0) {
      try {
        this.items.pop()?.();
      } catch (error) {
        console.error("cleanup failed", error);
      }
    }
  }
}

/**
 * Subscribes to a backend event and registers teardown.
 *
 * The async `listen` result is guarded: if the component is destroyed before
 * registration resolves, the listener is unregistered immediately. This
 * prevents the classic Tauri/Svelte listener leak.
 */
export function subscribe<T>(
  event: string,
  handler: (payload: T) => void,
  registry: CleanupRegistry,
): void {
  if (useMockBackend) {
    registry.add(mockSubscribe<T>(event, handler));
    return;
  }

  let unlisten: UnlistenFn | undefined;
  let disposed = false;
  void listen<T>(event, (message) => handler(message.payload))
    .then((dispose) => {
      if (disposed) {
        dispose();
      } else {
        unlisten = dispose;
      }
    })
    .catch((error) => {
      console.error(`cannot subscribe to ${event}`, error);
    });

  registry.add(() => {
    disposed = true;
    unlisten?.();
    unlisten = undefined;
  });
}
