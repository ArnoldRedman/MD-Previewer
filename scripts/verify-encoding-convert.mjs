import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

const root = fileURLToPath(new URL('..', import.meta.url));
const mainRs = await readFile(resolve(root, 'src/main.rs'), 'utf8');

// 静态检查：转码与另存为的消息必须由 Rust 侧接住
for (const marker of [
  '"convert-encoding" => {',
  'IpcMessage::ConvertEncoding { encoding, content } =>',
  'body.strip_prefix("save-as\\n")',
  'data-convert-encoding="UTF-8 BOM"',
  'id="btn-save-as"',
]) {
  if (!mainRs.includes(marker)) throw new Error(`Expected ${marker} in main.rs`);
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
  .replaceAll('{btn_edit_js}', 'Edit')
  .replaceAll('{btn_preview_js}', 'Preview')
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

const styleMarker = '<style>\n:root';
const styleStart = mainRs.indexOf(styleMarker);
const styleEnd = mainRs.indexOf('</style>', styleStart);
if (styleStart < 0 || styleEnd < 0) throw new Error('desktop style block not found');
const desktopStyle = mainRs.slice(styleStart, styleEnd + '</style>'.length)
  .replaceAll('{{', '{')
  .replaceAll('}}', '}')
  .replaceAll('{sidebar_toggle_left}', '272')
  .replaceAll('{sidebar_width}', '260');

const encodings = ['UTF-8', 'UTF-8 BOM', 'GBK', 'UTF-16 LE', 'UTF-16 BE'];
const reopenButtons = encodings
  .map((enc) => `<button type="button" class="encoding-option${enc === 'UTF-8' ? ' active' : ''}" data-encoding="${enc}">${enc}</button>`)
  .join('');
const convertButtons = encodings
  .map((enc) => `<button type="button" class="encoding-option" data-convert-encoding="${enc}">${enc}</button>`)
  .join('');

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
await page.setContent(`<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    ${desktopStyle}
  </head>
  <body class="has-tabs">
    <div class="tabbar" id="tabbar">
      <div class="tabs" id="tabs"></div>
      <div class="doc-stats" id="doc-stats"></div>
      <div class="encoding-control" id="encoding-control">
        <button class="encoding-btn" id="btn-encoding" type="button" title="Encoding">UTF-8</button>
        <div class="encoding-popover" id="encoding-popover" style="display:none;" role="menu">
          <div class="encoding-popover-title">Encoding</div>
          <div class="encoding-group-title">Reopen with encoding</div>
          ${reopenButtons}
          <div class="encoding-group-title">Convert to</div>
          ${convertButtons}
          <div class="encoding-sep"></div>
          <button type="button" class="encoding-option" id="btn-save-as">Save As…</button>
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
    </aside>
    <div id="topbar">
      <div class="toolbar sidebar-toggle"><button id="btn-sidebar"></button></div>
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
    <div id="lightbox" class="lightbox" style="display:none;">
      <div class="lightbox-backdrop"></div>
      <div class="lightbox-toolbar">
        <button type="button" id="lb-zoom-out">−</button>
        <button type="button" id="lb-zoom-reset">100%</button>
        <button type="button" id="lb-zoom-in">+</button>
        <button type="button" id="lb-close">×</button>
      </div>
      <div class="lightbox-stage"><img id="lb-img" class="lightbox-img" alt=""></div>
      <div id="lb-caption" class="lightbox-caption"></div>
    </div>
    <div id="app">
      <div id="preview"><h1>标题</h1><p>内容</p></div>
      <textarea id="editor" spellcheck="false"># 标题&#10;内容</textarea>
    </div>
  </body>
</html>`);

await page.evaluate(() => {
  window.ipcMessages = [];
  window.ipc = { postMessage: (msg) => { window.ipcMessages.push(msg); } };
});
await page.evaluate((code) => {
  const script = document.createElement('script');
  script.textContent = code;
  document.body.appendChild(script);
}, desktopScript);

const messages = () => page.evaluate(() => window.ipcMessages);

console.log('Verifying 1: Popover groups and current encoding disabled in convert group...');
await page.click('#btn-encoding');
if (!(await page.locator('#encoding-popover').isVisible())) throw new Error('popover should open');
if ((await page.locator('[data-encoding]').count()) !== 5) throw new Error('expected 5 reopen options');
if ((await page.locator('[data-convert-encoding]').count()) !== 5) throw new Error('expected 5 convert options');
if (!(await page.locator('#btn-save-as').isVisible())) throw new Error('save-as button should be visible');
const initialDisabled = await page.$$eval('[data-convert-encoding]', (els) => els.filter((el) => el.disabled).map((el) => el.dataset.convertEncoding));
if (initialDisabled.join() !== 'UTF-8') throw new Error(`expected only UTF-8 disabled at start, got ${initialDisabled}`);
await page.evaluate(() => window.__setEncoding('GBK'));
const gbkDisabled = await page.$$eval('[data-convert-encoding]', (els) => els.filter((el) => el.disabled).map((el) => el.dataset.convertEncoding));
if (gbkDisabled.join() !== 'GBK') throw new Error(`expected only GBK disabled after __setEncoding, got ${gbkDisabled}`);
if (!(await page.$eval('[data-encoding="GBK"]', (el) => el.classList.contains('active')))) throw new Error('GBK reopen option should be active');

console.log('Verifying 2: Popover fits the viewport...');
const popoverBox = await page.locator('#encoding-popover').boundingBox();
if (popoverBox.x < 0 || popoverBox.x + popoverBox.width > 900 || popoverBox.y + popoverBox.height > 700) {
  throw new Error(`popover overflows viewport: ${JSON.stringify(popoverBox)}`);
}
const clipped = await page.$eval('#encoding-popover', (el) => el.scrollWidth > el.clientWidth + 1);
if (clipped) throw new Error('popover content is clipped horizontally');

console.log('Verifying 3: Convert option sends the encoding and full editor text...');
await page.click('[data-convert-encoding="UTF-8 BOM"]');
let msgs = await messages();
if (msgs[msgs.length - 1] !== 'convert-encoding:UTF-8 BOM\n# 标题\n内容') {
  throw new Error(`unexpected convert message: ${JSON.stringify(msgs)}`);
}
if (!(await page.$eval('#encoding-popover', (el) => el.style.display === 'none'))) throw new Error('popover should close after convert');

console.log('Verifying 4: Disabled convert option sends nothing...');
const before = (await messages()).length;
await page.$eval('[data-convert-encoding="GBK"]', (el) => el.click());
if ((await messages()).length !== before) throw new Error('disabled convert option must not send a message');

console.log('Verifying 5: Save As via shortcut and popover button...');
await page.keyboard.press('Control+Shift+S');
msgs = await messages();
if (msgs[msgs.length - 1] !== 'save-as\n# 标题\n内容') throw new Error(`unexpected save-as message: ${JSON.stringify(msgs)}`);
if (msgs.some((m) => m.startsWith('save:'))) throw new Error('Ctrl+Shift+S must not trigger a plain save');
await page.click('#btn-encoding');
await page.click('#btn-save-as');
msgs = await messages();
if (msgs.filter((m) => m.startsWith('save-as\n')).length !== 2) throw new Error('save-as button should send a second save-as message');

console.log('Verifying 6: Save As is ignored with no document open...');
await page.evaluate(() => document.body.classList.add('empty'));
const beforeEmpty = (await messages()).length;
await page.keyboard.press('Control+Shift+S');
if ((await messages()).length !== beforeEmpty) throw new Error('save-as must be ignored in empty state');

await browser.close();
console.log('[encoding-convert-verify] ALL CHECKS PASSED');
