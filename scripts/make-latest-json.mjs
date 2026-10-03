// node scripts/make-latest-json.mjs "что нового"  — после сборки релиза:
// берёт версию и подпись установщика, пишет latest.json рядом с ним.
import { readFileSync, writeFileSync } from "node:fs";
import { buildLatestJson, installerName } from "./release-lib.mjs";

const version = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8")).version;
const dir = "src-tauri/target/release/bundle/nsis";
const signature = readFileSync(`${dir}/${installerName(version)}.sig`, "utf8");
const latest = buildLatestJson({
  version,
  notes: process.argv[2] ?? "",
  pubDate: new Date().toISOString(),
  signature,
});
writeFileSync(`${dir}/latest.json`, JSON.stringify(latest, null, 2) + "\n");
console.log(`written ${dir}/latest.json for ${version}`);
