import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vitest/config';
export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  server: { port: 5173, strictPort: true, proxy: {
    '/api': { target: 'http://127.0.0.1:8080' },
    '/cal': { target: 'http://127.0.0.1:8080' },
    '/health': { target: 'http://127.0.0.1:8080' }
  } },
  test: { include: ['src/**/*.test.ts'], environment: 'node' }
});
