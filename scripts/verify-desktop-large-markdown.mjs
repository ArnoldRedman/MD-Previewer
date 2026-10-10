// 大 Markdown 的懒布局验收：Rust 侧把顶层块按组包进 .mdp-md-chunk（超过 256KB 才包），
// 页面用 content-visibility 跳过屏外组的布局。这里用带包装层的 HTML 复现那个结构，
// 断言两件事：布局开销确实降下来了，以及包装层没有打破搜索/目录/作者模式取正文
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

const { configScript, desktopScript, desktopCss } = await import('./desktop-page.mjs');

const GROUP = 100;
const SECTIONS = 420;

// 每节的结构和 scripts/verify-desktop-tables.mjs 里的表格式样一致，保证表格插件也会被走到
function section(n) {
  return [
    `<h2 id="section-${n}">第 ${n} 节</h2>`,
    `<p>第 ${n} 节的正文段落，里面有 <strong>粗体</strong> 和 <code>行内代码</code>，用来撑出真实的块高。</p>`,
    '<ul><li>列表项一</li><li>列表项二</li><li>列表项三</li></ul>',
    '<blockquote><p>引用块</p></blockquote>',
    '<pre><code class="language-rust">fn demo() -&gt; usize { 1 }</code></pre>',
    '<table><thead><tr><th>列A</th><th>列B</th></tr></thead><tbody><tr><td>1</td><td>2</td></tr></tbody></table>',
  ];
}

const blocks = [];
for (let n = 1; n <= SECTIONS; n++) blocks.push(...section(n));
const flatHtml = blocks.join('\n');

// 按 GROUP 个块一组包装，复现 Rust 侧 push_chunked_html 的结构
const chunks = [];
for (let i = 0; i < blocks.length; i += GROUP) {
  chunks.push(`<div class="mdp-md-chunk">${blocks.slice(i, i + GROUP).join('\n')}</div>`);
}
const chunkedHtml = chunks.join('\n');

