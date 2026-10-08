// 取词与侧栏宽度两个交互的回归检查：
// 1. 双击或自己拖拽选中一段短文本，要把同一个词的其它出现位置标出来（Notepad++ 的 smart highlight），
//    自己的选区保持原生选中色，且任何一次标记都不能改坏正文；
// 2. 侧栏右缘可以拖着改宽度，松手落盘，越界值被夹回可用区间；
// 3. 侧栏压到最窄时，三个分区标签必须还在自己的框内（高度写死 28px 会让两行文字溢出）；
// 4. 选中底色（::selection）在两个主题下都显式定义，不依赖 WebView 默认值。
import { createRequire } from 'node:module';

import { configScript, desktopCss, desktopScript, desktopStyle } from './desktop-page.mjs';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

const SIDEBAR_MIN_W = 180;
const SIDEBAR_MAX_W = 560;
const DEFAULT_SIDEBAR_W = 260;

// 正文里刻意放了 4 处 beta：段落文本 3 处 + 代码块 1 处；svg 和公式里的不能碰
const PREVIEW_HTML = [
  '<p id="line1">beta <span id="probe">beta</span> beta</p>',
  '<p id="line2"><code id="codeprobe">beta()</code> tail</p>',
  '<p id="zh">中文取词高亮到此为止，这里是 <span id="zhprobe">取词高亮</span></p>',
  '<svg><text id="svgtext">beta</text></svg>',
  '<div class="katex"><span class="katex-html" id="katextext">beta</span></div>',
].join('');
const EXPECTED_WORD_HITS = 4;
const EXPECTED_ZH_HITS = 2;
// 三个分区标签取自 i18n 里最长的一组（中文），这里只验证框装不装得下文字
const SIDEBAR_SECTIONS = [
  ['folder', '当前文件夹'],
  ['recent', '最近打开'],
  ['outline', '大纲'],
];

