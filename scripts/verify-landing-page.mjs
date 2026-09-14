// 发布落地页验证：下载链接必须指向构建脚本产出的真实资产名，中英文页面都要能看，
// 桌面/移动视口不能横向溢出，搜索引擎入口（sitemap / robots / hreflang）不能漏
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { join } from 'node:path';

import { root } from './desktop-page.mjs';

const SITE = join(root, 'docs');
const PAGES = {
  'index.html': 'zh-CN',
  'en.html': 'en',
};

const windowsBuild = await readFile(join(root, 'scripts/build-windows.ps1'), 'utf8');
// 资产名以构建脚本为准，落地页不能自己另起一套
const assets = [...windowsBuild.matchAll(/\$(\w+)\s*=\s*Join-Path \$dist "([^"]+\.exe)"/g)].map(
  (match) => match[2],
);
if (assets.length === 0) throw new Error('build-windows.ps1 里没找到产物名');

const mobileBuild = await readFile(join(root, 'mobile/scripts/build-release.sh'), 'utf8');
// 安卓包同样以发布脚本里的资产名为准
const apk = mobileBuild.match(/MD-Previewer-mobile\.apk/);
if (!apk) throw new Error('build-release.sh 里没找到安卓发布资产名');
assets.push(apk[0]);

// Linux 两个包在 build-linux.sh 里复制成不带版本号的发布名，架构取自脚本里的默认值
const linuxBuild = await readFile(join(root, 'scripts/build-linux.sh'), 'utf8');
const releaseArch = linuxBuild.match(/^RELEASE_ARCH="([a-z0-9]+)"/m)?.[1];
const debArch = linuxBuild.match(/^DEB_ARCH="([a-z0-9]+)"/m)?.[1];
if (!releaseArch || !debArch) throw new Error('build-linux.sh 里没找到发布架构默认值');
for (const [template, name] of [
  ['MD-Previewer-linux-$RELEASE_ARCH.tar.gz', `MD-Previewer-linux-${releaseArch}.tar.gz`],
  ['MD-Previewer-linux-$DEB_ARCH.deb', `MD-Previewer-linux-${debArch}.deb`],
]) {
  if (!linuxBuild.includes(template)) throw new Error(`build-linux.sh 里没找到发布资产名模板 ${template}`);
  assets.push(name);
}

const pages = {};
for (const name of Object.keys(PAGES)) {
  pages[name] = await readFile(join(SITE, name), 'utf8');
}

for (const [name, html] of Object.entries(pages)) {
  for (const asset of assets) {
    const link = `releases/latest/download/${asset}`;
    if (!html.includes(link)) throw new Error(`${name} 缺少下载链接: ${link}`);
  }
  if (!html.includes('https://github.com/ArnoldRedman/MD-Previewer/releases')) {
    throw new Error(`${name} 必须给出 releases 入口`);
  }
  if (!html.includes('href="site.css"')) {
    throw new Error(`${name} 没有引用共用的 site.css`);
  }
  // 两套语言必须互相声明，否则搜索会当两条独立内容
  if (!html.includes('hreflang="zh-CN"') || !html.includes('hreflang="en"')) {
    throw new Error(`${name} 缺少 hreflang 交叉声明`);
  }
  for (const shot of ['icon.png', 'preview.png', 'preview-dark.png', 'editor.png']) {
    if (!html.includes(shot)) throw new Error(`${name} 没有引用截图 ${shot}`);
  }
  if (!html.includes('"@type": "SoftwareApplication"')) {
    throw new Error(`${name} 缺少结构化数据`);
  }
}

if (!pages['index.html'].includes('href="en.html"')) {
  throw new Error('中文页必须能切到英文页');
}
if (!pages['en.html'].includes('hreflang="zh-CN">中文</a>')) {
  throw new Error('英文页必须能切回中文页');
}

// 搜索入口
const sitemap = await readFile(join(SITE, 'sitemap.xml'), 'utf8');
for (const path of ['https://arnoldredman.github.io/MD-Previewer/', 'https://arnoldredman.github.io/MD-Previewer/en.html']) {
  if (!sitemap.includes(`<loc>${path}</loc>`)) throw new Error(`sitemap 缺少 ${path}`);
}
const robots = await readFile(join(SITE, 'robots.txt'), 'utf8');
if (!robots.includes('Sitemap: https://arnoldredman.github.io/MD-Previewer/sitemap.xml')) {
  throw new Error('robots.txt 没有指向 sitemap');
}

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');
const browser = await chromium.launch();
const expectText = {
  'index.html': ['打开 Markdown，就该像打开记事本一样快', '下载 Windows 版', 'MD-Previewer-Setup.exe', '快捷键'],
  'en.html': ['Markdown should open as fast as a text file', 'Download for Windows', 'MD-Previewer-Setup.exe', 'Shortcuts'],
};
for (const [name, needles] of Object.entries(expectText)) {
  for (const viewport of [
    { width: 1280, height: 900 },
    { width: 375, height: 780 },
  ]) {
    const page = await browser.newPage({ viewport });
    await page.goto(`file://${join(SITE, name)}`);
    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
    );
    if (overflow > 1) throw new Error(`${name} 在 ${viewport.width}px 视口横向溢出 ${overflow}px`);
    const lang = await page.evaluate(() => document.documentElement.lang);
    if (lang !== PAGES[name]) throw new Error(`${name} 的 html lang 应为 ${PAGES[name]}，实际 ${lang}`);
    for (const needle of needles) {
      if ((await page.getByText(needle, { exact: false }).count()) === 0) {
        throw new Error(`${name} 在 ${viewport.width}px 下缺少内容: ${needle}`);
      }
    }
    await page.close();
  }
}
await browser.close();
console.log('[landing-page-verify] OK');
