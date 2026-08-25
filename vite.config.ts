import { URL, fileURLToPath } from 'node:url'
import { defineConfig } from 'vite'
import Vue from '@vitejs/plugin-vue'
import UnoCSS from 'unocss/vite'
import Components from 'unplugin-vue-components/vite'
import SoybeanResolver from '@soybeanjs/ui/resolver'

const host = process.env.TAURI_DEV_HOST as string | undefined;

export default defineConfig(async () => ({
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
      '#ui': fileURLToPath(new URL('./src/ui', import.meta.url)),
    },
  },
  plugins: [
    Vue(),
    UnoCSS(),
    Components({
      dts: fileURLToPath(new URL('./src/typings/components.d.ts', import.meta.url)),
      resolvers: [SoybeanResolver()],
    })
  ],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}))
