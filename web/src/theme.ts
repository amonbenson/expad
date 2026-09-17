// PrimeVue theme of the interface, built on the flat Nora preset (2px radii, no shadows, no
// transitions) with a blue-gray surface ramp. The per-jack accents live in main.css, which
// re-points the design tokens of the controls inside a strip at its `--jack-color`.

import { definePreset, mix, palette, shade, tint } from "@primeuix/themes";
import Nora from "@primeuix/themes/nora";

/** Accent color of each jack, from the lightest to the deepest. */
export const JACK_COLORS = ["#FF7E7E", "#FFA259", "#FFCB56", "#FFEDB9"];

/** The two ends of the blue-gray ramp: shade 100 and shade 900. */
const SURFACE_LIGHT = "#E8EDF2";
const SURFACE_DARK = "#363e47";

/** Every shade in between is mixed from the two ends, the outer two lightened and darkened. */
const SURFACE = {
  0: "#ffffff",
  50: tint(SURFACE_LIGHT, 50),
  100: SURFACE_LIGHT,
  200: mix(SURFACE_DARK, SURFACE_LIGHT, 12.5),
  300: mix(SURFACE_DARK, SURFACE_LIGHT, 25),
  400: mix(SURFACE_DARK, SURFACE_LIGHT, 37.5),
  500: mix(SURFACE_DARK, SURFACE_LIGHT, 50),
  600: mix(SURFACE_DARK, SURFACE_LIGHT, 62.5),
  700: mix(SURFACE_DARK, SURFACE_LIGHT, 75),
  800: mix(SURFACE_DARK, SURFACE_LIGHT, 87.5),
  900: SURFACE_DARK,
  950: shade(SURFACE_DARK, 25),
};

export const EXPAD_PRESET = definePreset(Nora, {
  semantic: {
    primary: palette(JACK_COLORS[2]),
    surface: SURFACE,
  },
});
