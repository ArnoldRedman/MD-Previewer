import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

const root = fileURLToPath(new URL('..', import.meta.url));
const mainRs = await readFile(resolve(root, 'src/main.rs'), 'utf8');

// Validate CSS styling directly from main.rs source
if (!mainRs.includes('body.editing #btn-split {{ display: grid; place-items: center; }}')) {
  throw new Error('Expected #btn-split to be styled with display: grid; place-items: center;');
}
if (!mainRs.includes('order: 1;') || !mainRs.includes('order: 2;')) {
  throw new Error('Expected split-view to order editor (1) and preview (2)');
}
if (!mainRs.includes('border-right: 2px solid #d0d7de;')) {
  throw new Error('Expected split-view editor to have 2px solid right border divider');
}

const marker = 'var ICON_EDIT';
const markerIndex = mainRs.indexOf(marker);
if (markerIndex < 0) throw new Error('desktop script marker not found');
const scriptStart = mainRs.lastIndexOf('<script>', markerIndex);
const scriptEnd = mainRs.indexOf('</script>', markerIndex);
if (scriptStart < 0 || scriptEnd < 0) throw new Error('desktop script block not found');

const desktopScript = mainRs
  .slice(scriptStart + '<script>'.length, scriptEnd)
  .replaceAll('{{', '{')
  .replaceAll('}}', '}')
  .replaceAll('{btn_edit}', 'Edit')
  .replaceAll('{btn_preview}', 'Preview')
  .replaceAll('{btn_split}', 'Split View')
  .replaceAll('{code_copy_js}', 'Copy')
  .replaceAll('{code_copied_js}', 'Copied')
  .replaceAll('{sidebar_empty_js}', 'Nothing to show')
  .replaceAll('{sidebar_outline_empty_js}', 'No headings')
  .replaceAll('{stat_words_js}', 'words')
  .replaceAll('{stat_chars_js}', 'chars')
  .replaceAll('{copy_title_line_js}', 'Copy heading')
  .replaceAll('{copy_title_js}', 'Copy title')
  .replaceAll('{copy_body_js}', 'Copy body')
  .replaceAll('{copied_js}', 'Copied')
  .replaceAll('{tab_menu_close}', 'Close Tab')
  .replaceAll('{tab_menu_close_others}', 'Close Others')
  .replaceAll('{tab_menu_copy_path}', 'Copy Path')
  .replaceAll('{tab_menu_reveal}', 'Reveal in File Manager');

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1000, height: 750 } });

