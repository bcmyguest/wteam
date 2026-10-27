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
        { label: 'Overview', slug: 'index' },
        { label: 'Quickstart', slug: 'quickstart' },
        { label: 'Concepts', slug: 'concepts' },
        {
          label: 'Providers',
          items: [
            { label: 'Overview', slug: 'providers' },
            { label: 'Claude Code', slug: 'providers/claude' },
            { label: 'Codex', slug: 'providers/codex' },
            { label: 'OpenCode', slug: 'providers/opencode' },
            { label: 'Pi', slug: 'providers/pi' },
            { label: 'Cursor', slug: 'providers/cursor' },
          ],
        },
        {
          label: 'Architecture',
          items: [
            { label: 'Overview', slug: 'architecture' },
            {
              label: 'Mail routing',
              items: [
                { label: 'Overview', slug: 'architecture/routing' },
                { label: 'pair', slug: 'architecture/routing/pair' },
                { label: 'broadcast', slug: 'architecture/routing/broadcast' },
                { label: 'fan-in', slug: 'architecture/routing/fan-in' },
              ],
            },
          ],
        },
        { label: 'Permissions', slug: 'permissions' },
        { label: 'Benchmark', slug: 'benchmark' },
        { label: 'Reference', slug: 'reference' },
      ],
      customCss: ['./src/custom.css'],
      lastUpdated: false,
    }),
  ],
});
