// 标签栏验收：标签宽度固定不随数量收缩，放不下就横向滚动，鼠标纵向滚轮在标签栏上转成横向
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

const { configScript, desktopScript, desktopCss } = await import('./desktop-page.mjs');

const pageConfig = configScript({
  btnEditJs: 'Edit',
  btnPreviewJs: 'Preview',
  sidebarEmptyJs: 'empty',
  statWordsJs: '字',
  statCharsJs: '字符',
});

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1000, height: 700 } });
await page.setContent(`<!doctype html>
<html>
<head><meta charset="utf-8">
<style>:root { --sidebar-width: 260px; }</style>
<style>${desktopCss}</style>
</head>
<body class="has-tabs">
  <div class="tabbar"><div class="tabs" id="tabs"></div><div id="doc-stats"></div><button id="tab-open"></button></div>
  <div class="toolbar">
    <button id="btn-open"></button><button id="btn-search"></button><button id="btn-remember"></button>
    <button id="btn-toggle"></button><button id="btn-split"></button><button id="btn-print"></button>
    <div class="zoom-control" id="zoom-control"><button id="btn-zoom"></button>
      <div class="zoom-popover"><button id="btn-zoom-out"></button><button id="btn-zoom-reset"></button><button id="btn-zoom-in"></button></div>
    </div>
    <button id="btn-update" hidden></button><button id="btn-settings"></button><div id="settings-control"></div>
  </div>
  <button id="btn-sidebar"></button>
  <aside id="sidebar"><div id="sidebar-list"></div></aside>
  <div class="findbar"><input id="find-input"><span id="find-state"></span><button id="find-prev"></button><button id="find-next"></button><button id="find-close"></button></div>
  <div id="app"><div id="preview"></div><textarea id="editor"></textarea></div>
  <script>
    window.__messages = [];
    window.ipc = { postMessage(message) { window.__messages.push(message); } };
    window.__mdPreviewerFeatureFlags = { math: false, mermaid: false };
  </script>
  ${pageConfig}
  <script>${desktopScript}</script>
</body>
</html>`);

function tabsFor(count) {
  return Array.from({ length: count }, (_, index) => ({
    id: index + 1,
    name: `文件 ${index + 1}.md`,
    path: `D:\\docs\\文件 ${index + 1}.md`,
    active: index === 0,
    dirty: false,
    missing: false,
  }));
}

const readBar = () =>
  page.evaluate(() => {
    const tabsEl = document.getElementById('tabs');
    const widths = Array.from(document.querySelectorAll('.tab')).map((tab) => Math.round(tab.getBoundingClientRect().width));
    return {
      widths,
      uniqueWidths: Array.from(new Set(widths)),
      scrollLeft: Math.round(tabsEl.scrollLeft),
      scrollWidth: Math.round(tabsEl.scrollWidth),
      clientWidth: Math.round(tabsEl.clientWidth),
      overflows: tabsEl.scrollWidth > tabsEl.clientWidth,
    };
  });

// 1. 三个标签：宽度固定，不因为没有占满就撑开或收缩
await page.evaluate((tabs) => window.__setTabs(tabs), tabsFor(3));
const three = await readBar();
if (three.uniqueWidths.length !== 1 || three.uniqueWidths[0] !== 180) {
  throw new Error(`tabs should keep a fixed 180px width: ${JSON.stringify(three.widths)}`);
}
if (three.overflows) throw new Error('three tabs should fit without scrolling');

// 2. 十二个标签：宽度仍然是 180，不再越挤越小，而是出现横向溢出
await page.evaluate((tabs) => window.__setTabs(tabs), tabsFor(12));
const many = await readBar();
if (many.uniqueWidths.length !== 1 || many.uniqueWidths[0] !== 180) {
  throw new Error(`tabs must not shrink as more open: ${JSON.stringify(many.uniqueWidths)}`);
}
if (!many.overflows) throw new Error('twelve tabs must overflow horizontally');
if (many.scrollLeft !== 0) throw new Error(`first tab should stay in view: scrollLeft=${many.scrollLeft}`);

// 3. 鼠标纵向滚轮 = 横向滚动
const wheel = await page.evaluate(() => {
  const tabsEl = document.getElementById('tabs');
  const before = tabsEl.scrollLeft;
  tabsEl.dispatchEvent(new WheelEvent('wheel', { deltaY: 240, bubbles: true, cancelable: true }));
  const afterDown = tabsEl.scrollLeft;
  tabsEl.dispatchEvent(new WheelEvent('wheel', { deltaY: -120, bubbles: true, cancelable: true }));
  const afterUp = tabsEl.scrollLeft;
  // 横向滚轮（触控板）也要能用
  tabsEl.dispatchEvent(new WheelEvent('wheel', { deltaX: 60, deltaY: 0, bubbles: true, cancelable: true }));
  return { before, afterDown, afterUp, afterDX: tabsEl.scrollLeft };
});
if (wheel.afterDown !== wheel.before + 240) {
  throw new Error(`vertical wheel should scroll tabs horizontally: ${JSON.stringify(wheel)}`);
}
if (wheel.afterUp !== wheel.afterDown - 120) {
  throw new Error(`wheel up should scroll back: ${JSON.stringify(wheel)}`);
}
if (wheel.afterDX !== wheel.afterUp + 60) {
  throw new Error(`horizontal wheel should work too: ${JSON.stringify(wheel)}`);
}

// 4. 没有溢出时滚轮不抢事件（正文还能正常滚）
await page.evaluate((tabs) => window.__setTabs(tabs), tabsFor(2));
const noOverflow = await page.evaluate(() => {
  const tabsEl = document.getElementById('tabs');
  const event = new WheelEvent('wheel', { deltaY: 240, bubbles: true, cancelable: true });
  tabsEl.dispatchEvent(event);
  return { scrollLeft: Math.round(tabsEl.scrollLeft), prevented: event.defaultPrevented };
});
if (noOverflow.scrollLeft !== 0 || noOverflow.prevented) {
  throw new Error(`wheel without overflow must pass through: ${JSON.stringify(noOverflow)}`);
}

// 5. 切到最后一个标签时，要把它滚进可视区
const activated = await page.evaluate(async (tabs) => {
  const last = tabs[tabs.length - 1];
  window.__setTabs(tabs.map((tab) => ({ ...tab, active: tab.id === last.id })));
  await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  const tabsEl = document.getElementById('tabs');
  const active = document.querySelector('.tab.active').getBoundingClientRect();
  const bar = tabsEl.getBoundingClientRect();
  return {
    scrollLeft: Math.round(tabsEl.scrollLeft),
    activeLeft: Math.round(active.left - bar.left),
    activeRight: Math.round(active.right - bar.left),
    clientWidth: Math.round(bar.width),
  };
}, tabsFor(12));
if (activated.scrollLeft <= 0) {
  throw new Error(`activating a late tab should scroll the bar: ${JSON.stringify(activated)}`);
}
if (activated.activeLeft < -1 || activated.activeRight > activated.clientWidth + 1) {
  throw new Error(`the active tab should be inside the visible area: ${JSON.stringify(activated)}`);
}

console.log(
  `[desktop-tabbar-scroll-verify] OK width ${many.uniqueWidths[0]}px, overflow ${many.scrollWidth}>${many.clientWidth}, ` +
    `wheel ${wheel.before}->${wheel.afterDown}->${wheel.afterUp}->${wheel.afterDX}`,
);
await browser.close();
