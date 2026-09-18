import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

import { appSource, configJson, desktopCss, desktopScript, pageHtml } from './desktop-page.mjs';

// 1. Static checks on Rust and frontend sources
console.log('Verifying 1: Static checks on source...');

if (!appSource.includes('IpcMessage::LogError')) {
  throw new Error('Expected IpcMessage::LogError in Rust sources');
}
if (!appSource.includes('IpcMessage::OpenLog')) {
  throw new Error('Expected IpcMessage::OpenLog in Rust sources');
}
if (!appSource.includes('IpcMessage::ClearLog')) {
  throw new Error('Expected IpcMessage::ClearLog in Rust sources');
}
if (!appSource.includes('MAX_LOG_SIZE')) {
  throw new Error('Expected MAX_LOG_SIZE in logger.rs');
}
if (!appSource.includes('data-settings-tab="shortcuts"')) {
  throw new Error('Expected data-settings-tab="shortcuts" in page template');
}
if (!appSource.includes('disable-all-shortcuts')) {
  throw new Error('Expected disable-all-shortcuts toggle in page template');
}
if (!appSource.includes('data-shortcut-toggle="toggle-edit"')) {
  throw new Error('Expected data-shortcut-toggle="toggle-edit" in page template');
}
if (!appSource.includes('data-shortcut-toggle="escape"')) {
  throw new Error('Expected data-shortcut-toggle="escape" in page template');
}
if (!appSource.includes('isShortcutEnabled')) {
  throw new Error('Expected isShortcutEnabled helper in page.js');
}
if (!appSource.includes('e.isComposing || e.keyCode === 229')) {
  throw new Error('Expected IME composition guard in page.js keydown handler');
}

// 2. Playwright in-browser checks
console.log('Verifying 2: In-browser settings, shortcuts and logging UI & behavior...');
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1000, height: 750 } });

// Inject styles for headless testing: make toolbar interactable without hover
const testExtraCss = '\n.toolbar { opacity: 1 !important; pointer-events: auto !important; }\n';

// Assemble HTML from pageHtml template
let html = pageHtml
  .replace('{{base_tag}}', '')
  .replace('{{css_light}}', '')
  .replace('{{css_dark}}', '')
  .replace('{{sidebar_width}}', '260')
  .replace('{{sidebar_toggle_left}}', '272')
  .replace('{{css}}', desktopCss + testExtraCss)
  .replace('{{body_class}}', 'has-tabs')
  .replace('{{config_json}}', configJson())
  .replace('{{preview_html}}', '<p>Testing shortcuts and settings</p>')
  .replace('{{raw_md_escaped}}', 'Testing shortcuts and settings')
  .replace('{{needs_math}}', 'false')
  .replace('{{needs_mermaid}}', 'false')
  .replace('{{preview_enhance_js}}', '')
  .replace('{{js}}', desktopScript)
  .replace(/\{\{[a-zA-Z0-9_]+\}\}/g, 'label');

// Inject mock IPC before page.js runs
const mockIpcSnippet = `
<script>
  window.__messages = [];
  window.ipc = {
    postMessage: function(msg) {
      window.__messages.push(msg);
    }
  };
</script>
`;
html = html.replace('<body class="has-tabs">', `<body class="has-tabs">${mockIpcSnippet}`);

await page.setContent(html);

// Step A: Settings popover & Tab switching
console.log('Testing Step A: Settings popover & Tab switching...');
const isSettingsOpenInitially = await page.evaluate(() => {
  return document.getElementById('settings-control').classList.contains('open');
});
if (isSettingsOpenInitially) {
  throw new Error('Settings popover should be closed initially');
}

// Open settings popover
await page.click('#btn-settings', { force: true });
const isSettingsOpenAfterClick = await page.evaluate(() => {
  return document.getElementById('settings-control').classList.contains('open');
});
if (!isSettingsOpenAfterClick) {
  throw new Error('Settings popover should be open after clicking #btn-settings');
}

// General tab is active by default
const generalTabActive = await page.evaluate(() => {
  const btn = document.querySelector('.settings-tab-btn[data-settings-tab="general"]');
  const pane = document.getElementById('settings-tab-general');
  return btn.classList.contains('active') && pane.classList.contains('active');
});
if (!generalTabActive) {
  throw new Error('General tab should be active by default');
}

