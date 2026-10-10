// Ctrl+Tab 切换浮层验收：按住 Ctrl 停留、连按 Tab 选中、松开 Ctrl 才切
// 顺序是"最近用过的标签在前"，且只认真正的 Ctrl（macOS 的 Cmd+Tab 拿不到）
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

const { configScript, desktopScript } = await import('./desktop-page.mjs');

const pageConfig = configScript({
  btnEditJs: 'Edit',
  btnPreviewJs: 'Preview',
  sidebarEmptyJs: 'empty',
  statWordsJs: '字',
  statCharsJs: '字符',
  switchTabTitle: 'Recently Used Tabs',
  switchTabHint: 'Release Ctrl to switch to the selected tab',
});

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1000, height: 700 } });
await page.setContent(`<!doctype html>
<html>
<head><meta charset="utf-8"></head>
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
  <aside id="sidebar"><div id="sidebar-list"></div></aside>
  <div class="findbar"><input id="find-input"><span id="find-state"></span><button id="find-prev"></button><button id="find-next"></button><button id="find-close"></button></div>
  <div id="app"><div id="preview"></div><textarea id="editor"></textarea></div>
  <div id="tab-switcher" role="listbox" aria-hidden="true" style="display:none;">
    <div class="tab-switcher-title">Recently Used Tabs</div>
    <div class="tab-switcher-list" id="tab-switcher-list"></div>
    <div class="tab-switcher-hint">Release Ctrl to switch to the selected tab</div>
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

// 三个标签，当前是 b.md（模拟 Rust 侧推来的标签状态）
const TABS = [
  { id: 11, name: 'a.md', path: 'D:\\docs\\a.md', active: false, missing: false, dirty: false },
  { id: 22, name: 'b.md', path: 'D:\\docs\\work\\b.md', active: true, missing: false, dirty: true },
  { id: 33, name: 'c.md', path: 'D:\\docs\\c.md', active: false, missing: false, dirty: false },
];

// 先按 a -> b -> c -> b 的顺序激活过，MRU 应当是 b, c, a
await page.evaluate(async (tabs) => {
  const set = (activeId) => tabs.map((tab) => ({ ...tab, active: tab.id === activeId }));
  window.__setTabs(set(11));
  window.__setTabs(set(22));
  window.__setTabs(set(33));
  window.__setTabs(set(22));
}, TABS);

const press = (options) =>
  page.evaluate((opts) => {
    document.dispatchEvent(
      new KeyboardEvent('keydown', {
        key: 'Tab',
        code: 'Tab',
        keyCode: 9,
        ctrlKey: !!opts.ctrl,
        metaKey: !!opts.meta,
        shiftKey: !!opts.shift,
        bubbles: true,
        cancelable: true,
      }),
    );
  }, options);
const releaseCtrl = () =>
  page.evaluate(() => {
    document.dispatchEvent(new KeyboardEvent('keyup', { key: 'Control', bubbles: true }));
  });
const state = () =>
  page.evaluate(() => {
    const sw = document.getElementById('tab-switcher');
    const items = Array.from(document.querySelectorAll('.tab-switcher-item'));
    return {
      visible: sw.style.display !== 'none',
      ariaHidden: sw.getAttribute('aria-hidden'),
      names: items.map((item) => item.querySelector('.tab-switcher-name').textContent),
      paths: items.map((item) => item.querySelector('.tab-switcher-path').textContent),
      selected: items.findIndex((item) => item.classList.contains('selected')),
      activeDescendant: sw.getAttribute('aria-activedescendant'),
    };
  });

// 1. Ctrl+Tab：浮层出现，默认选中"上次看的那个"（MRU 第二项 = c.md）
await press({ ctrl: true });
let open = await state();
if (!open.visible || open.ariaHidden !== 'false') {
  throw new Error(`Ctrl+Tab should open the switcher: ${JSON.stringify(open)}`);
}
if (open.names.join(',') !== 'b.md,c.md,a.md') {
  throw new Error(`expected MRU order b.md,c.md,a.md, got ${JSON.stringify(open.names)}`);
}
if (open.selected !== 1) {
  throw new Error(`first Ctrl+Tab should select the previously used tab: ${open.selected}`);
}
// 第二项是 c.md（D:\docs\c.md），目录部分不带结尾分隔符
if (open.paths[1] !== 'D:\\docs') {
  throw new Error(`switcher should show the parent folder: ${JSON.stringify(open.paths)}`);
}
// 2. 连按 Tab 往下选，松开 Ctrl 才切到选中的标签
await press({ ctrl: true });
const moved = await state();
if (moved.selected !== 2) throw new Error(`second Tab should move down: ${moved.selected}`);
await releaseCtrl();
const closed = await state();
if (closed.visible) throw new Error('releasing Ctrl should close the switcher');
const activate = await page.evaluate(() =>
  window.__messages.filter((message) => message.startsWith('tab-action:activate:')),
);
if (activate.length !== 1 || !activate[0].startsWith('tab-action:activate:11')) {
  throw new Error(`releasing Ctrl should activate the selected tab: ${JSON.stringify(activate)}`);
}

// 3. Shift+Tab 反向：默认选中项往前一位（绕到最后一个）
await page.evaluate(() => { window.__messages.length = 0; });
await press({ ctrl: true });
await press({ ctrl: true, shift: true });
const backwards = await state();
if (backwards.selected !== 0) {
  throw new Error(`Shift+Ctrl+Tab should move up: ${backwards.selected}`);
}
await releaseCtrl();
const noSwitch = await page.evaluate(() =>
  window.__messages.filter((message) => message.startsWith('tab-action:activate:')),
);
if (noSwitch.length) {
  throw new Error(`selecting the current tab must not activate anything: ${JSON.stringify(noSwitch)}`);
}

// 4. Escape 放弃：不发激活消息
await page.evaluate(() => { window.__messages.length = 0; });
await press({ ctrl: true });
await page.evaluate(() => {
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', keyCode: 27, bubbles: true, cancelable: true }));
});
const escaped = await state();
if (escaped.visible) throw new Error('Escape should close the switcher');
await releaseCtrl();
const afterEscape = await page.evaluate(() =>
  window.__messages.filter((message) => message.startsWith('tab-action:activate:')),
);
if (afterEscape.length) throw new Error(`Escape must not switch tabs: ${JSON.stringify(afterEscape)}`);

// 5. 真正的 Cmd+Tab（macOS 系统键）不该触发：只有 Ctrl 生效
await press({ meta: true });
const metaState = await state();
if (metaState.visible) throw new Error('Cmd+Tab must not open the switcher');

// 6. 关掉两个标签后只有一个标签，浮层不该出现
await page.evaluate((tabs) => {
  window.__setTabs(tabs.filter((tab) => tab.id === 22).map((tab) => ({ ...tab, active: true })));
}, TABS);
await press({ ctrl: true });
const single = await state();
if (single.visible) throw new Error('a single tab should not open the switcher');
await releaseCtrl();

// 7. 快捷键被禁用时不弹
await page.evaluate(() => window.__setSettings({ disabledShortcuts: ['switch-tab'] }));
await page.evaluate((tabs) => window.__setTabs(tabs), TABS);
await press({ ctrl: true });
const disabled = await state();
if (disabled.visible) throw new Error('disabled switch-tab shortcut should not open the switcher');
await releaseCtrl();

// 长路径从左边截断，留得住目录尾巴
const longPath = await page.evaluate(() => {
  window.__setSettings({ disabledShortcuts: [] });
  window.__setTabs([
    { id: 1, name: 'deep.md', path: 'C:\\Users\\zhuzi\\Documents\\wechat\\file\\2026-08\\deep.md', active: true, dirty: false, missing: false },
    { id: 2, name: 'b.md', path: 'D:\\docs\\work\\b.md', active: false, dirty: false, missing: false },
  ]);
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', keyCode: 9, ctrlKey: true, bubbles: true, cancelable: true }));
  const popup = document.getElementById('tab-switcher');
  if (popup.style.display === 'none') throw new Error('switcher should be open for the path check');
  return document.querySelector('.tab-switcher-path').textContent;
});
if (!longPath.startsWith('…') || !longPath.endsWith('2026-08')) {
  throw new Error(`long paths should be truncated from the left: ${longPath}`);
}
await page.evaluate((tabs) => {
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', keyCode: 27, bubbles: true, cancelable: true }));
  document.dispatchEvent(new KeyboardEvent('keyup', { key: 'Control', bubbles: true }));
  // 把 MRU 恢复成 a→b→c→b（上面那段插曲改过激活顺序）
  const set = (id) => tabs.map((t) => Object.assign({}, t, { active: t.id === id }));
  window.__setTabs(set(11));
  window.__setTabs(set(22));
  window.__setTabs(set(33));
  window.__setTabs(set(22));
}, TABS);

console.log('[desktop-tab-switcher-verify] OK');
await browser.close();
