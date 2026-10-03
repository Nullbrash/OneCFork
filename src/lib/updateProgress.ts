export interface Sample {
  /** Время, мс. */
  at: number;
  /** Скачано байт к этому моменту. */
  bytes: number;
}

/** Скорость считается по последним секундам, а не от начала: так она
 * отражает текущую скорость сети, а не среднюю за всю загрузку. */
export const SPEED_WINDOW_MS = 3000;

export function addSample(samples: readonly Sample[], next: Sample): Sample[] {
  return [...samples, next].filter((s) => next.at - s.at <= SPEED_WINDOW_MS);
}

/** Байт в секунду по окну; null — пока данных мало. */
export function speed(samples: readonly Sample[]): number | null {
  if (samples.length < 2) return null;
  const first = samples[0];
  const last = samples[samples.length - 1];
  const dt = last.at - first.at;
  if (dt <= 0) return null;
  return ((last.bytes - first.bytes) * 1000) / dt;
}

export function percent(downloaded: number, total: number | null): number | null {
  if (!total || total <= 0) return null;
  return Math.min(100, Math.floor((downloaded * 100) / total));
}

export function formatSpeed(bytesPerSec: number | null): string {
  if (bytesPerSec === null) return "…";
  if (bytesPerSec < 1024) return `${Math.round(bytesPerSec)} Б/с`;
  if (bytesPerSec < 1024 * 1024) return `${Math.round(bytesPerSec / 1024)} КБ/с`;
  return `${(bytesPerSec / 1024 / 1024).toFixed(1)} МБ/с`;
}

export function formatMb(bytes: number): string {
  return (bytes / 1024 / 1024).toFixed(1);
}
