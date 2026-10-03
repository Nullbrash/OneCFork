export type ThemeChoice = "system" | "light" | "dark";

export function parseTheme(value: string | null): ThemeChoice {
  return value === "light" || value === "dark" ? value : "system";
}

export function resolveTheme(choice: ThemeChoice, systemPrefersDark: boolean): "light" | "dark" {
  if (choice === "system") return systemPrefersDark ? "dark" : "light";
  return choice;
}