await page.setContent(`<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <style>
      .tabbar { display: flex; align-items: center; gap: 4px; }
      body.editing #btn-split { display: grid; place-items: center; }
      body.editing.split-view #app { display: flex; flex-direction: row; }
      body.editing.split-view #editor { order: 1; flex: 1 1 50%; border-right: 2px solid #d0d7de; }
      body.editing.split-view #preview { order: 2; flex: 1 1 50%; }
      body { padding-left: 240px; }
      .sidebar { position: fixed; top: 0; left: 0; bottom: 0; width: 240px; display: flex; flex-direction: column; }
      .sidebar-list { flex: 1; overflow-y: auto; }
      .sidebar-tooltip { position: fixed; max-width: 420px; }
      .context-menu { position: fixed; }
    </style>
  </head>
  <body class="has-tabs">
    <div class="tabbar" id="tabbar">
      <div class="tabs" id="tabs"></div>
      <div class="doc-stats" id="doc-stats"></div>
      <div class="encoding-control" id="encoding-control">
        <button class="encoding-btn" id="btn-encoding" type="button" title="Encoding">UTF-8</button>
        <div class="encoding-popover" id="encoding-popover" style="display:none;" role="menu">
          <div class="encoding-popover-title">Encoding</div>
          <button type="button" class="encoding-option active" data-encoding="UTF-8">UTF-8</button>
          <button type="button" class="encoding-option" data-encoding="GBK">GBK / ANSI</button>
          <button type="button" class="encoding-option" data-encoding="UTF-16 LE">UTF-16 LE</button>
          <button type="button" class="encoding-option" data-encoding="UTF-16 BE">UTF-16 BE</button>
        </div>
      </div>
      <button class="tab-open" id="tab-open" type="button">+</button>
    </div>
    <aside class="sidebar" id="sidebar">
      <div class="sidebar-sections">
        <button type="button" data-sidebar-section="folder">Folder</button>
        <button type="button" data-sidebar-section="recent">Recent</button>
        <button type="button" data-sidebar-section="outline">Outline</button>
      </div>
      <div class="sidebar-list" id="sidebar-list"></div>
      <div class="sidebar-footer" id="sidebar-footer" style="display:none;"><button type="button" class="sidebar-clear" id="sidebar-clear-recent">Clear Recent Files</button></div>
    </aside>
    <div id="topbar">
      <div class="toolbar sidebar-toggle">
        <button id="btn-sidebar"></button>
      </div>
      <div class="toolbar">
        <button id="btn-open"></button>
        <button id="btn-search"></button>
        <button id="btn-toggle"></button>
        <button id="btn-split"></button>
        <button id="btn-print"></button>
        <div class="zoom-control" id="zoom-control">
          <button id="btn-zoom"></button>
          <div class="zoom-popover">
            <button id="btn-zoom-out">−</button>
            <button id="btn-zoom-reset">100%</button>
            <button id="btn-zoom-in">+</button>
          </div>
        </div>
        <div class="settings-control" id="settings-control">
          <button id="btn-settings"></button>
          <div class="settings-popover">
            <div class="settings-row">
              <span class="settings-label">Word wrap</span>
              <div class="settings-seg">
                <button type="button" data-setting="word-wrap" data-value="on" aria-pressed="true">On</button>
                <button type="button" data-setting="word-wrap" data-value="off" aria-pressed="false">Off</button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
    <div class="findbar">
      <input id="find-input">
      <span id="find-state"></span>
      <button id="find-prev"></button>
      <button id="find-next"></button>
      <button id="find-close"></button>
    </div>
    <div id="tab-context-menu" class="context-menu" style="display:none;">
      <button type="button" class="context-menu-item" data-tab-action="close">Close Tab</button>
      <button type="button" class="context-menu-item" data-tab-action="close-others">Close Others</button>
      <div class="context-menu-sep"></div>
      <button type="button" class="context-menu-item" data-tab-action="copy-path">Copy Path</button>
      <button type="button" class="context-menu-item" data-tab-action="reveal">Reveal in File Manager</button>
    </div>
    <div id="recent-context-menu" class="context-menu" style="display:none;">
      <button type="button" class="context-menu-item" data-recent-action="reveal">Reveal in File Manager</button>
      <button type="button" class="context-menu-item" data-recent-action="copy-path">Copy Path</button>
      <div class="context-menu-sep"></div>
      <button type="button" class="context-menu-item" data-recent-action="remove">Remove from Recent</button>
    </div>
    <div id="sidebar-tooltip" class="sidebar-tooltip" style="display:none;"></div>
    <div id="lightbox" class="lightbox" style="display:none;">
      <div class="lightbox-backdrop"></div>
      <div class="lightbox-toolbar">
        <button type="button" id="lb-zoom-out">−</button>
        <button type="button" id="lb-zoom-reset">100%</button>
        <button type="button" id="lb-zoom-in">+</button>
        <button type="button" id="lb-close">×</button>
      </div>
      <div class="lightbox-stage">
        <img id="lb-img" class="lightbox-img" alt="">
      </div>
      <div id="lb-caption" class="lightbox-caption"></div>
    </div>
    <div id="app">
      <div id="preview">
        <h1 id="sec-1">First Section</h1>
        <p>Some paragraph text.</p>
        <pre><code>console.log("hello world");</code></pre>
        <h2 id="sec-2">Second Section</h2>
        <img id="test-img" src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==" alt="Sample Image">
      </div>
      <textarea id="editor" spellcheck="false"># First Section&#10;&#10;\`\`\`javascript&#10;console.log("hello world");&#10;\`\`\`&#10;&#10;## Second Section&#10;&#10;![Sample Image](sample.png)</textarea>
    </div>
  </body>
</html>`);

// Mock IPC
await page.evaluate(() => {
  window.ipcMessages = [];
  window.ipc = {
    postMessage: (msg) => { window.ipcMessages.push(msg); }
  };
});

// Inject script
await page.evaluate((code) => {
  const script = document.createElement('script');
  script.textContent = code;
  document.body.appendChild(script);
}, desktopScript);

console.log('Verifying 1: Code block copy button...');
const copyBtn = page.locator('#preview pre .code-copy-btn');
if (await copyBtn.count() === 0) throw new Error('Code copy button not mounted in pre');
const copyText = await copyBtn.first().textContent();
if (copyText !== 'Copy') throw new Error(`Expected button text 'Copy', got '${copyText}'`);

