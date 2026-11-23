/**
 * Browser key → X11 keycode mapping for VNC. VNC transports X11 keycodes, not
 * keysyms; these values follow the standard X11 evdev layout used by common
 * Linux/macOS VNC servers. Keys outside the table are ignored rather than
 * guessed, so a wrong key never appears on a remote machine.
 */
const LETTER_ROW_1: Record<string, number> = {
  q: 24,
  w: 25,
  e: 26,
  r: 27,
  t: 28,
  y: 29,
  u: 30,
  i: 31,
  o: 32,
  p: 33,
};
const LETTER_ROW_2: Record<string, number> = {
  a: 38,
  s: 39,
  d: 40,
  f: 41,
  g: 42,
  h: 43,
  j: 44,
  k: 45,
  l: 46,
};
const LETTER_ROW_3: Record<string, number> = {
  z: 52,
  x: 53,
  c: 54,
  v: 55,
  b: 56,
  n: 57,
  m: 58,
};
const DIGITS: Record<string, number> = {
  "1": 10,
  "2": 11,
  "3": 12,
  "4": 13,
  "5": 14,
  "6": 15,
  "7": 16,
  "8": 17,
  "9": 18,
  "0": 19,
};

const NAMED: Record<string, number> = {
  Backspace: 22,
  Tab: 23,
  Enter: 36,
  Shift: 50,
  Control: 37,
  Alt: 64,
  Meta: 133,
  Escape: 9,
  " ": 65,
  ArrowLeft: 113,
  ArrowUp: 111,
  ArrowRight: 114,
  ArrowDown: 116,
  Home: 110,
  End: 115,
  PageUp: 112,
  PageDown: 117,
  Insert: 118,
  Delete: 119,
  CapsLock: 66,
  "-": 20,
  "=": 21,
  "[": 34,
  "]": 35,
  "\\": 51,
  ";": 47,
  "'": 48,
  ",": 59,
  ".": 60,
  "/": 61,
  "`": 49,
};

const FUNCTION: Record<string, number> = Object.fromEntries(
  Array.from({ length: 12 }, (_, index) => [`F${index + 1}`, 67 + index]),
);

export function x11Keycode(key: string): number | null {
  const lower = key.toLowerCase();
  return (
    LETTER_ROW_1[lower] ??
    LETTER_ROW_2[lower] ??
    LETTER_ROW_3[lower] ??
    DIGITS[lower] ??
    NAMED[key] ??
    FUNCTION[key] ??
    null
  );
}

/** Browser button mask → RFB button mask (bit 1 left, 2 middle, 4 right). */
export function x11Buttons(buttons: number): number {
  let mask = 0;
  if (buttons & 1) mask |= 1;
  if (buttons & 4) mask |= 2;
  if (buttons & 2) mask |= 4;
  return mask;
}

export interface ContainRect {
  width: number;
  height: number;
  offsetX: number;
  offsetY: number;
  scale: number;
}

/**
 * Geometry of a frame rendered with `object-fit: contain` inside a box.
 * Shared by the canvas sizing and the pointer mapping so they can never drift
 * apart (which previously shifted the remote cursor).
 */
export function containRect(
  containerWidth: number,
  containerHeight: number,
  width: number,
  height: number,
): ContainRect | null {
  if (containerWidth <= 0 || containerHeight <= 0 || width <= 0 || height <= 0) {
    return null;
  }
  const scale = Math.min(containerWidth / width, containerHeight / height);
  const renderedWidth = width * scale;
  const renderedHeight = height * scale;
  return {
    width: renderedWidth,
    height: renderedHeight,
    offsetX: (containerWidth - renderedWidth) / 2,
    offsetY: (containerHeight - renderedHeight) / 2,
    scale,
  };
}

export function surfaceCoordinates(
  event: { clientX: number; clientY: number },
  target: DOMRect,
  width: number,
  height: number,
): { x: number; y: number } | null {
  const box = containRect(target.width, target.height, width, height);
  if (!box) {
    return null;
  }
  const localX = event.clientX - target.left - box.offsetX;
  const localY = event.clientY - target.top - box.offsetY;
  if (
    localX < 0 ||
    localY < 0 ||
    localX > box.width ||
    localY > box.height
  ) {
    return null;
  }
  return {
    x: Math.max(0, Math.min(width - 1, Math.round(localX / box.scale))),
    y: Math.max(0, Math.min(height - 1, Math.round(localY / box.scale))),
  };
}

/** Backwards-compatible alias used by the VNC input path. */
export const vncCoordinates = surfaceCoordinates;
