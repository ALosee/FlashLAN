import { defineConfig } from 'unocss';
import { presetSbean } from '@soybeanjs/ui-uno';

export default defineConfig({
  presets: [
    presetSbean({
      overrides: {
        resetCSS: true,
        globalCSS: true,
        uiCSS: true
      }
    })
  ]
});
