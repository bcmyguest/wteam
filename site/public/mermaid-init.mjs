// Renders every ```mermaid code block as a diagram. Starlight emits each block
// as <pre data-language="mermaid">; the source text becomes a <div class="mermaid">.
import mermaid from './vendor/mermaid/mermaid.esm.min.mjs';

/** @returns {'light' | 'dark'} */
function resolvedTheme() {
  const theme = document.documentElement.dataset.theme;
  if (theme === 'light' || theme === 'dark') return theme;
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

/** @param {'light' | 'dark'} mode */
function mermaidConfig(mode) {
  const isDark = mode === 'dark';
  return {
    startOnLoad: false,
    securityLevel: 'strict',
    theme: isDark ? 'dark' : 'default',
    themeVariables: isDark
      ? {
          background: 'transparent',
          primaryTextColor: '#ffffff',
          secondaryTextColor: '#edeef3',
          tertiaryTextColor: '#edeef3',
          textColor: '#ffffff',
          lineColor: '#ffffff',
          arrowheadColor: '#ffffff',
          defaultLinkColor: '#ffffff',
          actorTextColor: '#ffffff',
          actorLineColor: '#ffffff',
          signalColor: '#ffffff',
          signalTextColor: '#ffffff',
          labelTextColor: '#ffffff',
          loopTextColor: '#ffffff',
          sequenceNumberColor: '#ffffff',
        }
      : undefined,
  };
}

/** @param {HTMLElement} el */
function resetDiagram(el) {
  const src = el.dataset.mermaidSource;
  if (!src) return false;
  el.removeAttribute('data-processed');
  el.textContent = src;
  return true;
}

async function renderDiagrams() {
  mermaid.initialize(mermaidConfig(resolvedTheme()));
  const nodes = document.querySelectorAll('.mermaid');
  for (const el of nodes) {
    resetDiagram(el);
  }
  if (nodes.length > 0) {
    await mermaid.run({ querySelector: '.mermaid' });
  }
}

const blocks = document.querySelectorAll('pre[data-language="mermaid"]');
for (const pre of blocks) {
  const div = document.createElement('div');
  div.className = 'mermaid';
  const source = pre.innerText.trim();
  div.dataset.mermaidSource = source;
  div.textContent = source;
  pre.replaceWith(div);
}

if (blocks.length > 0) {
  await renderDiagrams();

  const observer = new MutationObserver(() => {
    void renderDiagrams();
  });
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-theme'],
  });
}