async function buildPage(browser) {
  const page = await browser.newPage({ viewport: { width: 900, height: 700 } });
  await page.setContent(`<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    ${desktopStyle}
  </head>
  <body class="has-tabs sidebar-open">
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
    <aside class="sidebar" id="sidebar">
      <div class="sidebar-sections">
        ${SIDEBAR_SECTIONS.map(
          ([section, label], index) =>
            `<button type="button" data-sidebar-section="${section}" aria-pressed="${index === 0}">${label}</button>`,
        ).join('')}
      </div>
      <div id="sidebar-list"></div>
      <div class="sidebar-footer" id="sidebar-footer" style="display:none;"></div>
      <div class="sidebar-resizer" id="sidebar-resizer" role="separator"></div>
    </aside>
    <div class="findbar">
      <input id="find-input">
      <span id="find-state"></span>
      <button id="find-prev"></button>
      <button id="find-next"></button>
      <button id="find-close"></button>
    </div>
    <div id="app">
      <div id="preview">${PREVIEW_HTML}</div>
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
  await page.evaluate(() => window.__setSettings({ sidebarOpen: true, sidebarWidth: 260 }));
  return page;
}

const browser = await chromium.launch();
const failures = [];
const check = (label, ok, detail) => {
  console.log(`${ok ? '  ok  ' : ' FAIL '} ${label}${detail ? ` — ${detail}` : ''}`);
  if (!ok) failures.push(label);
};

const sidebarWidth = (page) =>
  page.evaluate(() =>
    Math.round(document.getElementById('sidebar').getBoundingClientRect().width),
  );

try {
  // 1. 选中底色：两个主题都要显式给背景色，不能只靠 WebView 默认值
  for (const [label, pattern] of [
    ['浅色', /::selection\s*{[^}]*background:\s*#[0-9a-f]{3,8}/i],
    ['深色', /@media\s*\(prefers-color-scheme:\s*dark\)[\s\S]*::selection\s*{[^}]*background:\s*#[0-9a-f]{3,8}/i],
  ]) {
    check(`${label}主题定义了 ::selection 底色`, pattern.test(desktopCss));
  }

  // 2. 双击取词：同一个词的其它出现位置都被标出来，svg / 公式和正文文本不进标记
  {
    const page = await buildPage(browser);
    const beforeHtml = await page.evaluate(() => document.getElementById('preview').innerHTML);
    const beforeText = await page.evaluate(() => document.getElementById('preview').textContent);

    await page.dblclick('#probe');
    const marked = await page.evaluate(() => ({
      hits: document.querySelectorAll('#preview mark.mdp-word-hit').length,
      texts: Array.from(document.querySelectorAll('#preview mark.mdp-word-hit')).map((m) => m.textContent),
      inSvg: !!document.querySelector('#preview svg mark.mdp-word-hit'),
      inKatex: !!document.querySelector('#preview .katex mark.mdp-word-hit'),
      text: document.getElementById('preview').textContent,
      selection: String(window.getSelection()),
    }));
    check(
      '双击后同一个词的 4 处出现全部标出',
      marked.hits === EXPECTED_WORD_HITS,
      `命中=${marked.hits} ${JSON.stringify(marked.texts)}`,
    );
    check(
      '每处标记都是被双击的那个词',
      marked.texts.every((text) => text === 'beta'),
      JSON.stringify(marked.texts),
    );
    check('svg 与公式内部不改写', !marked.inSvg && !marked.inKatex);
    check('正文文本不因标记增删', marked.text === beforeText);
    check('双击选中的原选区保持不变', marked.selection === 'beta', JSON.stringify(marked.selection));

    // Esc 清掉标记，并把 DOM 还原成双击前的样子
    await page.keyboard.press('Escape');
    const cleared = await page.evaluate(() => ({
      hits: document.querySelectorAll('#preview mark.mdp-word-hit').length,
      html: document.getElementById('preview').innerHTML,
    }));
    check('Esc 清掉全部取词标记', cleared.hits === 0, `剩余=${cleared.hits}`);
    check('清除后正文 HTML 与双击前一致', cleared.html === beforeHtml);

    // 点别处同样清除，再双击另一个词时只保留新的
    await page.dblclick('#probe');
    await page.mouse.click(20, 600);
    const afterClick = await page.evaluate(
      () => document.querySelectorAll('#preview mark.mdp-word-hit').length,
    );
    check('点击别处清除取词标记', afterClick === 0, `剩余=${afterClick}`);
    await page.close();
  }

  // 2b. 自己拖拽划选一段短文本（三四个字）也要取词，且不抢掉自己的原生选中
  {
    const page = await buildPage(browser);
    const dragOver = async (selector) => {
      const box = await page.locator(selector).boundingBox();
      const y = box.y + box.height / 2;
      await page.mouse.move(box.x + 1, y);
      await page.mouse.down();
      await page.mouse.move(box.x + box.width - 1, y, { steps: 5 });
      await page.mouse.up();
    };

    await dragOver('#zhprobe');
    const dragged = await page.evaluate(() => ({
      hits: Array.from(
        document.querySelectorAll('#preview mark.mdp-word-hit'),
        (mark) => mark.textContent,
      ),
      selection: String(window.getSelection()),
    }));
    check(
      '拖拽选中四个字后标出同一词的其它出现',
      dragged.hits.length === EXPECTED_ZH_HITS && dragged.hits.every((text) => text === '取词高亮'),
      `命中=${dragged.hits.length} ${JSON.stringify(dragged.hits)}`,
    );
    check('自己的选区保留原生选中色', dragged.selection === '取词高亮', JSON.stringify(dragged.selection));

    // 带空格的整句不是"一个词"，不该被标
    await page.keyboard.press('Escape');
    await dragOver('#line1');
    const sentence = await page.evaluate(
      () => document.querySelectorAll('#preview mark.mdp-word-hit').length,
    );
    check('拖拽整句（含空格）不取词', sentence === 0, `命中=${sentence}`);

    // 编辑框里的选区不归预览管
    await page.evaluate(() => {
      const editor = document.getElementById('editor');
      editor.value = 'secret token';
      editor.focus();
      editor.setSelectionRange(0, 6);
      document.dispatchEvent(new MouseEvent('mouseup', { bubbles: true, button: 0 }));
    });
    const inEditor = await page.evaluate(
      () => document.querySelectorAll('#preview mark.mdp-word-hit').length,
    );
    check('编辑框里的选中不触发正文取词', inEditor === 0, `命中=${inEditor}`);
    await page.close();
  }

  // 3. 侧栏宽度：拖动跟随指针、松手落盘、越界夹回区间
  {
    const page = await buildPage(browser);
    check('初始宽度来自设置', (await sidebarWidth(page)) === DEFAULT_SIDEBAR_W);

    const drag = async (toX) => {
      // 拖拽条跟着侧栏右缘走，每轮先把宽度复位再按下去，否则会按在侧栏内部
      await page.evaluate(() =>
        window.__setSettings({ sidebarOpen: true, sidebarWidth: 260 }),
      );
      await page.mouse.move(DEFAULT_SIDEBAR_W - 3, 400);
      await page.mouse.down();
      await page.mouse.move(toX, 400, { steps: 5 });
      await page.mouse.up();
    };

    await drag(340);
    const widened = await page.evaluate(() => ({
      width: Math.round(document.getElementById('sidebar').getBoundingClientRect().width),
      bodyPad: Math.round(parseFloat(getComputedStyle(document.body).paddingLeft)),
      message: window.__messages[window.__messages.length - 1],
    }));
    check('拖动后侧栏跟着变宽', widened.width === 340, `宽度=${widened.width}`);
    check('正文左边距同步让出空间', widened.bodyPad === 340, `padding-left=${widened.bodyPad}`);
    check(
      '松手把宽度报给 Rust 落盘',
      widened.message === 'set-setting:sidebar-width=340',
      JSON.stringify(widened.message),
    );

    await drag(2000);
    check(`拖过最宽收在 ${SIDEBAR_MAX_W}px`, (await sidebarWidth(page)) === SIDEBAR_MAX_W, `宽度=${await sidebarWidth(page)}`);
    await drag(50);
    check(`拖过最窄收在 ${SIDEBAR_MIN_W}px`, (await sidebarWidth(page)) === SIDEBAR_MIN_W);

    // 启动回显走的是同一条路：越界值也不能被照搬
    await page.evaluate(() => window.__setSettings({ sidebarOpen: true, sidebarWidth: 9999 }));
    check(
      '设置回显的越界宽度同样被夹住',
      (await sidebarWidth(page)) === SIDEBAR_MAX_W,
      `宽度=${await sidebarWidth(page)}`,
    );
    await page.evaluate(() => window.__setSettings({ sidebarOpen: true, sidebarWidth: 320 }));
    check('设置回显生效', (await sidebarWidth(page)) === 320);
    await page.close();
  }

  // 4. 侧栏压到最窄时，三个分区标签必须还待在自己的框里
  {
    const page = await buildPage(browser);
    await page.evaluate(
      (width) => window.__setSettings({ sidebarOpen: true, sidebarWidth: width }),
      SIDEBAR_MIN_W,
    );
    const overflowOfLabels = () =>
      page.evaluate(() =>
        Array.from(document.querySelectorAll('.sidebar-sections button'), (button) => ({
          label: button.textContent,
          height: Math.round(button.getBoundingClientRect().height),
          over: Math.max(button.scrollHeight - button.clientHeight, button.scrollWidth - button.clientWidth),
        })),
      );
    const inside = (rows) => rows.every((row) => row.over <= 0 && row.height > 0);

    // 先确认这把量尺能抓到老写法：高度写死 28px 时，两行文字必然溢出框外
    await page.addStyleTag({
      content: '.sidebar-sections button { height: 28px; padding: 0 6px; line-height: inherit; }',
    });
    const broken = await overflowOfLabels();
    check('量尺能抓到写死高度造成的溢出', !inside(broken), JSON.stringify(broken));
    await page.evaluate(() => {
      const styles = document.querySelectorAll('style');
      styles[styles.length - 1].remove();
    });

    const labels = await overflowOfLabels();
    check(`侧栏 ${SIDEBAR_MIN_W}px 时标签不溢出自己的框`, inside(labels), JSON.stringify(labels));
    await page.close();
  }
} finally {
  await browser.close();
}

if (failures.length) {
  throw new Error(`desktop word highlight / sidebar resize failed: ${failures.join(', ')}`);
}
console.log('[desktop-word-highlight-verify] OK');
