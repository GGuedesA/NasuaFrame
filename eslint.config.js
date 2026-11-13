import js from "@eslint/js";
import { defineConfig } from "eslint/config";
import prettier from "eslint-config-prettier";
import svelte from "eslint-plugin-svelte";
import globals from "globals";
import ts from "typescript-eslint";
import { loadConfig } from "@sveltejs/load-config";

const svelteConfig = (await loadConfig("./", { traverse: false }))?.config;

export default defineConfig(
  { ignores: ["build/", ".svelte-kit/", "target/", "src-tauri/target/", "src-tauri/gen/"] },
  js.configs.recommended,
  ts.configs.recommended,
  svelte.configs.recommended,
  prettier,
  svelte.configs.prettier,
  { languageOptions: { globals: { ...globals.browser, ...globals.node } } },
  {
    files: ["**/*.svelte", "**/*.svelte.ts", "**/*.svelte.js"],
    languageOptions: {
      parserOptions: {
        projectService: true,
        extraFileExtensions: [".svelte"],
        parser: ts.parser,
        svelteConfig,
      },
    },
  },
);
