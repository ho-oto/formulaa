// mascii VSCode extension: structural math editing over AA-format formulas
// embedded in ```math fenced blocks. The editor logic is the Rust core
// compiled to WASM (media/pkg, built with `npm run build-wasm`); this file
// only finds/replaces fenced blocks and hosts the webview.

const vscode = require('vscode');
const path = require('path');

const FENCES = ['```math', '```mascii', '~~~math', '~~~mascii'];

/** Find the fenced math block containing `line`, or null. */
function findBlock(doc, line) {
  let open = null;
  for (let i = 0; i < doc.lineCount; i++) {
    const t = doc.lineAt(i).text.trim();
    if (open === null && FENCES.includes(t)) {
      open = i;
    } else if (open !== null && (t === '```' || t === '~~~')) {
      if (open <= line && line <= i) {
        return { start: open, end: i };
      }
      open = null;
    }
  }
  return null;
}

function blockText(doc, block) {
  const lines = [];
  for (let i = block.start + 1; i < block.end; i++) {
    lines.push(doc.lineAt(i).text);
  }
  return lines.join('\n');
}

function webviewHtml(webview, mediaUri) {
  const nonce = Math.random().toString(36).slice(2);
  return `<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy"
      content="default-src 'none'; style-src ${webview.cspSource};
               script-src 'nonce-${nonce}'; connect-src ${webview.cspSource};">
<link rel="stylesheet" href="${mediaUri}/editor.css">
</head>
<body>
<div id="help">\\cmd &nbsp; ^/_ scripts &nbsp; ( ) insets &nbsp; Tab exit / Space ␣ &nbsp;
⇧←→ select &nbsp; <b>Ctrl+Enter: apply</b> &nbsp; Esc Esc: cancel</div>
<pre id="screen" tabindex="0"></pre>
<div id="minibuffer"></div>
<div id="latex"></div>
<div id="message"></div>
<script type="module" nonce="${nonce}">
  import init, { MasciiEditor } from '${mediaUri}/pkg/mascii_wasm.js';
  const vscode = acquireVsCodeApi();
  await init();
  const ed = new MasciiEditor();

  const screen = document.getElementById('screen');
  function update() {
    screen.textContent = ed.screen();
    // style the cursor glyph
    screen.innerHTML = screen.innerHTML
      .replace('▌', '<span class="cursor">▌</span>')
      // selected cells carry a combining underline (U+0332)
      .replace(/(.\u0332)/g, '<span class="sel">$1</span>');
    const mb = ed.minibuffer();
    document.getElementById('minibuffer').textContent =
      mb === undefined || mb === null ? '' : '\\\\' + mb + '▌';
    document.getElementById('latex').textContent = 'LaTeX: ' + ed.latex();
    document.getElementById('message').textContent = ed.message();
  }

  window.addEventListener('message', (e) => {
    if (e.data.type === 'load') {
      try { ed.load(e.data.aa); } catch (err) { /* start empty */ }
      update();
      screen.focus();
    }
  });

  let lastEsc = 0;
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      vscode.postMessage({ type: 'accept', aa: ed.aa(), latex: ed.latex() });
      return;
    }
    if (e.key === 'Escape') {
      const now = Date.now();
      if (now - lastEsc < 500) { vscode.postMessage({ type: 'cancel' }); return; }
      lastEsc = now;
    }
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    ed.key(e.key, e.shiftKey);
    update();
    e.preventDefault();
  });

  vscode.postMessage({ type: 'ready' });
</script>
</body>
</html>`;
}

function editFormula(context) {
  const editor = vscode.window.activeTextEditor;
  if (!editor) return;
  const doc = editor.document;
  const pos = editor.selection.active;
  const block = findBlock(doc, pos.line);
  const initialAA = block ? blockText(doc, block) : '';

  const panel = vscode.window.createWebviewPanel(
    'masciiEditor',
    'mascii formula',
    vscode.ViewColumn.Beside,
    {
      enableScripts: true,
      localResourceRoots: [vscode.Uri.file(path.join(context.extensionPath, 'media'))],
    }
  );
  const mediaUri = panel.webview.asWebviewUri(
    vscode.Uri.file(path.join(context.extensionPath, 'media'))
  );
  panel.webview.html = webviewHtml(panel.webview, mediaUri);

  panel.webview.onDidReceiveMessage(async (msg) => {
    if (msg.type === 'ready') {
      panel.webview.postMessage({ type: 'load', aa: initialAA });
    } else if (msg.type === 'accept') {
      const we = new vscode.WorkspaceEdit();
      if (block) {
        const range = new vscode.Range(block.start + 1, 0, block.end, 0);
        we.replace(doc.uri, range, msg.aa + '\n');
      } else {
        we.insert(doc.uri, pos, '```math\n' + msg.aa + '\n```\n');
      }
      await vscode.workspace.applyEdit(we);
      panel.dispose();
    } else if (msg.type === 'cancel') {
      panel.dispose();
    }
  });
}

async function convertSelection() {
  const editor = vscode.window.activeTextEditor;
  if (!editor) return;
  const text = editor.document.getText(editor.selection);
  try {
    const wasm = require('./media/pkg-node/mascii_wasm.js');
    const out = wasm.aa_to_latex(text);
    await vscode.env.clipboard.writeText(out);
    vscode.window.showInformationMessage(`mascii: copied latex to clipboard: ${out}`);
  } catch (e) {
    vscode.window.showErrorMessage(`mascii: ${e.message || e}`);
  }
}

function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand('mascii.editFormula', () => editFormula(context)),
    vscode.commands.registerCommand('mascii.toLatex', () => convertSelection())
  );
}

module.exports = { activate, deactivate: () => {} };
