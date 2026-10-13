// 标签拖动验收：使用真实模板，验证未保存正文、取消和接收确认的协议
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { configJson, desktopCss, desktopScript, pageHtml } from './desktop-page.mjs';

const replacements = {
  sidebar_width: '260', css: desktopCss, config_json: configJson(),
  needs_math: 'false', needs_mermaid: 'false',
  js: `window.__messages = []; window.ipc = { postMessage: function(message) { window.__messages.push(message); } };\n${desktopScript}`,
};
const html = pageHtml.replace(/\{\{(\w+)\}\}/g, (_, key) => replacements[key] ?? '');
const tabs = [
  { id: 1, name: '编辑中.md', path: 'D:/编辑中.md', active: true, dirty: false, missing: false },
  { id: 2, name: '另一个.md', path: 'D:/另一个.md', active: false, dirty: false, missing: false },
];
const browser = await chromium.launch();
try {
  for (const width of [1100, 390]) {
    const page = await browser.newPage({ viewport: { width, height: 780 } });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.setContent(html);
    await page.evaluate(tabs => {
      window.__setTabs(tabs);
      window.__setContent('<p>原文</p>', '原文', '', false, false, null, false, null);
      window.__enableTabDrag();
      window.__mdPreviewerEnterEdit();
      window.__mdPreviewerPauseAutosave();
    }, tabs);
    const editor = page.locator('#editor');
    await editor.fill('未保存😀\n第二行');
    await editor.evaluate(el => el.setSelectionRange(3, 5));
    // 小幅移动仍是点击，不应起拖
    const box = await page.locator('[data-tab-id="1"]').boundingBox();
    await page.mouse.move(box.x + 30, box.y + 20);
    await page.mouse.down();
    await page.mouse.move(box.x + 33, box.y + 21);
    assert.equal(await page.evaluate(() => window.__messages.some(m => m.startsWith('tab-drag:'))), false);
    // 标签重建不会打断手势；越过阈值后正文只进入快照，不触发保存
    await page.evaluate(tabs => window.__setTabs(tabs), tabs);
    await page.mouse.move(box.x + 60, box.y + 30);
    const request = await page.evaluate(() => JSON.parse(window.__messages.find(m => m.startsWith('tab-drag:')).slice(9)));
    assert.equal(request.id, 1);
    assert.equal(request.snapshot.raw, '未保存😀\n第二行');
    assert.equal(request.snapshot.dirty, true);
    assert.equal(request.snapshot.editMode, true);
    assert.equal(request.snapshot.autosavePaused, true);
    assert.equal(request.snapshot.selectionStart, 3);
    assert.equal(request.snapshot.selectionEnd, 5);
    assert.equal(await editor.evaluate(el => el.readOnly), true);
    await page.keyboard.press('Escape');
    assert.equal(await page.evaluate(() => window.__messages.includes('tab-drag-cancel')), true);
    await page.mouse.up();
    await page.evaluate(() => window.__endTabTransfer());
    assert.equal(await editor.inputValue(), '未保存😀\n第二行');
    assert.equal(await editor.evaluate(el => el.readOnly), false);
    // 接收方先提供自身未保存正文，宿主保存成功后才切换文档
    await page.evaluate(() => window.__prepareTabReceive(7));
    const prepared = await page.evaluate(() => JSON.parse(window.__messages.find(m => m.startsWith('tab-receive-ready:')).split(':').slice(1).join(':')));
    assert.deepEqual(prepared, { token: 7, activeId: 1, content: '未保存😀\n第二行' });
    await page.evaluate(() => {
      window.__markSaved('未保存😀\n第二行');
      window.__setTabs([{ id: 3, name: '转入.md', path: 'D:/转入.md', active: true, dirty: true }]);
      window.__setContent('<p>来自另一个窗口</p>', '来自另一个窗口', '', false, false, null, false, null);
      window.__restoreTransferredTab({ dirty: true, editMode: true, autosavePaused: true, progress: 0, zoom: 125, selectionStart: 1, selectionEnd: 4 }, 7, 999);
    });
    assert.equal(await page.evaluate(() => window.__messages.includes('tab-received:7')), false);
    await page.evaluate(() => {
      window.__restoreTransferredTab({ dirty: true, editMode: true, autosavePaused: true, progress: 0, zoom: 125, selectionStart: 1, selectionEnd: 4 }, 7, 3);
    });
    assert.equal(await editor.inputValue(), '来自另一个窗口');
    assert.equal(await editor.evaluate(el => el.selectionStart), 1);
    assert.equal(await editor.evaluate(el => el.selectionEnd), 4);
    assert.equal(await page.evaluate(() => window.__messages.at(-1)), 'tab-received:7');
    assert.equal(await page.locator('body').evaluate(el => el.classList.contains('tab-transferring')), true);
    await page.evaluate(() => window.__endTabTransfer());
    // 外部修改触发的自动保存暂停必须跟随正文，超过原来的 700ms 保存延时仍不能覆盖磁盘
    await page.waitForTimeout(800);
    assert.equal(await page.evaluate(() => window.__messages.some(m => m === 'save:来自另一个窗口')), false);
    const geometry = await page.evaluate(() => ({
      viewport: innerWidth, width: document.documentElement.scrollWidth,
      tabbarBottom: document.querySelector('.tabbar').getBoundingClientRect().bottom,
      topbarTop: document.querySelector('#topbar').getBoundingClientRect().top,
    }));
    assert.ok(geometry.width <= geometry.viewport, JSON.stringify(geometry));
    assert.ok(geometry.topbarTop >= geometry.tabbarBottom, JSON.stringify(geometry));
    assert.deepEqual(errors, []);
    await page.close();
  }
  console.log('PASS: tab drag threshold, DOM rebuild, dirty snapshot, cancellation, receive handshake, desktop/mobile layout');
} finally {
  await browser.close();
}
