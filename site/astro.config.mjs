import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// SITE_URL and BASE_PATH let one build target either host. Defaults match the
// GitHub Pages project site at https://bcmyguest.github.io/wteam/.
const site = process.env.SITE_URL || 'https://bcmyguest.github.io';
const base = process.env.BASE_PATH || '/wteam/';
const root = base.replace(/\/$/, '');

export default defineConfig({
  site,
  base,
  integrations: [
    starlight({
      title: 'wteam',
      description: 'Portable worker/reviewer teams for coding agents',
      social: [{ icon: 'github', label: 'Source repository', href: 'https://github.com/bcmyguest/wteam' }],
      head: [
        // Renders ```mermaid blocks in the browser; see public/mermaid-init.mjs.
        { tag: 'script', attrs: { type: 'module', src: `${root}/mermaid-init.mjs` } },
      ],
      sidebar: [
        { label: 'Start', items: [{ label: 'Landing', slug: 'index' }, { label: 'Quickstart', slug: 'quickstart' }] },
        {
          label: 'Guide',
          items: [
            { label: 'Concepts', slug: 'concepts' },
            { label: 'Architecture', slug: 'architecture' },
            { label: 'Permissions', slug: 'permissions' },
            { label: 'Benchmark', slug: 'benchmark' },
          ],
        },
        { label: 'Reference', items: [{ label: 'CLI and files', slug: 'reference' }] },
      ],
      lastUpdated: false,
    }),
  ],
});