console.log('Verifying 2: Sidebar Outline / TOC...');
// Click Outline tab
await page.click('button[data-sidebar-section="outline"]');
const outlineItems = page.locator('#sidebar-list .outline-item');
const outlineCount = await outlineItems.count();
if (outlineCount !== 2) throw new Error(`Expected 2 outline items, got ${outlineCount}`);
const item1Text = await outlineItems.nth(0).textContent();
const item2Text = await outlineItems.nth(1).textContent();
if (!item1Text.includes('First Section') || !item2Text.includes('Second Section')) {
  throw new Error(`Outline item text mismatch: "${item1Text}", "${item2Text}"`);
}

console.log('Verifying 3: Tab ergonomics & context menu...');
// Render tabs
await page.evaluate(() => {
  window.__setTabs([
    { id: 1, name: 'file1.md', path: '/path/file1.md', active: true, missing: false, dirty: false },
    { id: 2, name: 'file2.md', path: '/path/file2.md', active: false, missing: false, dirty: false }
  ]);
});

// Right click tab 1
const tab1 = page.locator('.tab[data-tab-id="1"]');
await tab1.click({ button: 'right' });
const contextMenu = page.locator('#tab-context-menu');
const isMenuVisible = await contextMenu.evaluate((el) => el.style.display !== 'none');
if (!isMenuVisible) throw new Error('Tab context menu did not open on right click');

// Click "Close Others"
await page.click('[data-tab-action="close-others"]');
const lastMsg = await page.evaluate(() => window.ipcMessages[window.ipcMessages.length - 1]);
if (lastMsg !== 'tab-action:close-others:1') {
  throw new Error(`Expected 'tab-action:close-others:1', got '${lastMsg}'`);
}
const isMenuHidden = await contextMenu.evaluate((el) => el.style.display === 'none');
if (!isMenuHidden) throw new Error('Context menu did not close after item click');

// Middle click on tab 2
const tab2 = page.locator('.tab[data-tab-id="2"]');
await tab2.click({ button: 'middle' });
const middleMsg = await page.evaluate(() => window.ipcMessages[window.ipcMessages.length - 1]);
if (middleMsg !== 'tab-action:close:2') {
  throw new Error(`Expected 'tab-action:close:2' for middle click, got '${middleMsg}'`);
}

console.log('Verifying 4: Image Lightbox...');
// Click image to open lightbox
await page.click('#test-img');
const lightbox = page.locator('#lightbox');
const isLbVisible = await lightbox.evaluate((el) => el.style.display !== 'none');
if (!isLbVisible) throw new Error('Lightbox did not open upon image click');
const lbImgSrc = await page.locator('#lb-img').getAttribute('src');
if (!lbImgSrc || !lbImgSrc.startsWith('data:image')) throw new Error('Lightbox image src mismatch');
const lbCaption = await page.locator('#lb-caption').textContent();
if (lbCaption !== 'Sample Image') throw new Error(`Lightbox caption mismatch: ${lbCaption}`);

// Press Escape to close lightbox
await page.keyboard.press('Escape');
const isLbClosed = await lightbox.evaluate((el) => el.style.display === 'none');
if (!isLbClosed) throw new Error('Lightbox did not close upon Escape key');

console.log('Verifying 5: Split View editing mode...');
// Enter edit mode
await page.evaluate(() => { window.__mdPreviewerEnterEdit(); });
const isEditing = await page.evaluate(() => document.body.classList.contains('editing'));
if (!isEditing) throw new Error('Expected editing mode to be active');

const btnSplit = page.locator('#btn-split');
const btnDisplay = await btnSplit.evaluate((el) => window.getComputedStyle(el).display);
if (btnDisplay !== 'grid') throw new Error(`Expected #btn-split display: grid, got ${btnDisplay}`);

// Verify SVG icon center within the button
const btnGeom = await btnSplit.evaluate((el) => {
  const rect = el.getBoundingClientRect();
  const svg = el.querySelector('svg');
  if (!svg) return null;
  const svgRect = svg.getBoundingClientRect();
  const btnCenterX = rect.left + rect.width / 2;
  const btnCenterY = rect.top + rect.height / 2;
  const svgCenterX = svgRect.left + svgRect.width / 2;
  const svgCenterY = svgRect.top + svgRect.height / 2;
  return {
    diffX: Math.abs(btnCenterX - svgCenterX),
    diffY: Math.abs(btnCenterY - svgCenterY)
  };
});
if (!btnGeom) throw new Error('SVG not found inside #btn-split');
if (btnGeom.diffX > 0.5 || btnGeom.diffY > 0.5) {
  throw new Error(`Split icon is not centered! diffX=${btnGeom.diffX}, diffY=${btnGeom.diffY}`);
}

