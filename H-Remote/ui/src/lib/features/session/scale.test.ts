import { describe, expect, it } from "vitest";
import { physicalPixels } from "./scale";

describe("physical session pixels", () => {
  it("scales CSS measurements by the device pixel ratio", () => {
    expect(physicalPixels(1280, 720, 1)).toEqual({ width: 1280, height: 720 });
    expect(physicalPixels(1280, 720, 1.25)).toEqual({ width: 1600, height: 900 });
    expect(physicalPixels(1920, 1080, 2)).toEqual({ width: 3840, height: 2160 });
  });

  it("falls back to CSS pixels for an invalid ratio", () => {
    expect(physicalPixels(800, 600, 0)).toEqual({ width: 800, height: 600 });
    expect(physicalPixels(800, 600, Number.NaN)).toEqual({ width: 800, height: 600 });
  });
});
