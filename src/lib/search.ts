import type { ListRow } from "../api";

export interface SearchFilters {
  roleIds: number[];
  specializationIds: number[];
  addressLabelIds: number[];
}

export const NO_FILTERS: SearchFilters = { roleIds: [], specializationIds: [], addressLabelIds: [] };

/** Строчные буквы, «ё» как «е», одиночные пробелы. */
export function normalize(s: string): string {
  return s.toLowerCase().replace(/ё/g, "е").replace(/\s+/g, " ").trim();
}

// Вторая половина — те же клавиши с Shift: заглавная «Б» в английской
// раскладке — это «<», а не «,» (запрос уже в нижнем регистре, но символы
// с Shift регистром не понижаются). Обратно (RU → EN) берётся первое
// вхождение — символ без Shift.
const EN = "`qwertyuiop[]asdfghjkl;'zxcvbnm,." + '~{}:"<>';
const RU = "ёйцукенгшщзхъфывапролджэячсмитьбю" + "ёхъжэбю";

function convert(s: string, from: string, to: string): string {
  return [...s].map((ch) => (from.includes(ch) ? to[from.indexOf(ch)] : ch)).join("");
}

/** Запрос как есть + в другой раскладке: «injhs» → «шторы», «ыфьыгтп» → «samsung». */
export function layoutVariants(token: string): string[] {
  const variants = [token, normalize(convert(token, EN, RU)), convert(token, RU, EN)];
  return [...new Set(variants)].filter(Boolean);
}

/** Цифры запроса как ключ телефона — так же, как `phone_key` в Rust. */
export function phoneDigits(token: string): string | null {
  const digits = token.replace(/\D/g, "");
  // Номер — если цифр хотя бы 3 и в запросе почти нет букв.
  if (digits.length < 3 || /[a-zа-я]/i.test(token)) return null;
  return digits.length === 11 && /^[78]/.test(digits) ? digits.slice(1) : digits;
}

/** Расстояние Левенштейна с ранним выходом, если уже больше `max`. */
export function editDistance(a: string, b: string, max: number): number {
  if (Math.abs(a.length - b.length) > max) return max + 1;
  let prev = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i++) {
    const cur = [i];
    let rowMin = i;
    for (let j = 1; j <= b.length; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      cur[j] = Math.min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + cost);
      rowMin = Math.min(rowMin, cur[j]);
    }
    if (rowMin > max) return max + 1;
    prev = cur;
  }
  return prev[b.length];
}

/** Сколько опечаток прощать: короткие слова — точно, длинные — до двух. */
function allowedTypos(length: number): number {
  if (length < 4) return 0;
  return length < 8 ? 1 : 2;
}

interface Indexed {
  row: ListRow;
  text: string;
  words: string[];
}

export function buildIndex(rows: readonly ListRow[]): Indexed[] {
  return rows.map((row) => {
    const text = normalize(
      [row.title, ...row.roles, ...row.specializations, ...row.companies, row.searchText].join(" \n "),
    );
    const words = [...new Set(text.split(/[^\p{L}\p{N}@_]+/u).filter((w) => w.length >= 3))];
    return { row, text, words };
  });
}

function tokenMatches(entry: Indexed, token: string): boolean {
  const digits = phoneDigits(token);
  if (digits && entry.row.phoneKeys.some((k) => k.includes(digits))) return true;
  for (const v of layoutVariants(token)) {
    if (entry.text.includes(v)) return true;
    const typos = allowedTypos(v.length);
    if (typos === 0) continue;
    // Опечатка в недописанном слове: сравниваем с целым словом и с его
    // началами длиной от длины запроса до +typos — пропущенная буква делает
    // нужное начало длиннее запроса («монтжн» ↔ «монтажн»иков).
    for (const w of entry.words) {
      if (editDistance(v, w, typos) <= typos) return true;
      for (let len = v.length; len <= v.length + typos && len < w.length; len++) {
        if (editDistance(v, w.slice(0, len), typos) <= typos) return true;
      }
    }
  }
  return false;
}

/** Каждое слово запроса должно найтись в карточке (порядок слов — любой). */
function textMatches(entry: Indexed, tokens: string[]): boolean {
  return tokens.every((t) => tokenMatches(entry, t));
}

/** Внутри фильтра — «любое из», между фильтрами — «и» (решение пользователя). */
function filtersMatch(row: ListRow, f: SearchFilters): boolean {
  const any = (chosen: number[], have: number[]) =>
    chosen.length === 0 || chosen.some((id) => have.includes(id));
  return (
    any(f.roleIds, row.roleIds) &&
    any(f.specializationIds, row.specializationIds) &&
    any(f.addressLabelIds, row.addressLabelIds)
  );
}

export function isActive(query: string, f: SearchFilters): boolean {
  return (
    normalize(query) !== "" ||
    f.roleIds.length > 0 ||
    f.specializationIds.length > 0 ||
    f.addressLabelIds.length > 0
  );
}

export function searchRows(index: readonly Indexed[], query: string, filters: SearchFilters): ListRow[] {
  const q = normalize(query);
  // Номер вводят с пробелами («8 900 000-11-22») — без букв весь запрос один номер.
  const tokens = phoneDigits(q) ? [q.replace(/\s/g, "")] : q.split(" ").filter(Boolean);
  return index.filter((e) => filtersMatch(e.row, filters) && textMatches(e, tokens)).map((e) => e.row);
}
