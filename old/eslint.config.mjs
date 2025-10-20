import js from "@eslint/js";
import strict from "eslint-config-strict"
import globals from "globals";
import tseslint from "typescript-eslint";
import { defineConfig } from "eslint/config";

export default defineConfig([
  {
    files: [["web/src/*", "**/*.{js,mjs,cjs,ts,mts,cts}"]],
    plugins: { js },
    extends: ["js/all", tseslint.configs.recommended],
    languageOptions: { globals: globals.browser },
    rules: {
      "func-style": ["error", "declaration"],
      "max-statements": ["warn", 16],
      "no-magic-numbers": ["warn", { "ignore": [0, -1, 1, 1000] }],
      "no-console": ["warn", { allow: ["warn", "error"] }],
      "@typescript-eslint/no-unused-vars": [
        "error",
        {
          "args": "all",
          "argsIgnorePattern": "^_",
          "caughtErrors": "all",
          "caughtErrorsIgnorePattern": "^_",
          "destructuredArrayIgnorePattern": "^_",
          "varsIgnorePattern": "^_",
          "ignoreRestSiblings": true
        }
      ],
      "max-lines": ["error", 500],
      "max-params": ["warn", 10],
      "max-lines-per-function": ["warn", 75],
      "no-warning-comments": "warn",
      "@typescript-eslint/no-explicit-any": "off",
      "no-unused-vars": "off",
      "max-classes-per-file": "off",
      "no-inline-comments": "off",
      "one-var": "off",
      "prefer-destructuring": "off",
      "sort-keys": "off",
      "init-declarations": "off"
      // "max-statements": "off"
    }
  }
]);
