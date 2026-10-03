import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  buildLatestJson,
  downloadUrl,
  installerName,
  isPrerelease,
  isValidVersion,
  readVersions,
  setVersions,
} from "./release-lib.mjs";

const real = {
  packageJson: readFileSync("package.json", "utf8"),
  cargoToml: readFileSync("src-tauri/Cargo.toml", "utf8"),
  tauriConf: readFileSync("src-tauri/tauri.conf.json", "utf8"),
};

describe("versions", () => {
  it("the three version fields in the repository agree", () => {
    const v = readVersions(real);
    expect(v.cargo).toBe(v.package);
    expect(v.tauri).toBe(v.package);
    expect(isValidVersion(v.package)).toBe(true);
  });

  it("set-version changes only [package] version, not dependency versions", () => {
    const next = setVersions(real, "9.8.7-beta.1");
    expect(readVersions(next)).toEqual({
      package: "9.8.7-beta.1",
      cargo: "9.8.7-beta.1",
      tauri: "9.8.7-beta.1",
    });
    const deps = (s) => s.split("[dependencies]")[1];
    expect(deps(next.cargoToml)).toBe(deps(real.cargoToml));
    expect(() => setVersions(real, "1.0")).toThrow();
  });

  it("suffix means prerelease", () => {
    expect(isPrerelease("1.0.0")).toBe(false);
    expect(isPrerelease("1.1.0-beta.2")).toBe(true);
  });
});

describe("latest.json", () => {
  it("points at the installer of the tagged release", () => {
    const j = buildLatestJson({
      version: "1.0.0",
      notes: "n",
      pubDate: "2026-10-03T00:00:00Z",
      signature: " sig\n",
    });
    expect(j.platforms["windows-x86_64"]).toEqual({
      signature: "sig",
      url: "https://github.com/Nullbrash/OneCFork/releases/download/v1.0.0/OneCFork_1.0.0_x64-setup.exe",
    });
    expect(installerName("1.0.0")).toBe("OneCFork_1.0.0_x64-setup.exe");
    expect(downloadUrl("1.0.0")).toContain("/v1.0.0/");
  });

  it("refuses an empty signature or a bad version", () => {
    expect(() => buildLatestJson({ version: "1.0.0", pubDate: "x", signature: "" })).toThrow();
    expect(() => buildLatestJson({ version: "v1", pubDate: "x", signature: "s" })).toThrow();
  });
});
