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

const browser = await chromium.launch();
const viewports = [
  { name: 'wide', width: 1180, height: 900 },
  { name: 'default', width: 900, height: 700 },
  { name: 'narrow', width: 680, height: 700 },
];
const report = [];
try {
  for (const viewport of viewports) {
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
      { timeout: 5000 }
    );

    const measured = await page.evaluate(() => {
      const tables = [...document.querySelectorAll('#preview table')];
      return {
        pageScroll: document.documentElement.scrollWidth - document.documentElement.clientWidth,
        tables: tables.map((element) => {
          const wrap = element.closest('.mdp-table-wrap');
          const cells = [...element.querySelectorAll('td, th')];
          const box = element.getBoundingClientRect();
          const limit = (wrap || document.getElementById('app')).getBoundingClientRect();
          return {
            cols: element.querySelector('tr').children.length,
            width: Math.round(box.width),
            beyondLimit: Math.round(box.right - limit.right),
            clippedCells: cells.filter((cell) => cell.scrollWidth > cell.clientWidth + 1).length,
            wrapScroll: wrap ? Math.round(wrap.scrollWidth - wrap.clientWidth) : null,
          };
        }),
      };
    });

    const problems = [];
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
      // 五列表格在阅读宽度内必须自适应到可视宽度，不能逼用户拖动
      if (item.cols === 5 && (item.wrapScroll ?? 0) > 0) {
        problems.push(`五列表格放不下可视宽度，还要横向拖 ${item.wrapScroll}px`);
      }
      // 十列表格允许自己滚，但不能顶出容器
      if (item.cols === 10 && item.wrapScroll === null) {
        problems.push('十列表格没有横向滚动容器');
      }
    }
    if (problems.length > 0) {
      throw new Error(`${viewport.name}(${viewport.width}px) 表格自适应失败：${problems.join('；')}\n${JSON.stringify(measured)}`);
    }
    report.push(`${viewport.name}=${JSON.stringify(measured.tables.map((item) => `${item.cols}列/${item.width}px/滚动${item.wrapScroll}`))}`);
  }
} finally {
  await browser.close();
}
console.log(`[desktop-tables] OK ${report.join(' ')}`);
