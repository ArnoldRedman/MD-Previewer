import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

import { configScript, desktopCss, desktopScript } from './desktop-page.mjs';

const pageConfig = configScript({
  btnEditJs: 'Edit',
  btnPreviewJs: 'Preview',
  sidebarEmptyJs: 'empty',
  statWordsJs: '字',
  statCharsJs: '字符',
});

// 字数统计在空闲时间分片算，先等状态栏落到期望值再断言
async function waitForStats(expected) {
  try {
    await page.waitForFunction(
      (want) => document.getElementById('doc-stats').textContent === want,
      expected,
      { timeout: 5000 },
    );
  } catch (error) {
    const actual = await page.locator('#doc-stats').textContent();
    throw new Error(`stats should be '${expected}', got '${actual}'`, { cause: error });
  }
}

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
await page.route('https://md-previewer.test/**', (route) => route.fulfill({
  contentType: 'text/html',
  body: '<!doctype html><title>MD Previewer test</title>',
}));
await page.goto('https://md-previewer.test/');
const previewBlocks = Array.from(
  { length: 100 },
  (_, index) => `<p>Preview paragraph ${index + 1}</p>`,
).join('');
const editorText = Array.from(
  { length: 280 },
  (_, index) => `Editor source line ${index + 1}`,
).join('\n');

await page.setContent(`<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <style>
      :root { --content-scale: 1; }
      body { margin: 0; font: 15px/1.6 system-ui, sans-serif; }
      #preview { font-size: calc(15px * var(--content-scale)); }
      #editor {
        display: none;
        width: 100%;
        box-sizing: border-box;
        overflow: hidden;
        resize: none;
        font: calc(14px * var(--content-scale))/1.6 monospace;
      }
      body.editing #preview { display: none; }
      body.editing #editor { display: block; }
      .zoom-popover { display: none; }
      .zoom-control.open .zoom-popover { display: flex; }
      .toolbar { position: fixed; top: 0; right: 0; z-index: 10; }
      .toolbar button { width: 34px; height: 34px; }
    </style>
  </head>
  <body class="has-tabs">
    <div class="tabbar">
      <div id="tabs"></div>
      <div id="doc-stats"></div>
      <button id="tab-open"></button>
    </div>
    <div class="toolbar">
      <button id="btn-open"></button>
      <button id="btn-search"></button>
      <button id="btn-remember" aria-pressed="false"></button>
      <button id="btn-toggle"></button>
      <button id="btn-print"></button>
      <div id="zoom-control" class="zoom-control">
        <button id="btn-zoom"></button>
        <div class="zoom-popover">
          <button id="btn-zoom-out"></button>
          <button id="btn-zoom-reset"></button>
          <button id="btn-zoom-in"></button>
        </div>
      </div>
      <button id="btn-update" hidden></button>
      <button id="btn-settings"></button>
      <div id="settings-control"></div>
    </div>
    <button id="btn-sidebar"></button>
    <aside id="sidebar"><div id="sidebar-list"></div></aside>
    <div class="findbar">
      <input id="find-input">
      <span id="find-state"></span>
      <button id="find-prev"></button>
      <button id="find-next"></button>
      <button id="find-close"></button>
    </div>
    <div id="app">
      <div id="preview">${previewBlocks}</div>
      <textarea id="editor">${editorText}</textarea>
    </div>
    <script>
      window.__messages = [];
      window.ipc = { postMessage(message) { window.__messages.push(message); } };
      window.__mdPreviewerFeatureFlags = { math: false, mermaid: false };
    </script>
    ${pageConfig}
    <script>${desktopScript}</script>
  </body>
</html>`);

await page.evaluate(() => window.__setContent(
  document.getElementById('preview').innerHTML,
  '你好 A\n',
  '',
  false,
  false,
));

await waitForStats('3 字 · 5 字符');
let result = await page.evaluate(() => ({
  scale: getComputedStyle(document.documentElement).getPropertyValue('--content-scale').trim(),
  toolbarWidth: document.getElementById('btn-open').getBoundingClientRect().width,
}));
if (result.scale !== '1' || result.toolbarWidth !== 34) {
  throw new Error(`initial reading tools failed: ${JSON.stringify(result)}`);
}

await page.locator('#btn-toggle').click();
await page.locator('#editor').fill('你 好');
await waitForStats('2 字 · 3 字符');
result = await page.evaluate(() => ({
  dirty: window.__messages.includes('dirty:1'),
}));
if (!result.dirty) {
  throw new Error(`live stats failed: ${JSON.stringify(result)}`);
}
await page.locator('#btn-toggle').click();

await page.locator('#btn-zoom').click();
await page.locator('#btn-zoom-in').click();
result = await page.evaluate(() => ({
  resetLabel: document.getElementById('btn-zoom-reset').textContent,
  scale: getComputedStyle(document.documentElement).getPropertyValue('--content-scale').trim(),
  stored: localStorage.getItem('md-previewer-content-zoom-v1'),
  toolbarWidth: document.getElementById('btn-open').getBoundingClientRect().width,
}));
if (result.resetLabel !== '110%' || result.scale !== '1.1' ||
    result.stored !== '110' || result.toolbarWidth !== 34) {
  throw new Error(`zoom in failed: ${JSON.stringify(result)}`);
}

