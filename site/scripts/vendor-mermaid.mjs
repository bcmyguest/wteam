// Copies Mermaid's browser build into public/vendor so the page loads it from
// the same origin. Mermaid renders client-side; the build needs no browser.
import { cpSync, mkdirSync, rmSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(import.meta.url);
const distDir = dirname(require.resolve('mermaid/package.json')) + '/dist';
const target = join(here, '..', 'public', 'vendor', 'mermaid');

rmSync(target, { recursive: true, force: true });
mkdirSync(target, { recursive: true });
cpSync(distDir, target, { recursive: true });
console.log(`mermaid vendored to ${target}`);
