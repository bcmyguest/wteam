// Renders every ```mermaid code block as a diagram. Starlight emits each block
// as <pre data-language="mermaid">; the source text becomes a <div class="mermaid">.
import mermaid from './vendor/mermaid/mermaid.esm.min.mjs';

const blocks = document.querySelectorAll('pre[data-language="mermaid"]');
if (blocks.length > 0) {
  mermaid.initialize({ startOnLoad: false, securityLevel: 'strict' });
  for (const pre of blocks) {
    const div = document.createElement('div');
    div.className = 'mermaid';
    div.textContent = pre.innerText;
    pre.replaceWith(div);
  }
  await mermaid.run({ querySelector: '.mermaid' });
}
