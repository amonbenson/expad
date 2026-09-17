import stylistic from "@stylistic/eslint-plugin";
import { defineConfigWithVueTs, vueTsConfigs } from "@vue/eslint-config-typescript";
import { globalIgnores } from "eslint/config";
import betterTailwindcss from "eslint-plugin-better-tailwindcss";
import simpleImportSort from "eslint-plugin-simple-import-sort";
import pluginVue from "eslint-plugin-vue";

export default defineConfigWithVueTs(
  {
    name: "app/files-to-lint",
    files: ["**/*.{ts,mts,tsx,vue}"],
  },
  globalIgnores(["**/dist/**", "**/dist-ssr/**", "**/coverage/**", "**/public/**"]),

  pluginVue.configs["flat/recommended"],
  vueTsConfigs.recommended,

  // disable the legacy core rules to avoid conflicts
  stylistic.configs["disable-legacy"],

  // setup basic stylistic configuration
  stylistic.configs.customize({
    indent: 2,
    quotes: "double",
    semi: true,
    jsx: false, // Vue project, not JSX
    commaDangle: "always-multiline",
  }),

  {
    name: "app/stylistic-overrides",
    rules: {
      // Enforce 1tbs brace placement. allowSingleLine is false by default,
      // which means { foo(); bar(); } on one line is an error.
      "@stylistic/brace-style": ["error", "1tbs"],

      // Require a blank line after any multiline block (if, for, while, etc.)
      // Only triggers on multiline blocks, not single-line object literals.
      "@stylistic/padding-line-between-statements": [
        "error",
        { blankLine: "always", prev: "multiline-block-like", next: "*" },
      ],

      // Ensure no trailing spaces and file ends with a newline
      "@stylistic/no-trailing-spaces": "error",
      "@stylistic/eol-last": ["error", "always"],
    },
  },

  {
    name: "app/import-sorting",
    plugins: {
      "simple-import-sort": simpleImportSort,
    },
    rules: {
      "simple-import-sort/imports": "error",
      "simple-import-sort/exports": "error",
    },
  },

  {
    name: "app/tailwind",
    plugins: {
      "better-tailwindcss": betterTailwindcss,
    },
    settings: {
      "better-tailwindcss": {
        // Path to your Tailwind v4 CSS entry file (the one with @import "tailwindcss")
        entryPoint: "src/main.css",
      },
    },
    rules: {
      "better-tailwindcss/enforce-consistent-class-order": "warn",
      "better-tailwindcss/no-unnecessary-whitespace": "warn",
      "better-tailwindcss/no-duplicate-classes": "warn",
    },
  },

  {
    name: "app/overrides",
    rules: {
      // TypeScript
      "@typescript-eslint/no-explicit-any": "off",
      "@typescript-eslint/no-unused-vars": ["error", {
        argsIgnorePattern: "^_",
        varsIgnorePattern: "^_",
        caughtErrorsIgnorePattern: "^_",
      }],
      "@typescript-eslint/explicit-function-return-type": ["error", {
        allowExpressions: true, // allow arrow functions like: const fn = () => 42
        allowTypedFunctionExpressions: true, // allow arrow functions like: const fn: MyType = () => 42
      }],

      // Always require curly braces for if/else/for/while/do - no braceless one-liners.
      // This is a logic/correctness rule (not stylistic), so it stays in core ESLint.
      "curly": ["error", "all"],

      // Vue
      "vue/block-order": ["error", { order: ["script", "template", "style"] }],
    },
  },
);
