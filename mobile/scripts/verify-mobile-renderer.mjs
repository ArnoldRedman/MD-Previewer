import { createRequire } from 'module';
import { readFile } from 'node:fs/promises';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

const root = new URL('../..', import.meta.url);
const preview = new URL('mobile/shared/preview.html', root).href;
const previewCss = await readFile(new URL('mobile/shared/mobile-preview.css', root), 'utf8');

// 公式探测的快路径必须和它替掉的老写法逐输入一致。老写法（O(n²)）留在这里当语义基准，
// 从 mobile-preview.js 里把两个函数抠出来穷举比对
const rendererSource = await readFile(new URL('mobile/shared/mobile-preview.js', root), 'utf8');

function extractFunction(name) {
  const start = rendererSource.indexOf(`function ${name}(`);
  if (start < 0) throw new Error(`mobile-preview.js 里找不到 ${name}，公式探测比对失效`);
  let depth = 0;
  for (let i = rendererSource.indexOf('{', start); i < rendererSource.length; i += 1) {
    if (rendererSource[i] === '{') depth += 1;
    else if (rendererSource[i] === '}') {
      depth -= 1;
      if (depth === 0) return rendererSource.slice(start, i + 1);
    }
  }
  throw new Error(`${name} 的函数体没有闭合`);
}

const { hasUnescapedPair, hasInlineDollarMath } = new Function(
  `${extractFunction('hasUnescapedPair')}\n${extractFunction('hasInlineDollarMath')}\n` +
  'return { hasUnescapedPair, hasInlineDollarMath };'
)();

function referenceHasUnescapedPair(text, open, close) {
  let pos = 0;
  while ((pos = text.indexOf(open, pos)) >= 0) {
    const body = pos + open.length;
    const found = text.indexOf(close, body);
    if (found > body) return true;
    pos = body;
  }
  return false;
}

function referenceMath(markdown) {
  return referenceHasUnescapedPair(markdown, '$$', '$$') ||
    referenceHasUnescapedPair(markdown, '\\[', '\\]') ||
    referenceHasUnescapedPair(markdown, '\\(', '\\)') ||
    /(^|[^\\])\$[^\s$][\s\S]*?[^\s\\]\$/.test(markdown);
}

function currentMath(markdown) {
  return hasUnescapedPair(markdown, '$$', '$$') ||
    hasUnescapedPair(markdown, '\\[', '\\]') ||
    hasUnescapedPair(markdown, '\\(', '\\)') ||
    hasInlineDollarMath(markdown);
}

const alphabet = ['$', '\\', ' ', 'a', '[', ']', '(', ')', '\n', 'x'];
const mismatches = [];
let inputs = 0;
for (let len = 0; len <= 6; len += 1) {
  const total = alphabet.length ** len;
  for (let code = 0; code < total; code += 1) {
    let n = code;
    let text = '';
    for (let i = 0; i < len; i += 1) {
      text += alphabet[n % alphabet.length];
      n = Math.floor(n / alphabet.length);
    }
    inputs += 1;
    if (referenceMath(text) !== currentMath(text)) mismatches.push(JSON.stringify(text));
  }
}
if (mismatches.length > 0) {
  throw new Error(`公式探测和基准实现在 ${mismatches.length} 个输入上不一致：${mismatches.slice(0, 10).join(' ')}`);
}

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({
  viewport: { width: 390, height: 844 },
  deviceScaleFactor: 2,
  isMobile: true
});

const errors = [];
page.on('pageerror', error => errors.push(error.message));
page.on('console', message => {
  if (message.type() === 'error') errors.push(message.text());
});

await page.goto(preview);
await page.waitForLoadState('domcontentloaded');

// `$` 密集的大文档：公式探测以前对每个 `$` 都往串尾扫一遍（O(n²)），手机上会卡到像打不开。
// 这里断言同步渲染耗时和「没成对公式就不加载 KaTeX」的判定
const heavy = await page.evaluate(() => {
  const markdown = Array.from({ length: 12000 }, (_, i) => `- item ${i} $x $y`).join('\n');
  const start = performance.now();
  window.MDPreviewer.render({ name: 'heavy-dollar.md', markdown });
  return {
    elapsed: performance.now() - start,
    bytes: markdown.length,
    katex: document.querySelectorAll('.katex').length,
    dollars: (markdown.match(/\$/g) || []).length
  };
});
if (heavy.katex !== 0) {
  throw new Error(`Dollar-heavy document loaded KaTeX: ${JSON.stringify(heavy)}`);
}
if (heavy.elapsed > 5000) {
  throw new Error(`Dollar-heavy document took ${heavy.elapsed}ms to render: ${JSON.stringify(heavy)}`);
}

