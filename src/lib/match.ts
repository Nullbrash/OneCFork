/** Ключ сравнения — как `text_key` в Rust: строчные буквы, одиночные пробелы. */
export function textKey(s: string): string {
  return s.trim().split(/\s+/).join(" ").toLowerCase();
}

/**
 * Подсказки для ввода: сначала то, что начинается с набранного, потом то,
 * что содержит его; уже выбранное не предлагается.
 */
export function suggest<T>(
  items: readonly T[],
  query: string,
  name: (item: T) => string,
  exclude: (item: T) => boolean = () => false,
  limit = 8,
): T[] {
  const q = textKey(query);
  if (!q) return [];
  const pool = items.filter((i) => !exclude(i));
  const starts = pool.filter((i) => textKey(name(i)).startsWith(q));
  const contains = pool.filter((i) => !starts.includes(i) && textKey(name(i)).includes(q));
  return [...starts, ...contains].slice(0, limit);
}

/** Есть ли среди вариантов точное совпадение (тогда «Создать …» не нужен). */
export function hasExact<T>(items: readonly T[], query: string, name: (item: T) => string): boolean {
  const q = textKey(query);
  return items.some((i) => textKey(name(i)) === q);
}
