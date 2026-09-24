import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import { configScript, desktopScript, desktopStyle, root } from './desktop-page.mjs';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');
const enhanceJs = await readFile(resolve(root, 'frontend/preview-enhance.js'), 'utf8');
const pageConfig = configScript({});

// 表格标记和 pulldown 渲染出来的一致（thead/th/tbody/td），用真页面样式和增强层跑
function table(rows) {
  const [head, ...body] = rows;
  const headHtml = `<thead><tr>${head.map((cell) => `<th>${cell}</th>`).join('')}</tr></thead>`;
  const bodyHtml = `<tbody>${body
    .map((row) => `<tr>${row.map((cell) => `<td>${cell}</td>`).join('')}</tr>`)
    .join('')}</tbody>`;
  return `<table>${headHtml}${bodyHtml}</table>`;
}

// 典型 AI 生成文档里的五列表格：中文长文本 + 表头
const fiveColumns = table([
  ['模块', '说明', '负责人', '状态', '备注'],
  ['预览渲染', '把 Markdown 转成 HTML 并做消毒，标题要生成锚点，链接要能在系统浏览器里打开', '张三', '已完成', '需要回归大文档与表格'],
  ['侧边栏', '文件树、最近文件、目录大纲三段折叠面板，宽窄窗口都要能用', '李四', '进行中', '移动端隐藏'],
  ['更新检查', '启动后定时检查新版本，下载要带进度和失败重试', '赵六', '待排期', '仅 Windows'],
]);
// 超长链接：必须能在单元格里断开，而不是把表格顶宽
const longLink = table([
  ['资源', '地址', '说明'],
  [
    '官网',
    'https://github.com/ArnoldRedman/md-preview/releases/latest/download/MD-Previewer-windows-x64.exe',
    '下载入口',
  ],
]);
// 列多到放不下：允许表格自己横向滚动，但不能顶宽整个页面
const huge = table([
  ['序号', '名称', '类型', '默认值', '取值范围', '生效时机', '依赖', '备注', '负责人', '状态'],
  ['1', '字号', 'number', '15', '12 到 24', '立即', '无', '只影响正文', '张三', '已完成'],
  ['2', '行高', 'number', '1.6', '1.2 到 2.0', '立即', '无', '影响全文', '李四', '已完成'],
]);

const preview = `<p>表格自适应检查</p>${fiveColumns}<p>超长链接</p>${longLink}<p>十列</p>${huge}`;

// 侧栏和编辑分栏都会挤掉正文宽度：表格容器如果按 100vw 算宽度就会从窗口右边溢出去
const layouts = [
  { name: 'default', apply: null },
  {
    name: 'sidebar',
    apply: () => {
      document.body.classList.remove('empty');
      document.body.classList.add('sidebar-open');
    },
  },
  {
    name: 'split',
    apply: () => {
      document.body.classList.add('editing', 'split-view');
    },
  },
];
const viewports = [
  { name: 'wide', width: 1180, height: 900 },
  { name: 'default', width: 900, height: 700 },
  { name: 'narrow', width: 680, height: 700 },
];

const browser = await chromium.launch();
const report = [];
try {
  for (const viewport of viewports) {
    for (const layout of layouts) {
      const page = await browser.newPage({ viewport: { width: viewport.width, height: viewport.height } });
      await page.setContent(`<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    ${desktopStyle}
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
      <div id="preview">${preview}</div>
      <textarea id="editor"></textarea>
    </div>
    <script>
      window.__messages = [];
      window.ipc = { postMessage(message) { window.__messages.push(message); } };
      window.__mdPreviewerFeatureFlags = { math: false, mermaid: false };
    </script>
    ${pageConfig}
    <script>${desktopScript}</script>
    <script>${enhanceJs}</script>
  </body>
</html>`);
      await page.evaluate(() => window.__setContent(
        document.getElementById('preview').innerHTML,
        'x',
        '',
        false,
        false,
      ));
      await page.evaluate(() => window.__enhancePreview());
      // 增强层把宽表（≥4 列）包进 .mdp-table-wrap 是 idle 回调里做的，要等它落地再量
      await page.waitForFunction(
        () => document.querySelectorAll('#preview .mdp-table-wrap').length >= 2,
        null,
        { timeout: 5000 },
      );
      if (layout.apply) {
        await page.evaluate(layout.apply);
        await page.waitForTimeout(200);
      }

      const measured = await page.evaluate(() => {
        const viewportWidth = document.documentElement.clientWidth;
        const tables = [...document.querySelectorAll('#preview table')].map((element) => {
          const wrap = element.closest('.mdp-table-wrap');
          const container = (wrap || element.parentElement).getBoundingClientRect();
          const cells = [...element.querySelectorAll('td, th')];
          return {
            cols: element.querySelector('tr').children.length,
            containerLeft: Math.round(container.left),
            containerWidth: Math.round(container.width),
            beyondViewport: Math.round(container.right - viewportWidth),
            wrapScroll: wrap ? Math.round(wrap.scrollWidth - wrap.clientWidth) : null,
            clippedCells: cells.filter((cell) => cell.scrollWidth > cell.clientWidth + 1).length,
          };
        });
        return {
          pageScroll: document.documentElement.scrollWidth - viewportWidth,
          tables,
        };
      });

      const problems = [];
      // 单元格宽度下限（min-width 64 + 左右 padding 24 + 边框 2）：列数 × 这个值放不进容器时，
      // 才允许表格在容器里横向滚；能放下就必须自适应，不能留横向拖动
      const minimumColumn = 90;
      if (measured.pageScroll > 0) {
        problems.push(`页面被表格顶出横向滚动：${measured.pageScroll}px`);
      }
      if (measured.tables.length !== 3) {
        problems.push(`表格数量不对：${measured.tables.length}`);
      }
      for (const item of measured.tables) {
        if (item.clippedCells > 0) {
          problems.push(`${item.cols} 列表格有 ${item.clippedCells} 个单元格内容溢出被裁`);
        }
        if (item.beyondViewport > 1 || item.containerLeft < -1) {
          problems.push(`${item.cols} 列表格容器顶出窗口：右超 ${item.beyondViewport}px，左 ${item.containerLeft}px`);
        }
        if ((item.wrapScroll ?? 0) > 0 && item.cols * minimumColumn <= item.containerWidth) {
          problems.push(
            `${item.cols} 列表格在 ${item.containerWidth}px 里还多出 ${item.wrapScroll}px 横向滚动，本来自适应就放得下`,
          );
        }
      }
      if (problems.length > 0) {
        throw new Error(
          `${viewport.name}/${layout.name}(${viewport.width}px) 表格自适应失败：${problems.join('；')}\n${JSON.stringify(measured)}`,
        );
      }
      report.push(
        `${viewport.name}/${layout.name}=${measured.tables
          .map((item) => `${item.cols}列滚${item.wrapScroll ?? '-'}`)
          .join(',')}`,
      );
      await page.close();
    }
  }
} finally {
  await browser.close();
}
console.log(`[desktop-tables] OK ${report.join(' ')}`);