// 表格要自适应到屏幕宽度：五列表格不能顶出可视范围，也不能靠整页横滑才看得到
await page.evaluate(() => {
  const rows = [
    ['模块', '说明', '负责人', '状态', '备注'],
    ['预览渲染', '把 Markdown 转成 HTML 并做消毒，标题要生成锚点', '张三', '已完成', '需要回归大文档与表格'],
    ['侧边栏', '文件树、最近文件、目录大纲三段折叠面板', '李四', '进行中', '移动端隐藏']
  ];
  const [head, ...body] = rows;
  const table = `<table><thead><tr>${head.map((cell) => `<th>${cell}</th>`).join('')}</tr></thead>` +
    `<tbody>${body.map((row) => `<tr>${row.map((cell) => `<td>${cell}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
  window.MDPreviewer.render({ name: 'table.md', markdown: `表格自适应\n\n${table}` });
});
await page.waitForFunction(() => document.querySelector('#preview .mdp-table-wrap'), null, { timeout: 5000 });
const tableFit = await page.evaluate(() => {
  const table = document.querySelector('#preview table');
  const wrap = table.closest('.mdp-table-wrap');
  const cells = [...table.querySelectorAll('td, th')];
  return {
    viewport: document.documentElement.clientWidth,
    pageScroll: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    beyondViewport: Math.round(table.getBoundingClientRect().right - document.documentElement.clientWidth),
    wrapScroll: Math.round(wrap.scrollWidth - wrap.clientWidth),
    clippedCells: cells.filter((cell) => cell.scrollWidth > cell.clientWidth + 1).length
  };
});
if (tableFit.pageScroll > 0 || tableFit.beyondViewport > 1 || tableFit.wrapScroll > 1 || tableFit.clippedCells > 0) {
  throw new Error(`五列表格在手机上顶出了屏幕：${JSON.stringify(tableFit)}`);
}

await page.evaluate(() => {
  window.MDPreviewer.render({
    name: 'mobile-fixture.md',
    baseHref: 'file:///tmp/md-previewer-docs/',
    markdown: [
      '# Mobile fixture',
      '',
      'Inline math $a^2+b^2=c^2$ and display math:',
      '',
      '$$E=mc^2$$',
      '',
      '> [!IMPORTANT]',
      '> This alert should render as a GitHub alert.',
      '',
      'This has ==highlighted text== but `==literal code==` stays literal.',
      '',
      '```mermaid',
      'graph TD',
      '  A[Open] --> B[Preview]',
      '```',
      '',
      '[bad](javascript:window.__bad=1)'
    ].join('\n')
  });
});

await page.waitForSelector('.katex', { timeout: 5000 });
await page.waitForSelector('.mdp-mermaid svg', { timeout: 5000 });
await page.waitForSelector('.markdown-alert-important', { timeout: 1000 });
await page.waitForSelector('mark.mdp-mark', { timeout: 1000 });
const beforeSearchTop = await page.locator('#app').boundingBox();
await page.locator('#search-toggle').click();
const searchingTop = await page.locator('#app').boundingBox();
await page.locator('#search-input').fill('math');
await page.waitForSelector('mark.search-hit.current', { timeout: 1000 });
const searchHitCount = await page.locator('mark.search-hit').count();
await page.locator('#search-close').click();
const afterSearchTop = await page.locator('#app').boundingBox();
await page.locator('a[href^="javascript:"]').click();
await page.emulateMedia({ colorScheme: 'dark' });
const darkAlert = await page.evaluate(() => ({
  background: getComputedStyle(document.querySelector('.markdown-alert-important')).backgroundColor,
  titleColor: getComputedStyle(document.querySelector('.markdown-alert-important .markdown-alert-title')).color
}));
await page.emulateMedia({ media: 'print', colorScheme: 'dark' });

