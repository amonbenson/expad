import "./main.css";

import Aura from "@primeuix/themes/aura";
import PrimeVue from "primevue/config";
import { createApp } from "vue";

import App from "./App.vue";

createApp(App)
  .use(PrimeVue, {
    license: import.meta.env.VITE_PRIMEUI_LICENSE,
    theme: {
      preset: Aura,
      options: {
        cssLayer: { name: "primevue", order: "theme, base, primevue" },
      },
    },
  })
  .mount("#app");
