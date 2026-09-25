// PrimeVue theme of the interface, built on the flat Nora preset (2px radii, no shadows, no
// transitions) with a blue-gray surface ramp. The per-jack accents come from the device's
// settings; main.css re-points the design tokens of the controls inside a strip at its `--jack-color`.

import { definePreset, mix, palette, shade, tint } from "@primeuix/themes";
import Nora from "@primeuix/themes/nora";

/** Accent of everything not belonging to one jack, the second jack's default color. */
const PRIMARY_COLOR = "#FFA259";

/**
 * Signal colors, also available as `--p-indicator-<name>`: red for a high rail, blue for a low
 * one, green for anything live. The firmware's LEDs show the same green (`ACTIVE_LED_COLOR`).
 */
export const INDICATOR_COLORS = {
  red: "#FB2C36",
  green: "#00C950",
  blue: "#2B7FFF",
};

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
    primary: palette(PRIMARY_COLOR),
    surface: SURFACE,
    // Remove borders from form fields
    formField: {
      borderColor: "transparent",
      hoverBorderColor: "transparent",
    },
  },
  // Custom tokens, emitted as `--p-indicator-red` and so on
  extend: {
    indicator: INDICATOR_COLORS,
  },
  components: {
    toggleswitch: {
      handle: {
        // Re-apply the surface color here, otherwise it would inherit the transparent form field border color
        background: "{surface.500}",
        hoverBackground: "{surface.400}",
      },
    },
  },
});
