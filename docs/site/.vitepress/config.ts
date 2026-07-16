import { defineConfig } from 'vitepress';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const here = path.dirname(fileURLToPath(import.meta.url)); // docs/site/.vitepress
const repoRoot = path.resolve(here, '../../..'); // -> repo root
const sdkSrc = path.resolve(repoRoot, 'sdk/ts/src');
const webSpline = path.resolve(repoRoot, 'web/spline.js');
const referencePagesFile = path.resolve(repoRoot, 'docs/site/reference/source/pages.json');

type SidebarItem = {
  text: string;
  link?: string;
  items?: SidebarItem[];
};

function readReferenceSidebar(): SidebarItem[] {
  try {
    const raw = fs.readFileSync(referencePagesFile, 'utf8');
    const parsed = JSON.parse(raw) as { reference: unknown };
    const items = parsed.reference;
    if (
      Array.isArray(items) &&
      items.every(
        (item) =>
          typeof item === 'object' &&
          item !== null &&
          typeof (item as { text?: unknown }).text === 'string',
      )
    ) {
      return items as SidebarItem[];
    }
  } catch {
    // Fall back to a minimal reference sidebar if metadata is unavailable.
  }
  return [{ text: 'Overview', link: '/reference/' }];
}

export default defineConfig({
  title: 'Dry',
  description: 'Interactive docs for the Dry toolpath compiler — editable code, live execution.',
  themeConfig: {
    outline: {
      level: [2, 3],
      label: 'On this page',
    },
    nav: [
      { text: 'Guide', link: '/guide/' },
      { text: 'Reference', link: '/reference/' },
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
      '/reference/': [
        {
          text: 'Reference',
          items: readReferenceSidebar(),
        },
      ],
    },
  },
  vite: {
    resolve: { alias: { '@sdk': sdkSrc, '@webspline': webSpline } },
    server: { fs: { allow: [sdkSrc, path.dirname(webSpline)] } },
  },
});
