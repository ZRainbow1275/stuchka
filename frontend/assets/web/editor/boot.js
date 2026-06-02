// External boot shim (E4). Moved out of an inline <script> so the page boots under
// a strict CSP `script-src 'self'` (NO 'unsafe-inline') that is byte-identical in
// both the editor source `index.html` and the copied frontend asset
// `frontend/assets/web/editor/index.html`. connect-src stays 'none' (channel A, D1).
//
// Boot is driven by the Dart shell once it knows caseId/docId/kbHash; this shim only
// wires the global if the shell injects a ready signal.
window.addEventListener('DOMContentLoaded', function () {
  if (window.StuchkaEditor && window.__STUCHKA_BOOT__) {
    window.StuchkaEditor.boot({
      mount: document.getElementById('stuchka-editor'),
      caseId: window.__STUCHKA_BOOT__.caseId,
      docId: window.__STUCHKA_BOOT__.docId,
      kbHash: window.__STUCHKA_BOOT__.kbHash,
    });
  }
});
