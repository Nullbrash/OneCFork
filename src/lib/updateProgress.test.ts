import { describe, expect, it } from "vitest";
import { addSample, formatMb, formatSpeed, percent, speed } from "./updateProgress";

describe("update progress", () => {
  it("speed uses only the last 3 seconds", () => {
    let s = addSample([], { at: 0, bytes: 0 });
    s = addSample(s, { at: 1000, bytes: 100_000 });
    expect(speed(s)).toBe(100_000);
    s = addSample(s, { at: 5000, bytes: 1_100_000 });
    // Оба старых отсчёта (0 и 1000 мс) старше 3 с — выпали из окна, скорости пока нет.
    expect(s.map((x) => x.at)).toEqual([5000]);
    expect(speed(s)).toBeNull();
    s = addSample(s, { at: 6000, bytes: 1_600_000 });
    expect(speed(s)).toBe(500_000);
  });

  it("percent is capped and needs a known total", () => {
    expect(percent(50, 200)).toBe(25);
    expect(percent(250, 200)).toBe(100);
    expect(percent(50, null)).toBeNull();
  });

  it("formats speed and size", () => {
    expect(formatSpeed(null)).toBe("…");
    expect(formatSpeed(512)).toBe("512 Б/с");
    expect(formatSpeed(2048)).toBe("2 КБ/с");
    expect(formatSpeed(3 * 1024 * 1024)).toBe("3.0 МБ/с");
    expect(formatMb(5 * 1024 * 1024)).toBe("5.0");
  });
});