// Click Split View button
await page.click('#btn-split');
const isSplit = await page.evaluate(() => document.body.classList.contains('split-view'));
if (!isSplit) throw new Error('Expected split-view class on body');

// Verify layout: Editor on Left (order 1), Preview on Right (order 2), with distinct divider
const editor = page.locator('#editor');
const preview = page.locator('#preview');
const editorOrder = await editor.evaluate((el) => window.getComputedStyle(el).order);
const previewOrder = await preview.evaluate((el) => window.getComputedStyle(el).order);
if (editorOrder !== '1') throw new Error(`Expected #editor order: 1, got ${editorOrder}`);
if (previewOrder !== '2') throw new Error(`Expected #preview order: 2, got ${previewOrder}`);

const editorBox = await editor.boundingBox();
const previewBox = await preview.boundingBox();
if (editorBox.x >= previewBox.x) {
  throw new Error(`Expected editor on the left (x=${editorBox.x}) and preview on the right (x=${previewBox.x})`);
}

const dividerStyle = await editor.evaluate((el) => {
  const cs = window.getComputedStyle(el);
  return {
    borderRightWidth: parseFloat(cs.borderRightWidth),
    borderRightStyle: cs.borderRightStyle
  };
});
if (dividerStyle.borderRightWidth < 1 || dividerStyle.borderRightStyle !== 'solid') {
  throw new Error(`Expected solid border-right divider on #editor, got: ${JSON.stringify(dividerStyle)}`);
}

// Verify Live Preview update
await page.evaluate(() => {
  window.__setLivePreview('<h1>Updated Content</h1>', false, false);
});
const previewH1 = await page.locator('#preview h1').textContent();
if (previewH1 !== 'Updated Content') throw new Error(`Live preview update failed, got '${previewH1}'`);

console.log('Verifying 6: Settings word wrap toggle...');
await page.evaluate(() => {
  window.__setSettings({ wordWrap: false });
});
const isNoWrap = await page.evaluate(() => document.body.classList.contains('no-wrap'));
if (!isNoWrap) throw new Error('Expected body to have no-wrap class');

await page.evaluate(() => {
  window.__setSettings({ wordWrap: true });
});
const isWrap = await page.evaluate(() => !document.body.classList.contains('no-wrap'));
if (!isWrap) throw new Error('Expected body to not have no-wrap class');

console.log('Verifying 7: Encoding display and switcher...');
const encodingBtn = page.locator('#btn-encoding');
const docStats = page.locator('#doc-stats');
if ((await encodingBtn.count()) !== 1) throw new Error('#btn-encoding not found');

// Verify initial encoding text
const initialEnc = await encodingBtn.textContent();
if (initialEnc.trim() !== 'UTF-8') throw new Error(`Expected UTF-8 initial encoding, got ${initialEnc}`);

// Verify positioning: encoding-control is placed next to doc-stats
const statsBox = await docStats.boundingBox();
const encBox = await encodingBtn.boundingBox();
if (encBox.x <= statsBox.x) throw new Error('Expected encoding button to be to the right of doc-stats');

// Click to open popover
await encodingBtn.click();
const popover = page.locator('#encoding-popover');
const isPopoverVisible = await popover.isVisible();
if (!isPopoverVisible) throw new Error('Expected encoding popover to be visible after clicking #btn-encoding');

// Check available options
const options = await page.locator('.encoding-option').allTextContents();
const expectedOptions = ['UTF-8', 'GBK', 'UTF-16 LE', 'UTF-16 BE'];
for (const opt of expectedOptions) {
  if (!options.some((o) => o.includes(opt))) {
    throw new Error(`Expected encoding option ${opt} not found in ${options}`);
  }
}

// Click GBK option
await page.click('.encoding-option[data-encoding="GBK"]');
const isPopoverClosed = await popover.evaluate((el) => el.style.display === 'none');
if (!isPopoverClosed) throw new Error('Expected encoding popover to close after option selection');

// Check window.__setEncoding UI updates
await page.evaluate(() => window.__setEncoding('GBK'));
const currentBtnText = await encodingBtn.textContent();
if (currentBtnText.trim() !== 'GBK') throw new Error(`Expected button text GBK, got ${currentBtnText}`);

const isGbkActive = await page.$eval('.encoding-option[data-encoding="GBK"]', (el) => el.classList.contains('active'));
if (!isGbkActive) throw new Error('Expected GBK option to have active class');

// Test Escape closes popover
await encodingBtn.click();
await page.keyboard.press('Escape');
const isClosedOnEsc = await popover.evaluate((el) => el.style.display === 'none');
if (!isClosedOnEsc) throw new Error('Expected encoding popover to close on Escape key');

