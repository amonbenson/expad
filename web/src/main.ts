import "./main.css";

import PrimeVue from "primevue/config";
import { createApp } from "vue";

import App from "./App.vue";
import { EXPAD_PRESET } from "./theme";

createApp(App)
  .use(PrimeVue, {
    license: import.meta.env.VITE_PRIMEUI_LICENSE,
    theme: {
      preset: EXPAD_PRESET,
      options: {
        cssLayer: { name: "primevue", order: "theme, base, primevue" },
      },
    },
  })
  .mount("#app");
