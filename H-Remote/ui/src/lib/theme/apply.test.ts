import { describe, expect, it } from "vitest";
import type { ResolvedTheme } from "../ipc/contracts";
import { mockPresets } from "../ipc/mock";
import { applyTheme, isDarkTheme, themeVariables, toCssColor } from "./apply";

const forest = mockPresets.find((preset) => preset.name === "Forest")!;
const light = mockPresets.find((preset) => preset.name === "Light")!;

function resolved(name = "Forest"): ResolvedTheme {
  const preset = mockPresets.find((candidate) => candidate.name === name)!;
  return {
    id: preset.name,
    displayName: preset.displayName,
    colors: preset.colors,
    fontFamily: "Inter",
    fontSize: 14,
  };
}

describe("theme helpers", () => {
  it("converts ARGB reference colours to CSS RGBA", () => {
    expect(toCssColor("#FF007ACC")).toBe("#007ACC");
    expect(toCssColor("#80112233")).toBe("#11223380");
    expect(toCssColor("#aabbcc")).toBe("#AABBCC");
    expect(toCssColor("nonsense")).toBe("#000000");
    expect(toCssColor("#12345")).toBe("#000000");
  });

  it("clamps derived font sizes to the 10-20px range", () => {
    const variables = themeVariables({ ...resolved(), fontSize: 99 });
    expect(variables["--hr-font-size-body"]).toBe("20px");
    expect(variables["--hr-font-size-title"]).toBe("20px");
    expect(variables["--hr-font-size-small"]).toBe("18px");
  });

  it("detects dark and light backgrounds", () => {
    expect(isDarkTheme(resolved("Dark"))).toBe(true);
    expect(isDarkTheme({ ...resolved("Light"), colors: light.colors })).toBe(false);
  });

  it("applies CSS variables and metadata to an element", () => {
    const root = document.createElement("div");
    applyTheme(resolved(), root);
    expect(root.style.getPropertyValue("--hr-accent-mid")).toBe(
      toCssColor(forest.colors.accentMid),
    );
    expect(root.style.getPropertyValue("--hr-font-size-body")).toBe("14px");
    expect(root.dataset.theme).toBe("Forest");
    expect(root.dataset.colorScheme).toBe("light");

    applyTheme(resolved("Dark"), root);
    expect(root.dataset.colorScheme).toBe("dark");
  });

  it("keeps every preset usable", () => {
    for (const preset of mockPresets) {
      const variables = themeVariables({
        id: preset.name,
        displayName: preset.displayName,
        colors: preset.colors,
        fontFamily: "system-ui",
        fontSize: 12,
      });
      expect(variables["--hr-background"]).toMatch(/^#[0-9A-F]{6}([0-9A-F]{2})?$/);
    }
  });
});
