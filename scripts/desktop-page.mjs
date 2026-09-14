// 桌面预览页前端资源与全项目源码的统一读取入口。
// 验证脚本据此注入页面和做静态断言，不再各自解析 src/main.rs。
import { readdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

export const root = fileURLToPath(new URL('..', import.meta.url));

// 桌面预览页的前端资源目录
const FRONTEND = 'frontend';

async function collectRustFiles(dir) {
  const entries = await readdir(dir, { withFileTypes: true });
  const files = [];
  for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) files.push(...(await collectRustFiles(full)));
    else if (entry.name.endsWith('.rs')) files.push(full);
  }
  return files;
}

const rustFiles = await collectRustFiles(join(root, 'src'));
export const rustSource = (
  await Promise.all(rustFiles.map((file) => readFile(file, 'utf8')))
).join('\n');

export const pageHtml = await readFile(join(root, FRONTEND, 'page.html'), 'utf8');
export const desktopScript = await readFile(join(root, FRONTEND, 'page.js'), 'utf8');
export const desktopCss = await readFile(join(root, FRONTEND, 'page.css'), 'utf8');

// 静态断言的统一查找目标：Rust 全量源码 + 页面模板 + 前端脚本
export const appSource = `${rustSource}\n${pageHtml}\n${desktopScript}`;

// 测试页样式：真实页面里侧栏宽度由 Rust 注入，测试页补固定值
export const desktopStyle = `<style>:root { --sidebar-width: 260px; --sidebar-toggle-left: 272px; }</style>
<style>
${desktopCss}</style>`;

// 真实页面里由 Rust 渲染注入的前端文案，测试页用这套默认值
const DEFAULT_CONFIG = {
  btnEditJs: 'Edit',
  btnPreviewJs: 'Preview',
  codeCopyJs: 'Copy',
  codeCopiedJs: 'Copied',
  sidebarEmptyJs: 'Nothing to show',
  sidebarOutlineEmptyJs: 'No headings',
  statWordsJs: 'words',
  statCharsJs: 'chars',
  copyTitleLineJs: 'Copy heading',
  copyTitleJs: 'Copy title',
  copyBodyJs: 'Copy body',
  copiedJs: 'Copied',
  cargoVersion: '1.4.2',
  updateStatusCheckingJs: 'Checking...',
  updateStatusLatestJs: 'Up to date',
  updateStatusFailedJs: 'Check failed',
  updateDownloadingJs: 'Downloading...',
  updateInstallingJs: 'Installing...',
  updateFailedJs: 'Update failed: ',
  btnUpdateTextJs: 'Update Available',
  btnCheckUpdateJs: 'Check for Updates',
};

export function configJson(overrides = {}) {
  return JSON.stringify({ ...DEFAULT_CONFIG, ...overrides });
}

export function configScript(overrides = {}) {
  return `<script>window.__mdPreviewerConfig = ${configJson(overrides)};</script>`;
}
