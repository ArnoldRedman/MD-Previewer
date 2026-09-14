// 发布落地页验证：下载链接必须指向真实存在的 release 资产名，
// 并且桌面/移动视口下不出现横向溢出
import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

import { root } from './desktop-page.mjs';

const html = await readFile(join(root, 'docs/index.html'), 'utf8');
const windowsBuild = await readFile(join(root, 'scripts/build-windows.ps1'), 'utf8');

// 资产名以构建脚本为准，落地页不能自己另起一套
const assets = [...windowsBuild.matchAll(/\$(\w+)\s*=\s*Join-Path \$dist "([^"]+\.exe)"/g)].map(
  (match) => match[2],
);
if (assets.length === 0) throw new Error('build-windows.ps1 里没找到产物名');
for (const asset of assets) {
  const link = `releases/latest/download/${asset}`;
  if (!html.includes(link)) throw new Error(`落地页缺少下载链接: ${link}`);
}

if (!html.includes('https://github.com/ArnoldRedman/MD-Previewer/releases')) {
  throw new Error('落地页必须给出 releases 入口');
}

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');
const browser = await chromium.launch();
for (const viewport of [
  { width: 1280, height: 800 },
  { width: 375, height: 720 },
]) {
  const page = await browser.newPage({ viewport });
  await page.goto(`file://${join(root, 'docs/index.html')}`);
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  );
  if (overflow > 1) {
    throw new Error(`${viewport.width}px 视口横向溢出 ${overflow}px`);
  }
  for (const text of ['Download for Windows', 'MD-Previewer-Setup.exe']) {
    if ((await page.getByText(text, { exact: false }).count()) === 0) {
      throw new Error(`${viewport.width}px 视口缺少内容: ${text}`);
    }
  }
  await page.close();
}
await browser.close();
console.log('[landing-page-verify] OK');
