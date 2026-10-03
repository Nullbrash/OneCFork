import { invoke } from "@tauri-apps/api/core";

export type DbMode = "real" | "test";

export interface AppInfo {
  version: string;
  mode: DbMode;
  dbPath: string;
  schemaVersion: number;
}

export function fetchAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

// Плашка «ТЕСТ» видна всегда, когда открыта тестовая база, — чтобы проверки
// через интерфейс нельзя было спутать с работой на рабочих данных.
export function showTestPlaque(info: Pick<AppInfo, "mode"> | null): boolean {
  return info?.mode === "test";
}
