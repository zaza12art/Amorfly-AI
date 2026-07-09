import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import path from 'path';
import {defineConfig} from 'vite';

// Tauri, geliştirme sunucusunun sabit bir portta olmasını bekler.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(() => ({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, '.'),
    },
  },
  // Tauri komutları için gerekli sabit ayarlar:
  clearScreen: false,
  server: {
    host: host || false,
    port: 1420,
    strictPort: true,
    watch: {
      // src-tauri klasöründeki değişiklikleri izlemeye gerek yok (Rust kendi derlenir)
      ignored: ['**/src-tauri/**'],
    },
  },
}));
