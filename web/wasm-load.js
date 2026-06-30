// Shared "WebAssembly Engine Load Failed" error UI for the Dry browser demo.
//
// index.html and blocks.html both call showWasmLoadError(err) from the catch block after
// `await init()`, so the engine-load failure DOM (and its console diagnostic) is built in a
// single place instead of being copy-pasted into each page's module script.

export function showWasmLoadError(err) {
  console.error("Wasm load failed:", err);
  const statusMsg = document.createElement('div');
  statusMsg.style.color = '#ff6b6b';
  statusMsg.style.padding = '20px';
  statusMsg.style.background = '#2a1a1a';
  statusMsg.style.border = '1px solid #ff4444';
  statusMsg.style.borderRadius = '8px';
  statusMsg.style.margin = '20px';
  const h3 = document.createElement('h3');
  h3.textContent = 'WebAssembly Engine Load Failed';
  statusMsg.appendChild(h3);
  const p1 = document.createElement('p');
  p1.textContent = err.message || String(err);
  statusMsg.appendChild(p1);
  const p2 = document.createElement('p');
  p2.textContent = 'Please run bash build.sh or refresh the page.';
  statusMsg.appendChild(p2);
  document.body.prepend(statusMsg);
}
