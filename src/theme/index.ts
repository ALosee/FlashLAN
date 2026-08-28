import type { ThemeOptions } from '@soybeanjs/theme'

/**
 * Application-level theme tokens.
 *
 * Components should consume `border-border` and `border-input` rather than
 * defining their own light/dark border opacities. The theme engine extracts
 * the alpha from these values and applies it consistently in both modes.
 */
export const flashlanTheme = {
  base: 'zinc',
  primary: 'indigo',
  format: 'hsl',
  overrides: {
    light: {
      border: 'zinc.200',
      input: 'zinc.200',
    },
    dark: {
      border: 'oklch(100% 0 0 / 0.14)',
      input: 'oklch(100% 0 0 / 0.18)',
    },
  },
} satisfies ThemeOptions
