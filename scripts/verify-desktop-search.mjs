import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

const root = fileURLToPath(new URL('..', import.meta.url));
const mainRs = await readFile(resolve(root, 'src/main.rs'), 'utf8');
const marker = "var ICON_EDIT";
const markerIndex = mainRs.indexOf(marker);
if (markerIndex < 0) throw new Error('desktop script marker not found');
const scriptStart = mainRs.lastIndexOf('<script>', markerIndex);
const scriptEnd = mainRs.indexOf('</script>', markerIndex);
if (scriptStart < 0 || scriptEnd < 0) throw new Error('desktop script block not found');

const desktopScript = mainRs
  .slice(scriptStart + '<script>'.length, scriptEnd)
  .replaceAll('{{', '{')
  .replaceAll('}}', '}')
  // 主 IIFE 里的 Rust 字符串占位符，测试页给空值即可
  .replaceAll('{btn_edit}', 'Edit')
  .replaceAll('{btn_preview}', 'Preview')
  .replaceAll('{btn_edit_js}', 'Edit')
  .replaceAll('{btn_preview_js}', 'Preview')
  .replaceAll('{sidebar_empty_js}', 'empty')
  .replaceAll('{stat_words_js}', '字')
  .replaceAll('{stat_chars_js}', '字符')
  .replaceAll('{copy_title_line_js}', 'Copy heading')
  .replaceAll('{copy_title_js}', 'Copy title')
  .replaceAll('{copy_body_js}', 'Copy body')
  .replaceAll('{copied_js}', 'Copied');

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 900, height: 700 } });

await page.setContent(`<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <style>
      body.finding .findbar { display: flex; }
      .findbar { display: none; }
      #preview mark.search-hit { background: #fff2a8; }
      #preview mark.search-hit.current { background: #ffcc4d; }
      .spacer { height: 900px; }
    </style>
  </head>
  <body>
    <div class="tabbar">
      <div id="tabs"></div>
      <div id="doc-stats"></div>
      <button id="tab-open"></button>
    </div>
    <div class="toolbar">
      <button id="btn-open"></button>
      <button id="btn-search"></button>
      <button id="btn-toggle"></button>
      <button id="btn-print"></button>
      <div id="zoom-control">
        <button id="btn-zoom"></button>
        <button id="btn-zoom-out"></button>
        <button id="btn-zoom-reset"></button>
        <button id="btn-zoom-in"></button>
      </div>
      <button id="btn-update" hidden></button>
      <button id="btn-settings"></button>
      <div id="settings-control"></div>
    </div>
    <button id="btn-sidebar"></button>
    <aside id="sidebar"><div id="sidebar-list"></div></aside>
    <div class="findbar">
      <input id="find-input" type="search">
      <span id="find-state"></span>
      <button id="find-prev"></button>
      <button id="find-next"></button>
      <button id="find-close"></button>
    </div>
    <div id="app">
      <div id="preview">
        <p>Alpha beta Beta</p>
        <p><code>beta()</code> should be searchable.</p>
        <svg><text>beta in svg should not be rewritten</text></svg>
        <div class="spacer"></div>
        <p>Final beta target</p>
      </div>
      <textarea id="editor"></textarea>
    </div>
    <script>
      window.__messages = [];
      window.ipc = { postMessage(message) { window.__messages.push(message); } };
      window.__mdPreviewerFeatureFlags = { math: false, mermaid: false };
    </script>
    <script>${desktopScript}</script>
  </body>
</html>`);

await page.locator('#btn-search').click();
await page.locator('#find-input').fill('beta');
await page.waitForFunction(() => document.querySelectorAll('#preview mark.search-hit').length === 4);

let result = await page.evaluate(() => ({
  finding: document.body.classList.contains('finding'),
  activeId: document.activeElement && document.activeElement.id,
  hitCount: document.querySelectorAll('#preview mark.search-hit').length,
  currentCount: document.querySelectorAll('#preview mark.search-hit.current').length,
  state: document.getElementById('find-state').textContent,
  svgRewritten: !!document.querySelector('svg mark.search-hit'),
}));

if (!result.finding ||
    result.activeId !== 'find-input' ||
    result.hitCount !== 4 ||
    result.currentCount !== 1 ||
    result.state !== '1/4' ||
    result.svgRewritten) {
  throw new Error(`initial desktop search failed: ${JSON.stringify(result)}`);
}

await page.locator('#find-input').press('Enter');
result = await page.evaluate(() => ({
  state: document.getElementById('find-state').textContent,
  currentText: document.querySelector('#preview mark.search-hit.current')?.textContent,
}));
if (result.state !== '2/4' || result.currentText !== 'Beta') {
  throw new Error(`desktop search next failed: ${JSON.stringify(result)}`);
}

await page.locator('#find-prev').click();
result = await page.evaluate(() => ({
  state: document.getElementById('find-state').textContent,
  currentText: document.querySelector('#preview mark.search-hit.current')?.textContent,
}));
if (result.state !== '1/4' || result.currentText !== 'beta') {
  throw new Error(`desktop search previous failed: ${JSON.stringify(result)}`);
}

await page.locator('#find-close').click();
result = await page.evaluate(() => ({
  finding: document.body.classList.contains('finding'),
  inputValue: document.getElementById('find-input').value,
  state: document.getElementById('find-state').textContent,
  hitCount: document.querySelectorAll('#preview mark.search-hit').length,
  previewText: document.getElementById('preview').textContent,
}));
if (result.finding ||
    result.inputValue !== '' ||
    result.state !== '' ||
    result.hitCount !== 0 ||
    !result.previewText.includes('Alpha beta Beta')) {
  throw new Error(`desktop search close failed: ${JSON.stringify(result)}`);
}

await page.keyboard.down(process.platform === 'darwin' ? 'Meta' : 'Control');
await page.keyboard.press('F');
await page.keyboard.up(process.platform === 'darwin' ? 'Meta' : 'Control');
await page.evaluate(() => {
  window.__setContent('<div class="mdp-plain-text">First line plain text\nSecond line plain search_target\nThird line target</div>', 'First line plain text\nSecond line plain search_target\nThird line target', '', false, false);
});
await page.locator('#btn-search').click();
await page.locator('#find-input').fill('plain');
await page.waitForFunction(() => document.querySelectorAll('#preview mark.search-hit').length === 2);
const plainResult = await page.evaluate(() => ({
  hitCount: document.querySelectorAll('#preview mark.search-hit').length,
  inPlainText: !!document.querySelector('.mdp-plain-text mark.search-hit')
}));
if (plainResult.hitCount !== 2 || !plainResult.inPlainText) {
  throw new Error(`desktop search on plain text failed: ${JSON.stringify(plainResult)}`);
}

await browser.close();
console.log('[desktop-search-verify] OK');
