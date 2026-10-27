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

/** @returns {HTMLDialogElement} */
function ensureLightbox() {
  let dialog = document.getElementById('wteam-mermaid-lightbox');
  if (dialog instanceof HTMLDialogElement) return dialog;

  dialog = document.createElement('dialog');
  dialog.id = 'wteam-mermaid-lightbox';
  dialog.className = 'wteam-mermaid-lightbox';
  dialog.innerHTML = `<div class="wteam-mermaid-lightbox__panel" role="document">
    <button type="button" class="wteam-mermaid-lightbox__close" aria-label="Close diagram">×</button>
    <div class="wteam-mermaid-lightbox__content"></div>
  </div>`;
  document.body.appendChild(dialog);

  const close = () => dialog.close();
  dialog.querySelector('.wteam-mermaid-lightbox__close')?.addEventListener('click', close);
  dialog.addEventListener('click', (e) => {
    if (e.target === dialog) close();
  });

  return dialog;
}

function bindMermaidLightbox() {
  const dialog = ensureLightbox();
  const content = dialog.querySelector('.wteam-mermaid-lightbox__content');
  if (!(content instanceof HTMLElement)) return;

  for (const el of document.querySelectorAll('.mermaid')) {
    if (!(el instanceof HTMLElement) || el.dataset.lightboxBound === '1') continue;
    if (!el.querySelector('svg')) continue;

    el.dataset.lightboxBound = '1';
    el.classList.add('mermaid-zoomable');
    el.tabIndex = 0;
    el.setAttribute('role', 'button');
    el.setAttribute('aria-label', 'Expand diagram');

    const open = () => {
      const svg = el.querySelector('svg');
      if (!svg) return;
      content.replaceChildren();
      const clone = svg.cloneNode(true);
      if (!(clone instanceof SVGElement)) return;
      clone.removeAttribute('width');
      clone.removeAttribute('height');
      clone.style.width = '100%';
      clone.style.maxWidth = 'min(96vw, 72rem)';
      clone.style.height = 'auto';
      content.appendChild(clone);
      dialog.showModal();
    };

    el.addEventListener('click', open);
    el.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        open();
      }
    });
  }
}

async function renderDiagrams() {
  mermaid.initialize(mermaidConfig(resolvedTheme()));
  const nodes = document.querySelectorAll('.mermaid');
  for (const el of nodes) {
    resetDiagram(el);
  }
  if (nodes.length > 0) {
    await mermaid.run({ querySelector: '.mermaid' });
    bindMermaidLightbox();
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
