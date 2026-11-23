// Minimal observable store. Subscribers must return their unsubscribe function
// and views call it on unmount, so listeners never accumulate.

export function createStore(initial = {}) {
  let state = { ...initial };
  const listeners = new Set();

  return {
    get() {
      return state;
    },
    set(patch) {
      state = { ...state, ...patch };
      for (const listener of [...listeners]) {
        try {
          listener(state);
        } catch (error) {
          console.error('store listener failed', error);
        }
      }
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}
