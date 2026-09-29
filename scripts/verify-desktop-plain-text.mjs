// 纯文本（配置 / 日志 / 源码）预览的折行宽度：
// .mdp-plain-text 这类行导向内容不能被 Markdown 的正文栏 820px 限宽，
// 否则窗口再宽也会把一条记录折成两行，两侧留大片空白。
import { createRequire } from 'node:module';

import { configScript, desktopScript, desktopStyle } from './desktop-page.mjs';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

const MARKDOWN_COLUMN_MAX = 820;
const APP_PADDING = 48;

const iniLines = [
  'GameplayTagList=(Tag="Level.YMB.EventTag.STK.Xunluo2Die",DevComment="巡逻兵2死亡")',
  'GameplayTagList=(Tag="Level.YMB.OverCrowd.BLJ.Left_01",DevComment="左翼八路军人群01")',
  'GameplayTagList=(Tag="Level.YMB.RUSH.CN_Shoot_LeftHill",DevComment="八路军走到指定地点后，射击左边山坡上的敌人")',
  'GameplayTagList=(Tag="Level.YMB.RUSH.JP_BeShot_A",DevComment="阳明堡冲锋时，被射死的鬼子，纯表演死亡")',
];

function escapeHtml(text) {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

const plainPreview = `<div class="mdp-plain-text">${escapeHtml(iniLines.join('\n'))}</div>`;
const markdownPreview = `<h1>标题</h1><p>${'正文内容 '.repeat(60)}</p>`;

async function buildPage(browser, viewport, html, extraClass) {
  const page = await browser.newPage({ viewport });
  await page.setContent(`<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    ${desktopStyle}
  </head>
  <body class="has-tabs${extraClass ? ` ${extraClass}` : ''}">
    <div class="tabbar">
      <div id="tabs"></div>
      <div id="doc-stats"></div>
      <button id="tab-open"></button>
    </div>
    <div id="topbar">
      <div class="toolbar sidebar-toggle">
        <button id="btn-sidebar"></button>
      </div>
      <div class="toolbar">
        <button id="btn-open"></button>
      <button id="btn-search"></button>
      <button id="btn-toggle"></button>
      <button id="btn-print"></button>
      <button id="btn-split"></button>
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
    </div>
    <aside id="sidebar"><div id="sidebar-list"></div></aside>
    <div class="findbar">
      <input id="find-input">
      <span id="find-state"></span>
      <button id="find-prev"></button>
      <button id="find-next"></button>
      <button id="find-close"></button>
    </div>
    <div id="app">
      <div id="preview">${html}</div>
      <textarea id="editor"></textarea>
    </div>
    <script>
      window.__messages = [];
      window.ipc = { postMessage(message) { window.__messages.push(message); } };
      window.__mdPreviewerFeatureFlags = { math: false, mermaid: false };
    </script>
    ${configScript({})}
    <script>${desktopScript}</script>
  </body>
</html>`);
  await page.evaluate(
    (previewHtml) => window.__setContent(previewHtml, 'raw', '', false, false),
    html,
  );
  return page;
}

function measure(page) {
  return page.evaluate(() => {
    const app = document.getElementById('app');
    const plain = document.querySelector('#preview .mdp-plain-text');
    const style = plain ? getComputedStyle(plain) : null;
    const topbar = document.getElementById('topbar');
    const sidebarBtn = document.getElementById('btn-sidebar');
    const topbarRect = topbar.getBoundingClientRect();    return {
      appWidth: Math.round(app.getBoundingClientRect().width),
      viewport: document.documentElement.clientWidth,
      overflowX: document.documentElement.scrollWidth - document.documentElement.clientWidth,
      plainRows: plain
        ? Math.round(plain.getBoundingClientRect().height / parseFloat(style.lineHeight))
        : null,
      plainWidth: plain ? Math.round(plain.getBoundingClientRect().width) : null,
      // 控件行占位：正文第一行必须在顶栏下方，左侧侧栏按钮也不能压在正文上（正文字形起点）
      topbarHeight: Math.round(topbarRect.height),
      topbarBottom: Math.round(topbarRect.bottom),
      sidebarBtnBottom: Math.round(sidebarBtn.getBoundingClientRect().bottom),
      firstGlyphTop: plain ? Math.round(plain.getBoundingClientRect().top) : null,
    };
  });
}

const browser = await chromium.launch();
const failures = [];
const check = (label, ok, detail) => {
  console.log(`${ok ? '  ok  ' : ' FAIL '} ${label}${detail ? ` — ${detail}` : ''}`);
  if (!ok) failures.push(label);
};

try {
  // 1. 宽窗口：纯文本正文用满可用宽度，一条 ini 记录不折行（1030 是反馈截图里的窗口宽度）
  for (const width of [1030, 1440, 2550]) {
    const page = await buildPage(browser, { width, height: 800 }, plainPreview, '');
    const m = await measure(page);
    check(
      `${width}px 纯文本正文不受 820 正文栏限宽`,
      m.appWidth > MARKDOWN_COLUMN_MAX + APP_PADDING + 100,
      `#app=${m.appWidth}px 正文=${m.plainWidth}px`,
    );
    check(
      `${width}px 每行配置各占一个视觉行（长行不折）`,
      m.plainRows === iniLines.length,
      `视觉行数=${m.plainRows}，源码行数=${iniLines.length}`,
    );
    check(`${width}px 不产生横向溢出`, m.overflowX <= 0, `overflow=${m.overflowX}px`);
    check(
      `${width}px 控件行单独占位，侧栏按钮不压正文`,
      m.topbarHeight >= 40 && m.firstGlyphTop > m.topbarBottom && m.firstGlyphTop > m.sidebarBtnBottom,
      `顶栏 ${m.topbarHeight}px 底 ${m.topbarBottom}，正文顶 ${m.firstGlyphTop}，按钮底 ${m.sidebarBtnBottom}`,
    );
    await page.close();
  }

  // 2. 侧栏展开时仍用满右侧剩余宽度，且不顶出窗口右边缘
  {
    const page = await buildPage(browser, { width: 1440, height: 800 }, plainPreview, 'sidebar-open');
    const m = await measure(page);
    check(
      '1440px 侧栏展开时纯文本仍吃满剩余宽度',
      m.appWidth > 1000,
      `#app=${m.appWidth}px 视口=${m.viewport}`,
    );
    check('1440px 侧栏展开时不横向溢出', m.overflowX <= 0, `overflow=${m.overflowX}px`);
    check(
      '1440px 侧栏展开时控件行仍不压正文',
      m.firstGlyphTop > m.topbarBottom && m.firstGlyphTop > m.sidebarBtnBottom,
      `顶栏底 ${m.topbarBottom}，按钮底 ${m.sidebarBtnBottom}，正文顶 ${m.firstGlyphTop}`,
    );
    await page.close();
  }

  // 3. Markdown 文档保持原来的 820px 阅读栏（这次改动不能连带放开）
  {
    const page = await buildPage(browser, { width: 1440, height: 800 }, markdownPreview, '');
    const m = await measure(page);
    check(
      'Markdown 文档仍保持 820px 正文栏',
      m.appWidth === MARKDOWN_COLUMN_MAX + APP_PADDING,
      `#app=${m.appWidth}px`,
    );
    await page.close();
  }

  // 4. 窄窗口：真的放不下时仍然折行（不能把长行顶出可视范围）
  {
    const page = await buildPage(browser, { width: 700, height: 800 }, plainPreview, '');
    const m = await measure(page);
    check('700px 纯文本放不下时长行折行', m.plainRows > iniLines.length, `视觉行数=${m.plainRows}`);
    check('700px 不产生横向溢出', m.overflowX <= 0, `overflow=${m.overflowX}px`);
    await page.close();
  }
} finally {
  await browser.close();
}

if (failures.length) {
  throw new Error(`plain text width checks failed: ${failures.join(', ')}`);
}
console.log('[desktop-plain-text-verify] OK');
