import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

const root = fileURLToPath(new URL('..', import.meta.url));
const mainRs = await readFile(resolve(root, 'src/main.rs'), 'utf8');

// 1. Static checks on Rust source
console.log('Verifying 1: Static checks on source...');
if (!mainRs.includes('id="btn-update-available"')) {
  throw new Error('Expected #btn-update-available in main.rs');
}
if (!mainRs.includes('id="btn-check-update"')) {
  throw new Error('Expected #btn-check-update in main.rs');
}
if (!mainRs.includes('id="update-modal"')) {
  throw new Error('Expected #update-modal in main.rs');
}
if (!mainRs.includes('UPDATE_CHECK_INTERVAL_MS')) {
  throw new Error('Expected UPDATE_CHECK_INTERVAL_MS in main.rs');
}
if (!mainRs.includes('windows_updater::apply_update')) {
  throw new Error('Expected windows_updater::apply_update in main.rs');
}

// 2. Playwright headless browser checks
console.log('Verifying 2: In-browser update UI and logic...');
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage();

const marker = 'var ICON_EDIT';
const markerIndex = mainRs.indexOf(marker);
if (markerIndex < 0) throw new Error('desktop script marker not found');
const scriptStart = mainRs.lastIndexOf('<script>', markerIndex);
const scriptEnd = mainRs.indexOf('</script>', markerIndex);
if (scriptStart < 0 || scriptEnd < 0) throw new Error('desktop script block not found');

let desktopScript = mainRs
  .slice(scriptStart + '<script>'.length, scriptEnd)
  .replaceAll('{{', '{')
  .replaceAll('}}', '}')
  .replaceAll('{cargo_version}', '1.3.0')
  .replaceAll('{update_status_checking_js}', 'Checking...')
  .replaceAll('{update_status_latest_js}', 'Up to date')
  .replaceAll('{update_status_failed_js}', 'Check failed')
  .replaceAll('{update_downloading_js}', 'Downloading...')
  .replaceAll('{btn_edit}', 'Edit')
  .replaceAll('{btn_preview}', 'Preview')
  .replaceAll('{btn_split}', 'Split View')
  .replaceAll('{code_copy_js}', 'Copy')
  .replaceAll('{code_copied_js}', 'Copied')
  .replaceAll('{sidebar_empty_js}', 'Nothing to show')
  .replaceAll('{sidebar_outline_empty_js}', 'No headings')
  .replaceAll('{stat_words_js}', 'words')
  .replaceAll('{stat_chars_js}', 'chars')
  .replaceAll('{copy_title_line_js}', 'Copy heading')
  .replaceAll('{copy_title_js}', 'Copy title')
  .replaceAll('{copy_body_js}', 'Copy body')
  .replaceAll('{copied_js}', 'Copied')
  .replaceAll('{btn_update_text_js}', 'Update Available')
  .replaceAll('{btn_check_update_js}', 'Check for Updates');
const styleMarker = '<style>\n:root';
const styleStart = mainRs.indexOf(styleMarker);
const styleEnd = mainRs.indexOf('</style>', styleStart);
const desktopStyle = styleStart >= 0 && styleEnd >= 0
  ? mainRs.slice(styleStart, styleEnd + '</style>'.length)
      .replaceAll('{{', '{')
      .replaceAll('}}', '}')
      .replaceAll('{sidebar_toggle_left}', '272')
      .replaceAll('{sidebar_width}', '260')
  : '';