const result = await page.evaluate((searchHits) => ({
  title: document.getElementById('title').textContent,
  katex: document.querySelectorAll('.katex').length,
  mermaidSvg: document.querySelectorAll('.mdp-mermaid svg').length,
  alertTitle: document.querySelector('.markdown-alert-important .markdown-alert-title')?.textContent.trim(),
  alertText: document.querySelector('.markdown-alert-important p:not(.markdown-alert-title)')?.textContent.trim(),
  alertBg: getComputedStyle(document.querySelector('.markdown-alert-important')).backgroundColor,
  highlightText: document.querySelector('mark.mdp-mark')?.textContent,
  codeLiteral: document.querySelector('code')?.textContent,
  topActionIcons: document.querySelectorAll('#top-actions .tool-button svg').length,
  searchHits,
  printTopbarDisplay: getComputedStyle(document.getElementById('topbar')).display,
  printSearchDisplay: getComputedStyle(document.getElementById('search-box')).display,
  printPreviewDisplay: getComputedStyle(document.getElementById('preview')).display,
  bad: window.__bad === 1
}), searchHitCount);

await page.evaluate(() => {
  window.MDPreviewer.render({
    name: 'notes.txt',
    markdown: [
      'Line 1: Plain text file with <tag> & symbols',
      'Line 2: # Not A Heading',
      'Line 3: *not italic*',
      'Line 4: search_keyword_plain_text'
    ].join('\n')
  });
});

await page.waitForSelector('.mdp-plain-text', { timeout: 1000 });
const txtResult = await page.evaluate(() => ({
  title: document.getElementById('title').textContent,
  plainTextContent: document.querySelector('.mdp-plain-text')?.textContent,
  plainTextHtml: document.querySelector('.mdp-plain-text')?.innerHTML,
  h1Count: document.querySelectorAll('#preview h1').length,
  emCount: document.querySelectorAll('#preview em').length,
  katexCount: document.querySelectorAll('.katex').length
}));

await browser.close();

if (errors.length) {
  throw new Error(`Renderer console errors:\n${errors.join('\n')}`);
}
if (result.title !== 'mobile-fixture.md') {
  throw new Error(`Unexpected title: ${result.title}`);
}
if (!result.katex || !result.mermaidSvg || !result.searchHits) {
  throw new Error(`Renderer feature check failed: ${JSON.stringify(result)}`);
}
if (result.alertTitle !== 'Important' ||
    result.alertText !== 'This alert should render as a GitHub alert.' ||
    result.alertBg === 'rgba(0, 0, 0, 0)' ||
    result.highlightText !== 'highlighted text' ||
    result.codeLiteral !== '==literal code==') {
  throw new Error(`Markdown extension check failed: ${JSON.stringify(result)}`);
}
if (darkAlert.background !== 'rgb(22, 27, 34)' ||
    darkAlert.titleColor !== 'rgb(163, 113, 247)') {
  throw new Error(`Dark alert style check failed: ${JSON.stringify(darkAlert)}`);
}
if (result.topActionIcons !== 3) {
  throw new Error(`Toolbar icons missing: ${JSON.stringify(result)}`);
}
if (Math.abs(beforeSearchTop.y - searchingTop.y) > 1 ||
    Math.abs(beforeSearchTop.y - afterSearchTop.y) > 1) {
  throw new Error(`Search changed document position: ${JSON.stringify({
    before: beforeSearchTop.y,
    searching: searchingTop.y,
    after: afterSearchTop.y
  })}`);
}
if (result.printTopbarDisplay !== 'none' ||
    result.printSearchDisplay !== 'none' ||
    result.printPreviewDisplay === 'none' ||
    !/@page\s*{\s*margin:\s*12mm;\s*}/.test(previewCss)) {
  throw new Error(`Print stylesheet check failed: ${JSON.stringify(result)}`);
}
if (result.bad) {
  throw new Error('javascript: link executed');
}

if (txtResult.title !== 'notes.txt') {
  throw new Error(`Unexpected txt title: ${txtResult.title}`);
}
if (!txtResult.plainTextContent.includes('Line 1: Plain text file with <tag> & symbols') ||
    !txtResult.plainTextContent.includes('\nLine 2: # Not A Heading') ||
    !txtResult.plainTextHtml.includes('&lt;tag&gt; &amp; symbols') ||
    txtResult.h1Count !== 0 ||
    txtResult.emCount !== 0 ||
    txtResult.katexCount !== 0) {
  throw new Error(`TXT document render check failed: ${JSON.stringify(txtResult)}`);
}

console.log(`[mobile-renderer] OK (${inputs} flag inputs matched the reference; dollar-heavy ${heavy.bytes} bytes / ${heavy.dollars} dollars in ${Math.round(heavy.elapsed)}ms)`);