const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';
await page.keyboard.press(`${modifier}+0`);
result = await page.evaluate(() => ({
  resetLabel: document.getElementById('btn-zoom-reset').textContent,
  scale: getComputedStyle(document.documentElement).getPropertyValue('--content-scale').trim(),
}));
if (result.resetLabel !== '100%' || result.scale !== '1') {
  throw new Error(`zoom reset failed: ${JSON.stringify(result)}`);
}

await page.evaluate((source) => {
  const preview = document.getElementById('preview');
  preview.innerHTML = Array.from(
    { length: 100 },
    (_, index) => `<p>Preview paragraph ${index + 1}</p>`,
  ).join('');
  document.getElementById('editor').value = source;
  const max = document.documentElement.scrollHeight - innerHeight;
  scrollTo(0, max * 0.5);
}, editorText);
const previewProgress = await page.evaluate(
  () => scrollY / (document.documentElement.scrollHeight - innerHeight),
);
await page.locator('#btn-toggle').click();
await page.waitForTimeout(100);
const editorState = await page.evaluate(() => ({
  progress: scrollY / (document.documentElement.scrollHeight - innerHeight),
  editing: document.body.classList.contains('editing'),
  scrollY,
  scrollHeight: document.documentElement.scrollHeight,
  editorHeight: document.getElementById('editor').getBoundingClientRect().height,
  editorScrollHeight: document.getElementById('editor').scrollHeight,
}));
const editorProgress = editorState.progress;
await page.locator('#btn-toggle').click();
await page.waitForTimeout(100);
const restoredPreviewProgress = await page.evaluate(
  () => scrollY / (document.documentElement.scrollHeight - innerHeight),
);

if (Math.abs(previewProgress - editorProgress) > 0.03 ||
    Math.abs(previewProgress - restoredPreviewProgress) > 0.03) {
  throw new Error(`scroll progress drifted: ${JSON.stringify({
    previewProgress,
    editorProgress,
    restoredPreviewProgress,
    editorState,
  })}`);
}

// 书签按钮：默认描边（从头打开），点一下把当前位置报给 Rust
await page.evaluate(() => {
  document.getElementById('preview').innerHTML = Array.from(
    { length: 100 },
    (_, index) => `<p>Preview paragraph ${index + 1}</p>`,
  ).join('');
  scrollTo(0, (document.documentElement.scrollHeight - innerHeight) * 0.5);
  window.__messages.length = 0;
});
let remember = await page.evaluate(() => ({
  on: document.getElementById('btn-remember').classList.contains('on'),
  pressed: document.getElementById('btn-remember').getAttribute('aria-pressed'),
  title: document.getElementById('btn-remember').title,
}));
if (remember.on || remember.pressed !== 'false' || remember.title !== 'Remember reading position') {
  throw new Error(`bookmark button should start off: ${JSON.stringify(remember)}`);
}

await page.locator('#btn-remember').click();
const rememberMsg = await page.evaluate(() => {
  const message = window.__messages[window.__messages.length - 1];
  return { message, ratio: scrollY / (document.documentElement.scrollHeight - innerHeight) };
});
if (!rememberMsg.message || !rememberMsg.message.startsWith('remember-position:1\n')) {
  throw new Error(`click should ask Rust to remember the position: ${rememberMsg.message}`);
}
const reportedRatio = Number(rememberMsg.message.split('\n')[1]);
if (!(Math.abs(reportedRatio - rememberMsg.ratio) < 0.01)) {
  throw new Error(`reported ratio ${reportedRatio} != ${rememberMsg.ratio}`);
}

// Rust 回推状态：高亮实心 + 提示语变成「不再记住」
await page.evaluate(() => window.__setRememberPosition(true));
remember = await page.evaluate(() => ({
  on: document.getElementById('btn-remember').classList.contains('on'),
  pressed: document.getElementById('btn-remember').getAttribute('aria-pressed'),
  title: document.getElementById('btn-remember').title,
}));
if (!remember.on || remember.pressed !== 'true' || remember.title !== 'Stop remembering reading position') {
  throw new Error(`bookmark button should be filled when remembered: ${JSON.stringify(remember)}`);
}
// 实心效果来自真实样式表，不是只在 JS 里换 class
if (!/#btn-remember\.on svg \{ fill: currentColor; \}/.test(desktopCss)) {
  throw new Error('desktop css should fill the bookmark icon once the position is remembered');
}
await page.evaluate(() => { window.__messages.length = 0; });
await page.locator('#btn-remember').click();
const forgetMsg = await page.evaluate(() => window.__messages[window.__messages.length - 1]);
if (!forgetMsg || !forgetMsg.startsWith('remember-position:0\n')) {
  throw new Error(`second click should stop remembering: ${forgetMsg}`);
}
await page.evaluate(() => window.__setRememberPosition(false));