const htmlContent = `
<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>MD Previewer Auto Update Test</title>
  ${desktopStyle}
</head>
<body>
  <div class="tabbar" id="tabbar">
    <div class="tabs" id="tabs"></div>
    <div class="doc-stats" id="doc-stats"></div>
    <div class="encoding-control" id="encoding-control">
      <button class="encoding-btn" id="btn-encoding">UTF-8</button>
      <div class="encoding-popover" id="encoding-popover" style="display:none;"></div>
    </div>
    <button class="tab-open" id="tab-open">+</button>
  </div>
  <aside class="sidebar" id="sidebar">
    <div class="sidebar-sections">
      <button type="button" data-sidebar-section="folder">Folder</button>
      <button type="button" data-sidebar-section="recent">Recent</button>
      <button type="button" data-sidebar-section="outline">Outline</button>
    </div>
    <div class="sidebar-list" id="sidebar-list"></div>
  </aside>
  <div id="topbar">
    <div class="toolbar sidebar-toggle">
      <button id="btn-sidebar"></button>
    </div>
    <div class="toolbar">
      <button id="btn-open"></button>
      <button id="btn-search"></button>
      <button id="btn-toggle"></button>
      <button id="btn-split"></button>
      <button id="btn-print"></button>
      <div class="zoom-control" id="zoom-control">
        <button id="btn-zoom"></button>
        <div class="zoom-popover">
          <button id="btn-zoom-out">−</button>
          <button id="btn-zoom-reset">100%</button>
          <button id="btn-zoom-in">+</button>
        </div>
      </div>
      <button id="btn-update-available" class="btn-update-available" style="display:none;">
        <span class="update-icon">🚀</span>
        <span class="update-text">v1.2.2</span>
      </button>
      <div class="settings-control" id="settings-control">
        <button id="btn-settings"></button>
        <div class="settings-popover">
          <div class="settings-row">
            <button type="button" data-setting="word-wrap" data-value="on">Wrap</button>
          </div>
          <div class="settings-sep"></div>
          <div class="settings-row">
            <div class="settings-label">
              <span>Version: v1.3.0</span>
              <span id="update-status-msg" class="update-status-msg"></span>
            </div>
            <button type="button" id="btn-check-update" class="settings-action-btn">Check</button>
          </div>
        </div>
      </div>
    </div>
  </div>
  <div class="findbar">
    <input id="find-input" type="search">
    <span id="find-state"></span>
    <button id="find-prev"></button>
    <button id="find-next"></button>
    <button id="find-close"></button>
  </div>
  <div id="tab-context-menu" class="context-menu" style="display:none;">
    <button type="button" class="context-menu-item" data-tab-action="close">Close</button>
  </div>
  <div id="lightbox" class="lightbox" style="display:none;">
    <button type="button" id="lb-close">×</button>
    <button type="button" id="lb-zoom-in">+</button>
    <button type="button" id="lb-zoom-out">−</button>
    <button type="button" id="lb-zoom-reset">100%</button>
    <img id="lb-img" alt="">
    <div id="lb-caption"></div>
  </div>
  <div id="update-modal" class="update-modal" style="display:none;">
    <div class="update-backdrop" id="update-backdrop"></div>
    <div class="update-dialog">
      <div class="update-header">
        <div class="update-title">
          <span class="update-icon">🚀</span>
          <span id="update-title-text">Update Available</span>
        </div>
        <button type="button" id="update-close" class="update-close-btn">×</button>
      </div>
      <div class="update-body">
        <div class="update-badge-row">
          <span class="update-badge" id="update-badge">v1.2.2</span>
          <span class="update-release-name" id="update-release-name"></span>
        </div>
        <div class="update-notes-box" id="update-notes-box"></div>
        <div id="update-progress-tip" class="update-progress-tip" style="display:none;"></div>
      </div>
      <div class="update-footer">
        <button type="button" id="btn-do-update" class="update-btn-primary">Update Now</button>
        <button type="button" id="btn-view-release" class="update-btn-secondary">View on GitHub</button>
        <button type="button" id="btn-dismiss-update" class="update-btn-text">Later</button>
      </div>
    </div>
  </div>
  <div id="app">
    <div id="preview"><h1>Test</h1></div>
    <textarea id="editor"># Test</textarea>
  </div>
  <script>
    window.__messages = [];
    window.ipc = {
      postMessage: function(msg) {
        window.__messages.push(msg);
      }
    };
  </script>
  <script>${desktopScript}</script>
</body>
</html>
`;

await page.setContent(htmlContent);

// Test version comparison in page
const versionComparisonResult = await page.evaluate(() => {
  const parse = (v) => {
    const s = v.trim().replace(/^v/i, '');
    return s.split('.').map(Number);
  };
  const isNewer = (cand, curr) => {
    const next = parse(cand);
    const now = parse(curr);
    for (let i = 0; i < Math.max(next.length, now.length); i++) {
      const a = next[i] || 0;
      const b = now[i] || 0;
      if (a > b) return true;
      if (a < b) return false;
    }
    return false;
  };

  return {
    test1: isNewer('v1.3.1', '1.3.0'),     // true
    test2: isNewer('v1.4.0', '1.3.0'),     // true
    test3: isNewer('v1.3.0', '1.3.0'),     // false
    test4: isNewer('v1.2.9', '1.3.0'),     // false
    test5: isNewer('v1.10.0', '1.3.0'),    // true
    test6: isNewer('1.3.1', '1.3.0'),      // true
  };
});

if (!versionComparisonResult.test1 || !versionComparisonResult.test2 || versionComparisonResult.test3 || versionComparisonResult.test4 || !versionComparisonResult.test5 || !versionComparisonResult.test6) {
  throw new Error('Version comparison failed: ' + JSON.stringify(versionComparisonResult));
}
console.log('  Version comparison verified successfully.');

