import { describe, expect, it } from "vitest";
import { containRect, vncCoordinates, x11Buttons, x11Keycode } from "./vnc";

describe("VNC input mapping", () => {
  it("maps letters, digits and named keys", () => {
    expect(x11Keycode("a")).toBe(38);
    expect(x11Keycode("A")).toBe(38);
    expect(x11Keycode("0")).toBe(19);
    expect(x11Keycode("Enter")).toBe(36);
    expect(x11Keycode("ArrowLeft")).toBe(113);
    expect(x11Keycode("F12")).toBe(78);
    expect(x11Keycode("UnmappedKey")).toBeNull();
  });

  it("translates browser buttons to RFB masks", () => {
    expect(x11Buttons(0)).toBe(0);
    expect(x11Buttons(1)).toBe(1);
    expect(x11Buttons(2)).toBe(4);
    expect(x11Buttons(4)).toBe(2);
    expect(x11Buttons(3)).toBe(5);
  });

  it("computes the contained frame geometry once for sizing and input", () => {
    expect(containRect(400, 300, 200, 100)).toEqual({
      width: 400,
      height: 200,
      offsetX: 0,
      offsetY: 50,
      scale: 2,
    });
    expect(containRect(300, 400, 200, 100)).toEqual({
      width: 300,
      height: 150,
      offsetX: 0,
      offsetY: 125,
      scale: 1.5,
    });
    expect(containRect(0, 300, 200, 100)).toBeNull();
    expect(containRect(400, 300, 0, 100)).toBeNull();
  });

  it("maps pointer coordinates through object-fit contain", () => {    const rect = {
      left: 10,
      top: 20,
      width: 400,
      height: 300,
    } as DOMRect;
    // 200x100 image inside 400x300 → scale 2, vertical offset 50.
    expect(vncCoordinates({ clientX: 110, clientY: 170 }, rect, 200, 100)).toEqual({
      x: 50,
      y: 50,
    });
    // Clicking the letterbox area is ignored.
    expect(vncCoordinates({ clientX: 110, clientY: 30 }, rect, 200, 100)).toBeNull();
    expect(vncCoordinates({ clientX: 10, clientY: 20 }, rect, 0, 0)).toBeNull();
  });
});
