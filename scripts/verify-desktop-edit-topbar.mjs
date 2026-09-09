// 编辑模式顶栏验证：进入编辑模式后，工具栏不再悬浮遮挡正文，
// 而是独占一条 sticky 顶栏，正文文本与顶栏不重叠。
import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import { join as resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

const root = fileURLToPath(new URL('..', import.meta.url));
const mainRs = await readFile(resolve(root, 'src/main.rs'), 'utf8');
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
  .replaceAll('{stat_words_js}', '字')
  .replaceAll('{stat_chars_js}', '字符');

// 样式块：从 <style> 提取，替换 Rust 占位符后注入真实页面
const styleStart = mainRs.indexOf('\n<style>\n');
const styleEnd = mainRs.indexOf('</style>', styleStart);
if (styleStart < 0 || styleEnd < 0) throw new Error('style block not found');
const realCss = mainRs
  .slice(styleStart + '<style>\n'.length, styleEnd)
  .replaceAll('{{', '{')
  .replaceAll('}}', '}')
  .replaceAll('{sidebar_width}', '260')
  .replaceAll('{sidebar_toggle_left}', '272')
  .replaceAll('{css_light}', '')
  .replaceAll('{css_dark}', '');

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
await page.route('https://md-previewer.test/**', (route) => route.fulfill({
  contentType: 'text/html',
  body: '<!doctype html><title>MD Previewer test</title>',
}));
await page.goto('https://md-previewer.test/');
const editorText = Array.from(
  { length: 120 },
  (_, index) => `Editor source line ${index + 1}`,
).join('\n');

await page.setContent(`<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <style>${realCss}</style>
    <style>
      /* 测试页精简：hljs/KaTeX 等无关样式不存在，补齐脚本依赖的最小布局 */
      #preview { font-size: 15px; }
      .zoom-popover { display: none; }
      .zoom-control.open .zoom-popover { display: flex; }
      .settings-popover { display: none; }
      .settings-control.open .settings-popover { display: flex; }
    </style>
  </head>
  <body class="has-tabs">
    <div class="tabbar" id="tabbar">
      <div class="tabs" id="tabs"></div>
      <div id="doc-stats"></div>
      <button id="tab-open"></button>
    </div>
    <aside class="sidebar" id="sidebar">
      <div class="sidebar-sections">
        <button type="button" data-sidebar-section="folder" aria-pressed="true"></button>
        <button type="button" data-sidebar-section="recent" aria-pressed="false"></button>
      </div>
      <div class="sidebar-list" id="sidebar-list"></div>
    </aside>
    <div id="topbar">
    <div class="toolbar sidebar-toggle">
      <button id="btn-sidebar"></button>
    </div>
    <div class="toolbar">
      <button id="btn-open"></button>
      <button id="btn-search"></button>
      <button id="btn-toggle"></button>
      <button id="btn-print"></button>
      <div id="zoom-control" class="zoom-control">
        <button id="btn-zoom"></button>
        <div class="zoom-popover">
          <button id="btn-zoom-out"></button>
          <button id="btn-zoom-reset" class="zoom-reset">100%</button>
          <button id="btn-zoom-in"></button>
        </div>
      </div>
      <div id="settings-control" class="settings-control">
        <button id="btn-settings"></button>
        <div class="settings-popover" role="group">
          <div class="settings-row">
            <span class="settings-label"></span>
            <div class="settings-seg">
              <button type="button" data-setting="tab-mode" data-value="accumulate"></button>
            </div>
          </div>
        </div>
      </div>
    </div>
    </div>
    <div class="findbar" role="search">
      <input id="find-input" type="search">
      <span id="find-state"></span>
      <button id="find-prev"></button>
      <button id="find-next"></button>
      <button id="find-close"></button>
    </div>
    <div id="app">
      <div id="preview"><p>Preview paragraph</p></div>
      <textarea id="editor" spellcheck="false">${editorText}</textarea>
    </div>
    <script>
      window.__messages = [];
      window.ipc = { postMessage(message) { window.__messages.push(message); } };
    </script>
    <script>${desktopScript}</script>
  </body>
</html>`);

await page.evaluate(() => window.__setTabs([{
  id: 1, name: 'doc.md', path: '/tmp/doc.md', active: true, missing: false, dirty: false,
}]));
await page.evaluate((source) => window.__setContent(
  '<p>Preview</p>',
  source,
  '',
  false,
  false,
), editorText);

// 进入编辑模式（工具栏 hover 显示，先悬停再点击）
await page.locator('body').hover();
await page.locator('#btn-toggle').click();
await page.waitForTimeout(100);

const layout = await page.evaluate(() => {
  const topbar = document.getElementById('topbar');
  const editor = document.getElementById('editor');
  const settingsBtn = document.getElementById('btn-settings');
  const sidebarBtn = document.getElementById('btn-sidebar');
  const topbarRect = topbar.getBoundingClientRect();
  const editorRect = editor.getBoundingClientRect();
  // textarea 内容行没有 DOM 节点，用镜像 div 同步布局后量第一行位置
  const mirror = document.createElement('div');
  const taStyle = getComputedStyle(editor);
  mirror.style.cssText = `position:absolute; visibility:hidden; left:${editorRect.left}px; top:${editorRect.top}px; width:${editorRect.width}px; box-sizing:${taStyle.boxSizing}; font:${taStyle.font}; white-space:pre; overflow:hidden;`;
  mirror.textContent = 'Editor source line 1';
  document.body.appendChild(mirror);
  const firstLineTop = mirror.getBoundingClientRect().top;
  mirror.remove();
  return {
    editing: document.body.classList.contains('editing'),
    topbarDisplay: getComputedStyle(topbar).display,
    topbarPosition: getComputedStyle(topbar).position,
    toolbarPosition: getComputedStyle(settingsBtn.closest('.toolbar')).position,
    topbarBottom: topbarRect.bottom,
    editorTop: editorRect.top,
    firstLineTop,
    overlapTopFirstLine: topbarRect.bottom > firstLineTop + 1,
    editorBelowTopbar: editorRect.top >= topbarRect.bottom - 1,
    sidebarBtnVisible: !!sidebarBtn.offsetParent,
    toolbarWidth: document.getElementById('btn-toggle').getBoundingClientRect().width,
  };
});

if (!layout.editing) throw new Error('did not enter editing mode');
if (layout.topbarDisplay !== 'flex') throw new Error(`topbar not flex: ${layout.topbarDisplay}`);
if (layout.topbarPosition !== 'sticky') throw new Error(`topbar not sticky: ${layout.topbarPosition}`);
if (layout.toolbarPosition !== 'static') {
  throw new Error(`toolbar still floating in edit mode: ${layout.toolbarPosition}`);
}
if (layout.overlapTopFirstLine) {
  throw new Error(`topbar covers first editor line: ${JSON.stringify(layout)}`);
}
if (!layout.editorBelowTopbar) {
  throw new Error(`editor starts above topbar bottom: ${JSON.stringify(layout)}`);
}
if (!layout.sidebarBtnVisible) {
  throw new Error('sidebar toggle hidden behind topbar in edit mode');
}

// 滚动后顶栏仍吸顶，正文第一行可滚入可视区且不被遮挡
await page.evaluate(() => scrollTo(0, 400));
await page.waitForTimeout(50);
const scrolled = await page.evaluate(() => {
  const topbarRect = document.getElementById('topbar').getBoundingClientRect();
  return {
    stickyStillTop: topbarRect.top <= 40,
    topbarBottom: topbarRect.bottom,
  };
});
if (!scrolled.stickyStillTop) {
  throw new Error(`topbar did not stay pinned on scroll: ${JSON.stringify(scrolled)}`);
}

// 退出编辑模式后顶栏消失，工具栏恢复悬浮
await page.evaluate(() => scrollTo(0, 0));
await page.locator('body').hover();
await page.locator('#btn-toggle').click();
await page.waitForTimeout(50);
const previewMode = await page.evaluate(() => {
  const topbar = document.getElementById('topbar');
  const toolbar = document.getElementById('btn-settings').closest('.toolbar');
  return {
    editing: document.body.classList.contains('editing'),
    topbarDisplay: getComputedStyle(topbar).display,
    toolbarPosition: getComputedStyle(toolbar).position,
  };
});
if (previewMode.editing) throw new Error('did not leave editing mode');
if (previewMode.topbarDisplay !== 'block') {
  throw new Error(`topbar visible in preview mode: ${JSON.stringify(previewMode)}`);
}
if (previewMode.toolbarPosition !== 'fixed') {
  throw new Error(`toolbar not floating in preview mode: ${previewMode.toolbarPosition}`);
}

await browser.close();
console.log('[desktop-edit-topbar-verify] OK');
