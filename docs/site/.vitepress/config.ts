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
          items: [
            { text: 'Overview', link: '/reference/' },
            {
              text: 'TypeScript SDK',
              link: '/reference/generated/typescript-sdk',
              items: [
                { text: 'Design', link: '/reference/generated/typescript-sdk/design' },
                { text: 'Core types', link: '/reference/generated/typescript-sdk/types' },
                { text: 'Generator exports', link: '/reference/generated/typescript-sdk/generators' },
              ],
            },
            {
              text: 'Python SDK',
              link: '/reference/generated/python-sdk',
              items: [
                { text: 'Design', link: '/reference/generated/python-sdk/design' },
                { text: 'Module API', link: '/reference/generated/python-sdk/module' },
              ],
            },
            {
              text: 'CLI',
              link: '/reference/generated/cli',
              items: [
                { text: 'emit', link: '/reference/generated/cli/emit' },
                { text: 'verify', link: '/reference/generated/cli/verify' },
                { text: 'simulate', link: '/reference/generated/cli/simulate' },
                { text: 'review-gcode', link: '/reference/generated/cli/review-gcode' },
                { text: 'rewrite-gcode', link: '/reference/generated/cli/rewrite-gcode' },
                { text: 'trace-gcode', link: '/reference/generated/cli/trace-gcode' },
                { text: 'explain', link: '/reference/generated/cli/explain' },
                { text: 'compare', link: '/reference/generated/cli/compare' },
              ],
            },
            {
              text: 'IR',
              link: '/reference/generated/ir',
              items: [
                { text: 'Data model', link: '/reference/generated/ir/data-model' },
                { text: 'JSON wire form', link: '/reference/generated/ir/json-wire-form' },
              ],
            },
            { text: 'Generators', link: '/reference/generated/generators' },
            { text: 'Verification', link: '/reference/generated/verification' },
            {
              text: 'Profiles and reports',
              link: '/reference/generated/profiles-and-reports',
              items: [
                { text: 'Profile schema', link: '/reference/generated/profiles-and-reports/profile-schema' },
                { text: 'Rule catalog', link: '/reference/generated/profiles-and-reports/verification-rules' },
                { text: 'Report outputs', link: '/reference/generated/profiles-and-reports/report-outputs' },
                { text: 'Profile matrix', link: '/reference/generated/profiles-and-reports/supported-profile-matrix' },
              ],
            },
            { text: 'Examples', link: '/reference/generated/examples' },
          ],
        },
      ],
    },
  },
  vite: {
    resolve: { alias: { '@sdk': sdkSrc, '@webspline': webSpline } },
    server: { fs: { allow: [sdkSrc, path.dirname(webSpline)] } },
  },
});