// Switch to Shortcuts tab
await page.click('.settings-tab-btn[data-settings-tab="shortcuts"]', { force: true });
const shortcutsTabActive = await page.evaluate(() => {
  const btn = document.querySelector('.settings-tab-btn[data-settings-tab="shortcuts"]');
  const pane = document.getElementById('settings-tab-shortcuts');
  const oldPane = document.getElementById('settings-tab-general');
  return btn.classList.contains('active') && pane.classList.contains('active') && !oldPane.classList.contains('active');
});
if (!shortcutsTabActive) {
  throw new Error('Shortcuts tab should become active when clicked');
}

// Step B: Shortcuts Master Switch
console.log('Testing Step B: Shortcuts Master Switch...');
await page.click('button[data-setting="disable-all-shortcuts"][data-value="on"]', { force: true });
const disableAllMsgSent = await page.evaluate(() => {
  return window.__messages.includes('set-setting:disable-all-shortcuts=on');
});
if (!disableAllMsgSent) {
  throw new Error('Expected "set-setting:disable-all-shortcuts=on" to be posted');
}

// Simulate settings update arriving from backend
await page.evaluate(() => {
  window.__setSettings({ disableAllShortcuts: true, disabledShortcuts: [] });
});

// Verify shortcuts-list has .disabled class
const hasDisabledClass = await page.evaluate(() => {
  return document.getElementById('shortcuts-list').classList.contains('disabled');
});
if (!hasDisabledClass) {
  throw new Error('Expected #shortcuts-list to have .disabled class');
}

// Close settings popover so keydowns target page
await page.click('#btn-settings', { force: true });

// Press Ctrl+E while all shortcuts disabled -> must NOT enter edit mode
await page.keyboard.press('Control+e');
const isEditingWhileDisabled = await page.evaluate(() => {
  return document.body.classList.contains('editing');
});
if (isEditingWhileDisabled) {
  throw new Error('Expected Ctrl+E to be suppressed when all shortcuts are disabled');
}

// Re-open settings to re-enable all shortcuts
await page.click('#btn-settings', { force: true });
await page.click('.settings-tab-btn[data-settings-tab="shortcuts"]', { force: true });
await page.click('button[data-setting="disable-all-shortcuts"][data-value="off"]', { force: true });
const enableAllMsgSent = await page.evaluate(() => {
  return window.__messages.includes('set-setting:disable-all-shortcuts=off');
});
if (!enableAllMsgSent) {
  throw new Error('Expected "set-setting:disable-all-shortcuts=off" to be posted');
}
await page.evaluate(() => {
  window.__setSettings({ disableAllShortcuts: false, disabledShortcuts: [] });
});

// Step C: Granular Shortcut Disabling
console.log('Testing Step C: Granular Shortcut Disabling...');
// Disable only "toggle-edit"
await page.click('button[data-shortcut-toggle="toggle-edit"][data-value="disable"]', { force: true });
const disableToggleEditMsg = await page.evaluate(() => {
  return window.__messages.includes('set-setting:disable-shortcut=toggle-edit');
});
if (!disableToggleEditMsg) {
  throw new Error('Expected "set-setting:disable-shortcut=toggle-edit" to be posted');
}

await page.evaluate(() => {
  window.__setSettings({ disableAllShortcuts: false, disabledShortcuts: ['toggle-edit'] });
});

// Close settings popover so keydowns target page
await page.click('#btn-settings', { force: true });

// Press Ctrl+E -> must NOT enter edit mode
await page.keyboard.press('Control+e');
const isEditingWithToggleDisabled = await page.evaluate(() => {
  return document.body.classList.contains('editing');
});
if (isEditingWithToggleDisabled) {
  throw new Error('Expected Ctrl+E to be suppressed when toggle-edit is disabled');
}

// But other shortcut (Ctrl+F) should still work!
await page.keyboard.press('Control+f');
const isFindingOpen = await page.evaluate(() => {
  return document.body.classList.contains('finding');
});
if (!isFindingOpen) {
  throw new Error('Expected Ctrl+F to still work when only toggle-edit is disabled');
}
// Close findbar
await page.keyboard.press('Escape');

// Re-open settings and re-enable toggle-edit
await page.click('#btn-settings', { force: true });
await page.click('.settings-tab-btn[data-settings-tab="shortcuts"]', { force: true });
await page.click('button[data-shortcut-toggle="toggle-edit"][data-value="enable"]', { force: true });
const enableToggleEditMsg = await page.evaluate(() => {
  return window.__messages.includes('set-setting:enable-shortcut=toggle-edit');
});
if (!enableToggleEditMsg) {
  throw new Error('Expected "set-setting:enable-shortcut=toggle-edit" to be posted');
}
await page.evaluate(() => {
  window.__setSettings({ disableAllShortcuts: false, disabledShortcuts: [] });
});

