import { describe, expect, it } from "vitest";
import { showTestPlaque } from "./appInfo";
import { t } from "../i18n";

describe("showTestPlaque", () => {
  it("shows the plaque for the test database", () => {
    expect(showTestPlaque({ mode: "test" })).toBe(true);
  });

  it("hides the plaque for the real database and before info is loaded", () => {
    expect(showTestPlaque({ mode: "real" })).toBe(false);
    expect(showTestPlaque(null)).toBe(false);
  });
});

describe("dictionary", () => {
  it("has no empty texts", () => {
    const walk = (value: unknown, path: string): void => {
      if (typeof value === "string") {
        expect(value.trim(), path).not.toBe("");
      } else if (value && typeof value === "object") {
        for (const [key, child] of Object.entries(value)) walk(child, `${path}.${key}`);
      }
    };
    walk(t, "t");
  });
});
