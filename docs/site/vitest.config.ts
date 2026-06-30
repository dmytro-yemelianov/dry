import { defineConfig } from 'vitest/config';
import vue from '@vitejs/plugin-vue';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, '../..');

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@sdk': path.resolve(repoRoot, 'sdk/ts/src'),
      '@webspline': path.resolve(repoRoot, 'web/spline.js'),
    },
  },
  test: { environment: 'jsdom', include: ['.vitepress/theme/**/*.test.ts', 'smoke/**/*.unit.test.ts'] },
});
