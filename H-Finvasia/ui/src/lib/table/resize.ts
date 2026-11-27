export interface ColumnResizeParams {
  width: number;
  min?: number;
  max?: number;
  onresize: (width: number) => void;
  onend?: (width: number) => void;
}

/**
 * Pointer-drag a column edge. Listeners live on `window` only while dragging,
 * so nothing is retained after pointer-up or component teardown.
 */
export function columnResize(node: HTMLElement, params: ColumnResizeParams) {
  let current = params;

  const start = (event: PointerEvent) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    const startX = event.clientX;
    const startWidth = current.width;
    let latest = startWidth;

    const move = (moveEvent: PointerEvent) => {
      latest = Math.max(
        current.min ?? 40,
        Math.min(current.max ?? 640, startWidth + moveEvent.clientX - startX),
      );
      current.onresize(latest);
    };
    const finish = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", finish);
      window.removeEventListener("pointercancel", finish);
      current.onend?.(latest);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", finish);
    window.addEventListener("pointercancel", finish);
  };

  node.addEventListener("pointerdown", start);
  return {
    update(next: ColumnResizeParams) {
      current = next;
    },
    destroy() {
      node.removeEventListener("pointerdown", start);
    },
  };
}