const pageConfig = configScript({
  btnEditJs: 'Edit',
  btnPreviewJs: 'Preview',
  sidebarEmptyJs: 'empty',
  statWordsJs: '字',
  statCharsJs: '字符',
});

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1200, height: 800 } });
await page.setContent(`<!doctype html>
<html>
<head><meta charset="utf-8">
<style>:root { --sidebar-width: 260px; }</style>
<style>${desktopCss}</style>
</head>
<body class="has-tabs">
  <div class="tabbar"><div id="tabs"></div><div id="doc-stats"></div><button id="tab-open"></button></div>
  <div class="toolbar">
    <button id="btn-open"></button><button id="btn-search"></button><button id="btn-remember"></button>
    <button id="btn-toggle"></button><button id="btn-split"></button><button id="btn-print"></button>
    <div class="zoom-control" id="zoom-control"><button id="btn-zoom"></button>
      <div class="zoom-popover"><button id="btn-zoom-out"></button><button id="btn-zoom-reset"></button><button id="btn-zoom-in"></button></div>
    </div>
    <button id="btn-update" hidden></button><button id="btn-settings"></button><div id="settings-control"></div>
  </div>
  <button id="btn-sidebar"></button>
  <aside id="sidebar">
    <button type="button" data-sidebar-section="outline" aria-pressed="false">outline</button>
    <div class="sidebar-list" id="sidebar-list"></div>
  </aside>
  <div class="findbar"><input id="find-input"><span id="find-state"></span><button id="find-prev"></button><button id="find-next"></button><button id="find-close"></button></div>
  <div id="app">
    <div id="doc-notice" class="doc-notice" style="display:none;">
      <span id="doc-notice-text">This file is very large</span>
      <button type="button" id="doc-notice-action">Render as Markdown anyway</button>
    </div>
    <div id="preview"></div><textarea id="editor"></textarea>
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

// 1. 同一份内容先按平铺渲染一次，作为高度与布局开销的基线
const baseline = await page.evaluate(async (html) => {
  window.__setContent(html, 'flat', '', false, false, null);
  const start = performance.now();
  const height = document.documentElement.scrollHeight;
  return { height, layoutMs: performance.now() - start, textLength: document.getElementById('preview').textContent.length };
}, flatHtml);

// 2. 再按分组渲染：布局应当显著变快，高度不能偏太多
const chunked = await page.evaluate(async (html) => {
  window.__setContent(html, 'chunked', '', false, false, null);
  const preview = document.getElementById('preview');
  const start = performance.now();
  const heightBeforeCalibration = document.documentElement.scrollHeight;
  const layoutMs = performance.now() - start;
  // 校准在第二帧写入 --md-chunk-height
  await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  return {
    heightBeforeCalibration,
    layoutMs,
    height: document.documentElement.scrollHeight,
    chunkVar: preview.style.getPropertyValue('--md-chunk-height'),
    chunks: preview.querySelectorAll('.mdp-md-chunk').length,
    textLength: preview.textContent.length,
  };
}, chunkedHtml);

if (chunked.chunks < 2) throw new Error(`expected grouped blocks, got ${chunked.chunks} groups`);
if (chunked.layoutMs * 5 > baseline.layoutMs) {
  throw new Error(
    `chunked layout should be much cheaper: ${chunked.layoutMs.toFixed(0)}ms vs ${baseline.layoutMs.toFixed(0)}ms`,
  );
}
if (!chunked.chunkVar) throw new Error('chunk height estimate was never calibrated');
if (chunked.textLength !== baseline.textLength) {
  throw new Error(`text changed after grouping: ${chunked.textLength} vs ${baseline.textLength}`);
}
// 预估高度直接决定滚动条与阅读位置比例，偏超过 10% 就会让"记住阅读位置"落不准
const drift = Math.abs(chunked.height - baseline.height) / baseline.height;
if (drift > 0.1) {
  throw new Error(
    `chunk height estimate drifts ${(drift * 100).toFixed(1)}%: ${chunked.height} vs ${baseline.height}`,
  );
}

// 3. 目录：标题在包装层里也要能被扫到，点一下要滚过去（第 300 节在很后面）
await page.evaluate(() => window.__setSidebar({ folder: [], recent: [] }));
await page.locator('[data-sidebar-section="outline"]').click();
await page.waitForTimeout(150);
const outline = await page.evaluate(() => {
  const items = Array.from(document.querySelectorAll('.outline-item'));
  return { count: items.length, hasLate: items.some((item) => item.textContent.includes('第 300 节')) };
});
if (outline.count !== SECTIONS || !outline.hasLate) {
  throw new Error(`outline should list every heading, got ${JSON.stringify(outline)}`);
}
const jumped = await page.evaluate(async () => {
  const target = Array.from(document.querySelectorAll('.outline-item')).find((item) =>
    item.textContent.includes('第 300 节'),
  );
  const started = performance.now();
  target.click();
  // 大文档禁用平滑滚动（平滑在懒布局里会边滚边重排、几秒都落不下来），跳一帧就该到位
  const heading = document.getElementById('section-300');
  let top = Math.round(heading.getBoundingClientRect().top);
  while (Math.abs(top) > 300 && performance.now() - started < 2000) {
    await new Promise((resolve) => requestAnimationFrame(resolve));
    top = Math.round(heading.getBoundingClientRect().top);
  }
  return { settleMs: Math.round(performance.now() - started), scrollY: Math.round(scrollY), headingTop: top };
});
if (Math.abs(jumped.headingTop) > 300) {
  throw new Error(`outline jump should land on the heading: ${JSON.stringify(jumped)}`);
}

// 4. 搜索：命中远处的组之后要能跳过去并高亮（屏外的组此时还没排过版）
const found = await page.evaluate(async () => {
  const input = document.getElementById('find-input');
  input.value = '第 400 节';
  window.__mdPreviewerShowFind();
  input.dispatchEvent(new Event('input', { bubbles: true }));
  await new Promise((resolve) => setTimeout(resolve, 1500));
  const mark = document.querySelector('mark.search-hit');
  if (!mark) return { state: document.getElementById('find-state').textContent, marks: 0 };
  mark.scrollIntoView({ block: 'center' });
  await new Promise((resolve) => requestAnimationFrame(resolve));
  return {
    state: document.getElementById('find-state').textContent,
    marks: document.querySelectorAll('mark.search-hit').length,
    visibleRects: mark.getClientRects().length,
    markTop: Math.round(mark.getBoundingClientRect().top),
  };
});
if (!found.marks || !found.visibleRects) {
  throw new Error(`find should highlight a late hit inside a skipped group: ${JSON.stringify(found)}`);
}
if (Math.abs(found.markTop) > 400) {
  throw new Error(`find should scroll the hit into view: ${JSON.stringify(found)}`);
}

// 5. 作者模式取正文：包装层不能让"复制正文"变空或漏掉后面的章节
const authorCopy = await page.evaluate(async () => {
  // 页面是 with_html 载入的，不是安全上下文，剪贴板走 execCommand 退路：拦下它拿文本
  window.__copied = '';
  document.execCommand = function () {
    const holder = document.querySelector('textarea[readonly]');
    window.__copied = holder ? holder.value : '';
    return true;
  };
  window.__setAuthorDoc({ titleLine: '# 大文档测试', title: '大文档测试' });
  window.__setSettings({ authorMode: true });
  await new Promise((resolve) => setTimeout(resolve, 50));
  const button = document.querySelector('.author-body-actions [data-author-copy]');
  if (button) button.click();
  await new Promise((resolve) => setTimeout(resolve, 100));
  return { hasButton: !!button, text: window.__copied };
});
if (!authorCopy.hasButton) throw new Error('author body copy button should exist for a grouped document');
if (!authorCopy.text || !authorCopy.text.includes('第 400 节的正文')) {
  throw new Error(`author body copy should include late sections, got ${authorCopy.text.length} chars`);
}
if (!authorCopy.text.startsWith('第 1 节的正文')) {
  throw new Error(`author body copy should start at the first block: ${JSON.stringify(authorCopy.text.slice(0, 40))}`);
}

// 6. 超大 Markdown 降级提示条：__setContent 第七个参数为 true 时显示，按钮上报出口消息
const notice = await page.evaluate(async () => {
  const el = document.getElementById('doc-notice');
  const action = document.getElementById('doc-notice-action');
  // 降级渲染：第七个参数为 true
  window.__setContent('<div class="mdp-plain-text">大文件</div>', 'x', '', false, false, null, true);
  const shown = el.style.display !== 'none';
  window.__messages.length = 0;
  action.click();
  await new Promise((resolve) => setTimeout(resolve, 50));
  return { shown, messages: window.__messages.slice() };
});
if (!notice.shown) throw new Error('degraded documents must show the notice');
if (!notice.messages.includes('render-markdown-anyway')) {
  throw new Error(`notice action should ask Rust to render markdown: ${JSON.stringify(notice.messages)}`);
}
const afterNormal = await page.evaluate(() => {
  window.__setContent('<p>普通文档</p>', 'x', '', false, false, null, false);
  return document.getElementById('doc-notice').style.display === 'none';
});
if (!afterNormal) throw new Error('the notice must hide for documents that are not degraded');
const afterDegraded = await page.evaluate(() => {
  window.__setContent('<div class="mdp-plain-text">大文件</div>', 'x', '', false, false, null, true);
  return document.getElementById('doc-notice').style.display !== 'none';
});
if (!afterDegraded) throw new Error('the notice must show again for a degraded document');

console.log(
  `[desktop-large-markdown-verify] OK layout ${baseline.layoutMs.toFixed(0)}ms -> ${chunked.layoutMs.toFixed(0)}ms, ` +
    `height drift ${(drift * 100).toFixed(1)}%, groups ${chunked.chunks}, outline ${outline.count} (jump ${jumped.settleMs}ms), ` +
    `find ${found.state}, author body ${authorCopy.text.length} chars`,
);
await browser.close();
