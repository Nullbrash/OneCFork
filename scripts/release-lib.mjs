// Общие функции выпуска: версия в трёх файлах и latest.json для автообновления.
// Секретов здесь нет — ключ подписи и разрешение GitHub сюда не попадают.

export const REPO = "Nullbrash/OneCFork";

const SEMVER = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;

export function isValidVersion(v) {
  return SEMVER.test(v);
}

/** Бета и проверочные сборки (с суффиксом: 1.2.0-beta.1) — только prerelease:
 * иначе автообновление раздаст их всем. */
export function isPrerelease(v) {
  return v.includes("-");
}

export function installerName(version) {
  return `OneCFork_${version}_x64-setup.exe`;
}

export function downloadUrl(version) {
  return `https://github.com/${REPO}/releases/download/v${version}/${installerName(version)}`;
}

/** Формат, который читает плагин обновлений Tauri. */
export function buildLatestJson({ version, notes, pubDate, signature }) {
  if (!isValidVersion(version)) throw new Error(`bad version: ${version}`);
  if (!signature || !signature.trim()) throw new Error("empty signature");
  return {
    version,
    notes: notes ?? "",
    pub_date: pubDate,
    platforms: {
      "windows-x86_64": { signature: signature.trim(), url: downloadUrl(version) },
    },
  };
}

export function readVersions({ packageJson, cargoToml, tauriConf }) {
  const cargo = /^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m.exec(cargoToml);
  return {
    package: JSON.parse(packageJson).version,
    cargo: cargo ? cargo[1] : null,
    tauri: JSON.parse(tauriConf).version,
  };
}

export function setVersions({ packageJson, cargoToml, tauriConf }, version) {
  if (!isValidVersion(version)) throw new Error(`bad version: ${version}`);
  const pkg = JSON.parse(packageJson);
  pkg.version = version;
  const conf = JSON.parse(tauriConf);
  conf.version = version;
  // Только версия в [package], не версии зависимостей.
  const cargo = cargoToml.replace(/^(\[package\][\s\S]*?^version\s*=\s*")[^"]+(")/m, `$1${version}$2`);
  return {
    packageJson: JSON.stringify(pkg, null, 2) + "\n",
    cargoToml: cargo,
    tauriConf: JSON.stringify(conf, null, 2) + "\n",
  };
}
