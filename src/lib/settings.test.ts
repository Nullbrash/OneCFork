import { describe, expect, it } from "vitest";
import { clampKeep, formatSize, formatTime, passwordProblem } from "./settings";

describe("settings helpers", () => {
  it("validates new password and its repeat", () => {
    expect(passwordProblem("", "")).toBe("empty");
    expect(passwordProblem("abc", "abd")).toBe("mismatch");
    expect(passwordProblem("abc", "abc")).toBeNull();
  });

  it("clamps the number of kept copies", () => {
    expect(clampKeep(0)).toBe(1);
    expect(clampKeep(10.4)).toBe(10);
    expect(clampKeep(5000)).toBe(1000);
    expect(clampKeep(Number.NaN)).toBe(10);
  });

  it("formats sizes and times", () => {
    expect(formatSize(500)).toBe("500 Б");
    expect(formatSize(2048)).toBe("2 КБ");
    expect(formatSize(3 * 1024 * 1024)).toBe("3.0 МБ");
    expect(formatTime(null)).toBe("—");
    expect(formatTime(1_790_000_000)).not.toBe("—");
  });
});