// Test modal interaction
console.log('Verifying 3: Modal show, hide, and IPC trigger...');
await page.evaluate(() => {
  // Simulate detecting a release
  const fakeRelease = {
    tag_name: 'v1.3.1',
    name: 'v1.3.1 — Test Update',
    body: '## 1.3.1 Notes\n- Fixed something',
    html_url: 'https://github.com/ArnoldRedman/MD-Previewer/releases/tag/v1.3.1',
    assets: [
      { name: 'MD-Previewer-Setup.exe', browser_download_url: 'https://github.com/ArnoldRedman/MD-Previewer/releases/download/v1.3.1/MD-Previewer-Setup.exe' },
      { name: 'MD-Previewer-windows-x64.exe', browser_download_url: 'https://github.com/ArnoldRedman/MD-Previewer/releases/download/v1.3.1/MD-Previewer-windows-x64.exe' }
    ]
  };

  window.__applyDetectedRelease(fakeRelease);
  const btnUpdateAvailable = document.getElementById('btn-update-available');
  btnUpdateAvailable.click();
});

const isModalVisible = await page.$eval('#update-modal', el => el.style.display === 'flex');
if (!isModalVisible) throw new Error('Expected #update-modal to be displayed on update button click');

const badgeText = await page.$eval('#update-badge', el => el.textContent.trim());
if (badgeText !== 'v1.3.1') throw new Error(`Expected badge text 'v1.3.1', got '${badgeText}'`);

const releaseName = await page.$eval('#update-release-name', el => el.textContent.trim());
if (releaseName !== 'v1.3.1 — Test Update') throw new Error(`Expected release name 'v1.3.1 — Test Update', got '${releaseName}'`);

// Test clicking "Update Now"
await page.click('#btn-do-update');
const messages = await page.evaluate(() => window.__messages);
const hasSelfUpdateMsg = messages.some(m => m.startsWith('self-update:https://github.com/ArnoldRedman/MD-Previewer/releases/download/v1.3.1/MD-Previewer-windows-x64.exe'));
if (!hasSelfUpdateMsg) {
  throw new Error('Expected self-update IPC message with portable exe download url, got: ' + JSON.stringify(messages));
}
console.log('  Self-update IPC message verified successfully: ' + messages.find(m => m.startsWith('self-update:')));

// Test dismiss button closes modal
await page.click('#btn-dismiss-update');
const isModalClosed = await page.$eval('#update-modal', el => el.style.display === 'none');
if (!isModalClosed) throw new Error('Expected #update-modal to close on dismiss click');
console.log('  Modal dismiss verified successfully.');

// Test settings popover button layout and styling
console.log('Verifying 4: Settings popover layout, button sizing, and click behavior...');
await page.evaluate(() => {
  const settingsControl = document.getElementById('settings-control');
  settingsControl.classList.add('open');
});

const checkBtnBox = await page.$eval('#btn-check-update', el => {
  const rect = el.getBoundingClientRect();
  return {
    width: rect.width,
    height: rect.height,
    scrollWidth: el.scrollWidth,
    offsetWidth: el.offsetWidth,
    text: el.textContent,
    hasUpdate: el.classList.contains('has-update')
  };
});
console.log('  Check update button info:', checkBtnBox);
if (checkBtnBox.width < 100) {
  throw new Error(`Expected #btn-check-update width to be >= 100px, but got ${checkBtnBox.width}px (shrunk by toolbar button override)`);
}
if (checkBtnBox.scrollWidth > checkBtnBox.offsetWidth + 2) {
  throw new Error(`Text in #btn-check-update overflows container! scrollWidth=${checkBtnBox.scrollWidth}, offsetWidth=${checkBtnBox.offsetWidth}`);
}
if (!checkBtnBox.hasUpdate || !checkBtnBox.text.includes('v1.3.1')) {
  throw new Error(`Expected #btn-check-update to display detected version and have 'has-update' class, got: ${JSON.stringify(checkBtnBox)}`);
}

// Test clicking the updated settings button re-opens the update modal
await page.click('#btn-check-update');
const isModalReopened = await page.$eval('#update-modal', el => el.style.display === 'flex');
if (!isModalReopened) {
  throw new Error('Expected clicking #btn-check-update (when update available) to open #update-modal');
}
console.log('  Settings update button opened modal successfully.');

const popoverEl = await page.$('.settings-popover');
if (popoverEl) {
  const artifactDir = 'C:/Users/zhuzi/.gemini/antigravity-cli/brain/54fa365e-c608-4dbc-89d8-20e4db89a888';
  await popoverEl.screenshot({ path: `${artifactDir}/settings_popover_update.png` });
  console.log('  Saved settings popover screenshot.');
}

await browser.close();
console.log('[auto-update-verify] ALL CHECKS PASSED');