// Close settings
await page.click('#btn-settings', { force: true });

// Now Ctrl+E should enter edit mode
await page.keyboard.press('Control+e');
const isEditingNow = await page.evaluate(() => {
  return document.body.classList.contains('editing');
});
if (!isEditingNow) {
  throw new Error('Expected Ctrl+E to enter edit mode when toggle-edit is re-enabled');
}

// Step D: IME Composition & Escape Protection (Root cause test)
console.log('Testing Step D: IME Composition & Escape Protection...');
// In edit mode, trigger keydown Escape with isComposing = true (Chinese/CJK IME dismissing candidate window)
await page.evaluate(() => {
  const event = new KeyboardEvent('keydown', {
    key: 'Escape',
    code: 'Escape',
    bubbles: true,
    cancelable: true,
  });
  Object.defineProperty(event, 'isComposing', { value: true });
  document.dispatchEvent(event);
});

const isStillEditingAfterImeEscape = await page.evaluate(() => {
  return document.body.classList.contains('editing');
});
if (!isStillEditingAfterImeEscape) {
  throw new Error('IME candidate dismissal with Escape must NOT leave edit mode!');
}

// Also test keyCode 229
await page.evaluate(() => {
  const event = new KeyboardEvent('keydown', {
    key: 'Process',
    keyCode: 229,
    bubbles: true,
    cancelable: true,
  });
  document.dispatchEvent(event);
});
const isStillEditingAfter229 = await page.evaluate(() => {
  return document.body.classList.contains('editing');
});
if (!isStillEditingAfter229) {
  throw new Error('IME composition keyCode 229 must NOT leave edit mode!');
}

// Step E: Disable Escape shortcut granularly
console.log('Testing Step E: Disable Escape shortcut granularly...');
await page.evaluate(() => {
  window.__setSettings({ disableAllShortcuts: false, disabledShortcuts: ['escape'] });
});
// Plain Escape keydown should NOT leave edit mode now
await page.keyboard.press('Escape');
const isStillEditingAfterDisabledEscape = await page.evaluate(() => {
  return document.body.classList.contains('editing');
});
if (!isStillEditingAfterDisabledEscape) {
  throw new Error('Escape shortcut disabled must NOT leave edit mode!');
}

// Re-enable escape shortcut and normal Escape leaves edit mode
await page.evaluate(() => {
  window.__setSettings({ disableAllShortcuts: false, disabledShortcuts: [] });
});
await page.keyboard.press('Escape');
const isNotEditingAfterEscape = await page.evaluate(() => {
  return !document.body.classList.contains('editing');
});
if (!isNotEditingAfterEscape) {
  throw new Error('Escape shortcut re-enabled should leave edit mode');
}

// Step F: Logs UI & Uncaught Error Forwarder
console.log('Testing Step F: Logs UI & Uncaught Error Forwarder...');
// Open settings popover
await page.click('#btn-settings', { force: true });
// Switch back to General tab
await page.click('.settings-tab-btn[data-settings-tab="general"]', { force: true });

// Click Open Log button
await page.click('#btn-open-log', { force: true });
const openLogMsgSent = await page.evaluate(() => {
  return window.__messages.includes('open-log');
});
if (!openLogMsgSent) {
  throw new Error('Expected "open-log" message to be posted');
}

// Click Clear Log button
await page.click('#btn-clear-log', { force: true });
const clearLogMsgSent = await page.evaluate(() => {
  return window.__messages.includes('clear-log');
});
if (!clearLogMsgSent) {
  throw new Error('Expected "clear-log" message to be posted');
}

// Dispatch an uncaught error on window
await page.evaluate(() => {
  window.dispatchEvent(new ErrorEvent('error', {
    message: 'Test frontend crash simulated',
    filename: 'page.js',
    lineno: 42,
    colno: 7,
  }));
});

const errorLogged = await page.evaluate(() => {
  return window.__messages.some((msg) => msg.startsWith('log-error:') && msg.includes('Test frontend crash simulated'));
});
if (!errorLogged) {
  throw new Error('Expected uncaught error to be forwarded to IPC via log-error');
}

await browser.close();
console.log('All shortcut settings and logging checks passed successfully!');
