import js from "@eslint/js";
import tseslint from "typescript-eslint";
import react from "eslint-plugin-react";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";

export default tseslint.config(
  { ignores: ["dist", "src-tauri", "node_modules"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: { globals: globals.browser },
    settings: { react: { version: "detect" } },
    plugins: { react, "react-hooks": reactHooks },
    rules: {
      ...reactHooks.configs.recommended.rules,
      // Карточки содержат текст, введённый человеком и импортированный из Excel, —
      // показывать его только как текст; вставка HTML запрещена целиком.
      "react/no-danger": "error",
      "no-restricted-properties": [
        "error",
        { property: "innerHTML", message: "Данные карточек — только как текст (textContent)." },
        { property: "outerHTML", message: "Данные карточек — только как текст." },
      ],
    },
  },
);
