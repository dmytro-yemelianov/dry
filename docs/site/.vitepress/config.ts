import { defineConfig } from 'vitepress';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const here = path.dirname(fileURLToPath(import.meta.url)); // docs/site/.vitepress
const repoRoot = path.resolve(here, '../../..'); // -> repo root
const sdkSrc = path.resolve(repoRoot, 'sdk/ts/src');
const webSpline = path.resolve(repoRoot, 'web/spline.js');

export default defineConfig({
  title: 'Dry',
  description: 'Interactive docs for the Dry toolpath compiler — editable code, live execution.',
  themeConfig: {
    nav: [
      { text: 'Guide', link: '/guide/' },
      { text: 'Gallery', link: 'https://github.com/dmytro-yemelianov/dry' },
    ],
    sidebar: {
      '/guide/': [
        {
          text: 'Guide',
          items: [
            { text: 'Overview', link: '/guide/' },
            { text: '1. Author a path', link: '/guide/author' },
            { text: '2. Lower to the Dry IR', link: '/guide/lower' },
            { text: '3. Simulate', link: '/guide/simulate' },
            { text: '4. Verify', link: '/guide/verify' },
            { text: '5. Optimize', link: '/guide/optimize' },
            { text: '6. Generative', link: '/guide/generative' },
          ],
        },
      ],
    },
  },
  vite: {
    resolve: { alias: { '@sdk': sdkSrc, '@webspline': webSpline } },
    server: { fs: { allow: [repoRoot] } },
  },
});
