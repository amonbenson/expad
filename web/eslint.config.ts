import { defineConfigWithVueTs, vueTsConfigs } from "@vue/eslint-config-typescript";
import { globalIgnores } from "eslint/config";
import pluginVue from "eslint-plugin-vue";

export default defineConfigWithVueTs(
  { name: "app/files-to-lint", files: ["**/*.{vue,ts,mts}"] },
  globalIgnores(["**/dist/**"]),
  pluginVue.configs["flat/essential"],
  vueTsConfigs.recommended,
);