// 已记住的文件按上次比例打开；没记住的（Rust 传 null 且先把滚动归零）停在开头
await page.evaluate((source) => {
  scrollTo(0, document.documentElement.scrollHeight);
  window.__setContent(source, 'doc', '', false, false, 0.5);
}, previewBlocks);
await page.waitForTimeout(200);
const restored = await page.evaluate(
  () => scrollY / (document.documentElement.scrollHeight - innerHeight),
);
if (Math.abs(restored - 0.5) > 0.02) {
  throw new Error(`remembered progress should be restored, got ${restored}`);
}
await page.evaluate((source) => {
  scrollTo(0, document.documentElement.scrollHeight);
  scrollTo(0, 0);
  window.__setContent(source, 'doc', '', false, false, null);
}, previewBlocks);
await page.waitForTimeout(200);
const started = await page.evaluate(() => Math.round(scrollY));
if (started !== 0) {
  throw new Error(`null progress must stay at the top, scrollY=${started}`);
}

// 滚动停下来后上报一次，滚动过程中不刷屏；上报内容带标签 id 和当时的比例
await page.evaluate((source) => {
  window.__messages.length = 0;
  // 用同一份高文档，保证确实能滚动
  window.__setContent(source, 'doc', '', false, false, null);
  window.__setTabs([
    { id: 31, name: 'doc.md', path: 'D:/docs/doc.md', active: true, dirty: false, missing: false },
  ]);
}, previewBlocks);
// 换文档后的归零/恢复期间不上报，等静默窗口过去再滚
await page.waitForTimeout(500);
const scrolled = await page.evaluate(() => {
  window.__messages.length = 0;
  const max = document.documentElement.scrollHeight - innerHeight;
  scrollTo(0, max * 0.25);
  return { expected: scrollY / (document.documentElement.scrollHeight - innerHeight), max: Math.round(max) };
});
await page.waitForTimeout(400);
const scrollReports = await page.evaluate(
  () => window.__messages.filter((message) => message.startsWith('reading-progress:')),
);
if (scrollReports.length !== 1) {
  throw new Error(`expected one debounced scroll report, got ${JSON.stringify(scrollReports)}`);
}
const [reportTabId, reportRatioValue] = scrollReports[0].replace('reading-progress:', '').split(':');
if (Number(reportTabId) !== 31) {
  throw new Error(`scroll report should carry the tab id: ${scrollReports[0]}`);
}
if (Math.abs(Number(reportRatioValue) - scrolled.expected) > 0.02) {
  const all = await page.evaluate(() => window.__messages.slice());
  throw new Error(
    `scroll report ratio should match the scrolled position: ${scrollReports[0]} vs ${scrolled.expected} :: ${JSON.stringify(all)}`,
  );
}

// 9. 换文档（真实流程：推送新内容 + 切标签）不能把上一份的位置上报成"读到顶部"
//    这正是把书签改成 0.0006 的元凶：Rust 以前在换文档前先 scrollTo(0,0)，
//    那个程序性归零被当成用户在读的位置记到了上一份文档头上
const switched = await page.evaluate(async (source) => {
  window.__messages.length = 0;
  window.__setTabs([
    { id: 41, name: 'aaa.md', path: 'D:/docs/aaa.md', active: true, dirty: false, missing: false },
    { id: 42, name: 'bbb.md', path: 'D:/docs/bbb.md', active: false, dirty: false, missing: false },
  ]);
  window.__setContent(source, 'doc', '', false, false, null);
  await new Promise((resolve) => setTimeout(resolve, 500)); // 等换文档的静默窗口过去
  // 读者滚到 40%，然后在同一个任务里切走（应用里切标签就是这样：内容与标签一起换）
  scrollTo(0, (document.documentElement.scrollHeight - innerHeight) * 0.4);
  window.__setContent(source, 'doc', '', false, false, null);
  window.__setTabs([
    { id: 41, name: 'aaa.md', path: 'D:/docs/aaa.md', active: false, dirty: false, missing: false },
    { id: 42, name: 'bbb.md', path: 'D:/docs/bbb.md', active: true, dirty: false, missing: false },
  ]);
  await new Promise((resolve) => setTimeout(resolve, 700));
  return window.__messages.filter((message) => message.startsWith('reading-progress:'));
}, previewBlocks);
if (switched.length) {
  throw new Error(
    `switching documents must not report a position for the outgoing tab: ${JSON.stringify(switched)}`,
  );
}

// 10. 静默窗口过去之后，新文档自己的滚动仍然要带上它自己的标签
const afterSwitch = await page.evaluate(async () => {
  window.__messages.length = 0;
  scrollTo(0, (document.documentElement.scrollHeight - innerHeight) * 0.6);
  await new Promise((resolve) => setTimeout(resolve, 500));
  return window.__messages.filter((message) => message.startsWith('reading-progress:'));
});
if (afterSwitch.length !== 1 || !afterSwitch[0].startsWith('reading-progress:42:')) {
  throw new Error(`after a switch the new tab must report itself: ${JSON.stringify(afterSwitch)}`);
}

await browser.close();
console.log('[desktop-reading-tools-verify] OK');
