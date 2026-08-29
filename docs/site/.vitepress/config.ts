import { defineConfig } from 'vitepress';
import type { Plugin } from 'vite';
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
const publicSourceManifest = '.dry-public-source-manifest.json';
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

/** Normalize a Rollup module id into a stable, repository-relative provenance entry. */
function normalizeModuleId(moduleId: string): string {
  if (moduleId.startsWith('\0') || moduleId.startsWith('virtual:')) return moduleId;
  const clean = moduleId.split('?', 1)[0];
  if (clean.startsWith('/@') || clean.startsWith('/reference/previews/')) {
    return `public-url:${clean}`;
  }
  if (!path.isAbsolute(clean)) return clean.split(path.sep).join('/');
  return path.relative(repoRoot, clean).split(path.sep).join('/');
}

/** Emit the exact source-module set used by the public client bundle for the boundary audit. */
function publicSourceManifestPlugin(): Plugin {
  return {
    name: 'dry-public-source-manifest',
    apply: 'build',
    generateBundle(outputOptions, bundle) {
      if (!outputOptions.dir || path.resolve(outputOptions.dir) !== path.resolve(here, '../.vitepress/dist')) {
        return;
      }
      const modules = new Set<string>();
      for (const output of Object.values(bundle)) {
        if (output.type !== 'chunk') continue;
        for (const moduleId of Object.keys(output.modules)) modules.add(normalizeModuleId(moduleId));
      }
      this.emitFile({
        type: 'asset',
        fileName: publicSourceManifest,
        source: `${JSON.stringify([...modules].sort(), null, 2)}\n`,
      });
    },
  };
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
      { text: 'Book', link: '/book/' },
      { text: 'Guide', link: '/guide/' },
      { text: 'Cloud', link: '/cloud/' },
      { text: 'Reference', link: '/reference/' },
      { text: 'Market', link: '/marketing/' },
      { text: 'Licensing', link: '/licensing' },
      ...(!publicDocumentationBuild ? [{ text: 'Gallery', link: '/gallery/' }] : []),
    ],
    sidebar: {
      '/book/': [
        {
          text: 'The Dry Book',
          items: [
            { text: 'Overview & Table of Contents', link: '/book/' },
            { text: '1. System Specification', link: '/book/01_system_specification' },
            { text: '2. Algorithmic Authoring', link: '/book/02_algorithmic_authoring' },
            { text: '3. Multi-Axis CAM & Subtractive', link: '/book/03_multi_axis_and_subtractive' },
            { text: '4. Production & Cloud Operations', link: '/book/04_production_and_cloud_operations' },
            { text: '5. Executable Examples', link: '/book/05_executable_examples' },
          ],
        },
      ],
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
            { text: 'CI-gate quickstart', link: '/guide/ci-gate-quickstart' },
          ],
        },
      ],
      '/reference/': [
        {
          text: 'Reference',
          items: readReferenceSidebar(),
        },
      ],
      '/cloud/': [
        {
          text: 'Dry Cloud',
          items: [
            { text: 'Overview', link: '/cloud/' },
            { text: 'CLI quickstart', link: '/cloud/quickstart-cli' },
            { text: 'Integration quickstart', link: '/cloud/quickstart-integrations' },
            { text: 'API reference', link: '/cloud/api' },
          ],
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
    plugins: publicDocumentationBuild ? [publicSourceManifestPlugin()] : [],
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
