import { defineConfig } from 'vitepress';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const here = path.dirname(fileURLToPath(import.meta.url)); // docs/site/.vitepress
const repoRoot = path.resolve(here, '../../..'); // -> repo root
const sdkSrc = path.resolve(repoRoot, 'sdk/ts/src');
const webSpline = path.resolve(repoRoot, 'web/spline.js');
const referencePagesFile = path.resolve(repoRoot, 'docs/site/reference/source/pages.json');
const publicDocumentationBuild = process.env.DRY_DOCS_MODE === 'public';
const publicDocumentationAssets = path.resolve(here, '../.public-docs');
const liveExampleComponent = path.resolve(
  here,
  'theme',
  publicDocumentationBuild ? 'PublicExample.vue' : 'LiveExample.vue',
);

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
    // Fall back to an explicit minimal sidebar if metadata is missing.
  }
  return [{ text: 'Overview', link: '/reference/' }];
}

export default defineConfig({
  title: 'Dry',
  description: publicDocumentationBuild
    ? 'Documentation for the Dry proprietary toolpath compiler.'
    : 'Interactive product documentation for the Dry toolpath compiler.',
  // The allow-listed static gallery is staged after VitePress emits the docs. The post-build link
  // checker validates it after staging, so only this synthetic pre-stage target is ignored here.
  ignoreDeadLinks: ['/gallery/index'],
  themeConfig: {
    outline: {
      level: [2, 3],
      label: 'On this page',
    },
    nav: [
      { text: 'Guide', link: '/guide/' },
      { text: 'Reference', link: '/reference/' },
      { text: 'Market', link: '/marketing/' },
      { text: 'Licensing', link: '/licensing' },
      ...(!publicDocumentationBuild ? [{ text: 'Gallery', link: '/gallery/' }] : []),
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
      '/marketing/': [
        {
          text: 'Market Research',
          items: [
            { text: 'Overview', link: '/marketing/' },
            { text: 'ICP', link: '/marketing/#ideal-customer-profile' },
            { text: 'Competition', link: '/marketing/#competitive-landscape' },
            { text: 'Packages', link: '/marketing/#package-strategy' },
            { text: 'Pilot Design', link: '/marketing/#pilot-design' },
            {
              text: 'Product Architecture',
              items: [
                { text: 'G-code Machine SaaS', link: '/marketing/gcode-machine-saas' },
                { text: 'Printer Library', link: '/marketing/printer-capability-library' },
                { text: 'Slicer Attack Map', link: '/marketing/slicer-attack-map' },
                { text: 'CAD Embedding', link: '/marketing/cad-embedding' },
              ],
            },
          ],
        },
      ],
    },
  },
  vite: {
    resolve: {
      alias: {
        '@dry-live-example': liveExampleComponent,
        '@sdk': sdkSrc,
        '@webspline': webSpline,
      },
    },
    ...(publicDocumentationBuild ? { publicDir: publicDocumentationAssets } : {}),
    server: { fs: { allow: [sdkSrc, path.dirname(webSpline)] } },
  },
});
