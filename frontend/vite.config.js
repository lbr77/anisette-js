import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue()],
  optimizeDeps: {
    exclude: ['anisette-js']
  },
  build: {
    target: 'esnext'
  },
  server: {
    proxy: {
      "/api": "http://localhost:8080",
      "/wisp": { target: "ws://localhost:8080", ws: true },
    },
  },
})
