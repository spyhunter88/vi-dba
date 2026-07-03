import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1424,
    strictPort: true,
    hmr: {
      port: 1432
    },
    // Proxy
    proxy: {
      '/_tauri': {
        target: 'http://localhost:1424',
        changeOrigin: true,
        secure: false
      }
    }
  }
})
