/** Варианты периода копий, минуты. Значение из базы, которого нет в списке,
 * тоже показывается (его могли задать раньше). */
export const INTERVAL_OPTIONS: readonly number[] = [15, 30, 60, 120, 240, 480, 1440];

export const DEFAULT_KEEP = 10;
export const MAX_KEEP = 1000;

export function clampKeep(n: number): number {
  if (!Number.isFinite(n)) return DEFAULT_KEEP;
  return Math.min(MAX_KEEP, Math.max(1, Math.round(n)));
}

export type PasswordProblem = "empty" | "mismatch" | null;

export function passwordProblem(next: string, repeat: string): PasswordProblem {
  if (!next) return "empty";
  if (next !== repeat) return "mismatch";
  return null;
}

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} Б`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} КБ`;
  return `${(bytes / 1024 / 1024).toFixed(1)} МБ`;
}

/** Unix-секунды → местное время для показа. */
export function formatTime(secs: number | null): string {
  if (!secs) return "—";
  return new Date(secs * 1000).toLocaleString("ru-RU");
}
