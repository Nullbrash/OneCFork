// node scripts/set-version.mjs 1.2.0   — версия сразу в package.json,
// src-tauri/Cargo.toml и src-tauri/tauri.conf.json.
import { readFileSync, writeFileSync } from "node:fs";
import { readVersions, setVersions } from "./release-lib.mjs";

const version = process.argv[2];
const paths = {
  packageJson: "package.json",
  cargoToml: "src-tauri/Cargo.toml",
  tauriConf: "src-tauri/tauri.conf.json",
};
const files = Object.fromEntries(Object.entries(paths).map(([k, p]) => [k, readFileSync(p, "utf8")]));
const next = setVersions(files, version);
for (const [k, p] of Object.entries(paths)) writeFileSync(p, next[k]);
console.log("versions:", readVersions(next));