console.log('Verifying 8: Sidebar recent history tools...');
const recentPath = 'D:\\notes\\deep\\folder\\history.md';
await page.evaluate((path) => {
  window.__setSidebar({
    folder: [{ name: 'local.md', path: '/docs/local.md', dir: 'docs', active: true }],
    recent: [{ name: 'history.md', path, dir: 'folder', active: false }]
  });
}, recentPath);
await page.click('button[data-sidebar-section="recent"]');
const recentItem = page.locator('#sidebar-list [data-sidebar-path]');
if ((await recentItem.count()) !== 1) throw new Error('Expected exactly one recent item');
const footer = page.locator('#sidebar-footer');
if (await footer.evaluate((el) => el.style.display === 'none')) {
  throw new Error('Clear-recent footer should be visible when recent list is non-empty');
}

// 悬浮显示完整路径
await recentItem.hover();
await page.waitForFunction(() => {
  const tip = document.getElementById('sidebar-tooltip');
  return tip && tip.style.display !== 'none' && tip.textContent.length > 0;
}, null, { timeout: 2000 });
const tooltipText = await page.locator('#sidebar-tooltip').textContent();
if (tooltipText !== recentPath) throw new Error(`Tooltip should show full path, got '${tooltipText}'`);
const tooltipBox = await page.locator('#sidebar-tooltip').boundingBox();
const itemBox = await recentItem.boundingBox();
if (!tooltipBox || tooltipBox.x < itemBox.x + itemBox.width) {
  throw new Error('Tooltip should sit to the right of the sidebar item');
}
await page.mouse.move(600, 600);
if (!(await page.locator('#sidebar-tooltip').evaluate((el) => el.style.display === 'none'))) {
  throw new Error('Tooltip should hide when the pointer leaves the item');
}

// 右键：从历史中移除
await recentItem.click({ button: 'right' });
const recentMenu = page.locator('#recent-context-menu');
if (await recentMenu.evaluate((el) => el.style.display === 'none')) {
  throw new Error('Recent context menu did not open on right click');
}
await page.click('[data-recent-action="remove"]');
const removeMsg = await page.evaluate(() => window.ipcMessages[window.ipcMessages.length - 1]);
if (removeMsg !== 'forget-recent:' + recentPath) {
  throw new Error(`Expected 'forget-recent:${recentPath}', got '${removeMsg}'`);
}
if (!(await recentMenu.evaluate((el) => el.style.display === 'none'))) {
  throw new Error('Recent context menu did not close after remove');
}

// 右键：在文件管理器中显示
await recentItem.click({ button: 'right' });
await page.click('[data-recent-action="reveal"]');
const revealMsg = await page.evaluate(() => window.ipcMessages[window.ipcMessages.length - 1]);
if (revealMsg !== 'reveal-path:' + recentPath) {
  throw new Error(`Expected 'reveal-path:${recentPath}', got '${revealMsg}'`);
}

// Escape 关闭菜单，点击空白处也关闭
await recentItem.click({ button: 'right' });
await page.keyboard.press('Escape');
if (!(await recentMenu.evaluate((el) => el.style.display === 'none'))) {
  throw new Error('Recent context menu should close on Escape');
}
await recentItem.click({ button: 'right' });
await page.mouse.click(700, 500);
if (!(await recentMenu.evaluate((el) => el.style.display === 'none'))) {
  throw new Error('Recent context menu should close when clicking elsewhere');
}

// 底部一键清空
await page.click('#sidebar-clear-recent');
const clearMsg = await page.evaluate(() => window.ipcMessages[window.ipcMessages.length - 1]);
if (clearMsg !== 'clear-recent') throw new Error(`Expected 'clear-recent', got '${clearMsg}'`);
await page.evaluate(() => { window.__setSidebar({ folder: [], recent: [] }); });
if (!(await footer.evaluate((el) => el.style.display === 'none'))) {
  throw new Error('Clear-recent footer should hide when recent list is empty');
}

// 当前文件夹分区右键不弹历史菜单
await page.evaluate(() => {
  window.__setSidebar({ folder: [{ name: 'local.md', path: '/docs/local.md', dir: 'docs', active: true }], recent: [] });
});
await page.click('button[data-sidebar-section="folder"]');
await page.locator('#sidebar-list [data-sidebar-path]').click({ button: 'right' });
if (!(await recentMenu.evaluate((el) => el.style.display === 'none'))) {
  throw new Error('Recent context menu must not open for folder items');
}

await browser.close();
console.log('[ux-improvements-verify] ALL 8 CHECKS PASSED');
