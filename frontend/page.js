(function(){
  // 渲染期由 Rust 注入的界面文案与特性开关，全部集中在这里读取
  var CFG = window.__mdPreviewerConfig;
	  var ICON_EDIT = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>';
	  var ICON_VIEW = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>';
	  var ICON_OPEN = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 14 1.45-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.55 6A2 2 0 0 1 18.45 20H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"/></svg>';
	  var ICON_SEARCH = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>';
	  var ICON_PRINT = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 6 2 18 2 18 9"/><path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"/><rect x="6" y="14" width="12" height="8"/></svg>';
	  var ICON_ZOOM = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/><path d="M8 11h6"/><path d="M11 8v6"/></svg>';
	  var ICON_SIDEBAR = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18"/></svg>';
	  var ICON_SETTINGS = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>';
	  var ICON_UP = '<svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m18 15-6-6-6 6"/></svg>';
	  var ICON_DOWN = '<svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9 6 6 6-6"/></svg>';
	  var ICON_CLOSE = '<svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>';
	  var ICON_SPLIT = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="12" y1="3" x2="12" y2="21"/></svg>';
	  var L_EDIT = CFG.btnEditJs, L_VIEW = CFG.btnPreviewJs;
	  var L_COPY = CFG.codeCopyJs, L_COPIED = CFG.codeCopiedJs;
	  var SIDEBAR_OUTLINE_EMPTY = CFG.sidebarOutlineEmptyJs;

	  var btnOpen = document.getElementById('btn-open');
	  var btnSearch = document.getElementById('btn-search');
	  var btnToggle = document.getElementById('btn-toggle');
	  var btnSplit = document.getElementById('btn-split');
	  var btnPrint = document.getElementById('btn-print');
	  var btnZoom = document.getElementById('btn-zoom');
	  var btnZoomOut = document.getElementById('btn-zoom-out');
	  var btnZoomReset = document.getElementById('btn-zoom-reset');
	  var btnZoomIn = document.getElementById('btn-zoom-in');
	  var zoomControl = document.getElementById('zoom-control');
	  var btnSettings = document.getElementById('btn-settings');
	  var settingsControl = document.getElementById('settings-control');
	  var btnSidebar = document.getElementById('btn-sidebar');
	  var sidebarEl = document.getElementById('sidebar');
	  var sidebarList = document.getElementById('sidebar-list');
	  var sidebarFooter = document.getElementById('sidebar-footer');
	  var sidebarClearRecent = document.getElementById('sidebar-clear-recent');
	  var sidebarTooltip = document.getElementById('sidebar-tooltip');
	  var sidebarTooltipTimer = 0;
	  var recentContextMenu = document.getElementById('recent-context-menu');
	  var recentContextPath = '';
	  var sidebarData = { folder: [], recent: [] };
	  var sidebarSection = 'folder';
	  var SIDEBAR_EMPTY = CFG.sidebarEmptyJs;
	  var findInput = document.getElementById('find-input');
	  var findState = document.getElementById('find-state');
	  var findPrev = document.getElementById('find-prev');
	  var findNext = document.getElementById('find-next');
	  var findClose = document.getElementById('find-close');
	  var tabsEl = document.getElementById('tabs');
	  var docStats = document.getElementById('doc-stats');
	  var btnEncoding = document.getElementById('btn-encoding');
	  var encodingPopover = document.getElementById('encoding-popover');
	  var currentEncoding = (btnEncoding && btnEncoding.textContent.trim()) || 'UTF-8';
	  var tabOpen = document.getElementById('tab-open');
	  var ta = document.getElementById('editor');
	  var previewEl = document.getElementById('preview');
	  var tabContextMenu = document.getElementById('tab-context-menu');
	  var contextMenuTabId = null;
	  var contextMenuTabPath = '';
	  var lightbox = document.getElementById('lightbox');
	  var lbImg = document.getElementById('lb-img');
	  var lbCaption = document.getElementById('lb-caption');
	  var lbClose = document.getElementById('lb-close');
	  var lbZoomIn = document.getElementById('lb-zoom-in');
	  var lbZoomOut = document.getElementById('lb-zoom-out');
	  var lbZoomReset = document.getElementById('lb-zoom-reset');
	  var lbScale = 1.0;
	  var lbTranslateX = 0;
	  var lbTranslateY = 0;
	  var lbIsDragging = false;
	  var lbStartX = 0;
	  var lbStartY = 0;
	  var pendingLiveRenderTimer = 0;
	  var liveRenderInFlight = false;
	  var LIVE_RENDER_DEBOUNCE_MS = 150;
	  var TOOLTIP_DELAY_MS = 300;
	  var COPY_FLASH_MS = 1500;
	  var AUTHOR_FLASH_MS = 1200;
	  var OVERLAY_MARGIN_PX = 8;
	  var OUTLINE_ACTIVE_OFFSET_PX = 120;
	  // 事件委托统一取命中元素；e.target 可能是文本节点或没有 closest 的对象
	  function hit(e, selector) {
	    var target = e.target;
	    return target && target.closest ? target.closest(selector) : null;
	  }
	  var dirty = false;
	  var activeTabId = 0;
	  var pendingAutosaveTimer = 0;
	  var autosavePaused = false;
	  var AUTOSAVE_DEBOUNCE_MS = 700;
	  var composingFind = false;
	  var pendingFindTimer = 0;
	  var FIND_DEBOUNCE_MS = 300;
	  var findHits = [];
	  var currentFindHit = -1;
	  var lastFindQuery = '';
	  var STAT_WORDS = CFG.statWordsJs;
	  var STAT_CHARS = CFG.statCharsJs;
	  var ZOOM_STORAGE_KEY = 'md-previewer-content-zoom-v1';
	  var ZOOM_MIN = 70;
	  var ZOOM_MAX = 200;
	  var ZOOM_STEP = 10;
	  var zoomPercent = 100;

	  btnOpen.innerHTML = ICON_OPEN;
	  btnSearch.innerHTML = ICON_SEARCH;
	  btnToggle.innerHTML = ICON_EDIT;
	  if (btnSplit) btnSplit.innerHTML = ICON_SPLIT;
	  btnPrint.innerHTML = ICON_PRINT;
	  btnZoom.innerHTML = ICON_ZOOM;
	  btnSettings.innerHTML = ICON_SETTINGS;
	  btnSidebar.innerHTML = ICON_SIDEBAR;
	  findPrev.innerHTML = ICON_UP;
	  findNext.innerHTML = ICON_DOWN;
	  findClose.innerHTML = ICON_CLOSE;

  function inEdit() { return document.body.classList.contains('editing'); }
  function textSegments(text) {
    if (typeof Intl !== 'undefined' && Intl.Segmenter) {
      var segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });
      return Array.from(segmenter.segment(text), function(item) { return item.segment; });
    }
    return Array.from(text);
  }
  function updateDocumentStats(raw) {
    var segments = textSegments(String(raw || ''));
    var words = segments.reduce(function(total, segment) {
      return total + (/^\s+$/u.test(segment) ? 0 : 1);
    }, 0);
    var number = new Intl.NumberFormat().format;
    docStats.textContent =
      number(words) + ' ' + STAT_WORDS + ' · ' + number(segments.length) + ' ' + STAT_CHARS;
  }
  function loadZoomPercent() {
    try {
      var stored = Number(localStorage.getItem(ZOOM_STORAGE_KEY));
      if (Number.isFinite(stored) && stored >= ZOOM_MIN && stored <= ZOOM_MAX) return stored;
    } catch (_) {}
    return 100;
  }
  function applyZoom(percent, persist) {
    zoomPercent = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, percent));
    document.documentElement.style.setProperty('--content-scale', String(zoomPercent / 100));
    btnZoomReset.textContent = zoomPercent + '%';
    if (persist) {
      try { localStorage.setItem(ZOOM_STORAGE_KEY, String(zoomPercent)); } catch (_) {}
    }
    if (inEdit()) autoResize();
  }
  function changeZoom(delta) {
    applyZoom(zoomPercent + delta, true);
  }
  function currentScrollProgress() {
    var root = document.documentElement;
    var max = Math.max(root.scrollHeight - window.innerHeight, 0);
    var y = window.scrollY || root.scrollTop || 0;
    return max > 0 ? Math.min(1, Math.max(0, y / max)) : 0;
  }
  function restoreScrollProgress(progress) {
    function restore() {
      var max = Math.max(document.documentElement.scrollHeight - window.innerHeight, 0);
      window.scrollTo(window.scrollX || 0, max * progress);
    }
    restore();
    requestAnimationFrame(function() {
      restore();
      requestAnimationFrame(restore);
    });
  }
  applyZoom(loadZoomPercent(), false);
  updateDocumentStats(ta.value);
  function setEncodingUi(enc) {
    if (!enc) return;
    currentEncoding = enc;
    if (btnEncoding) btnEncoding.textContent = enc;
    if (!encodingPopover) return;
    var options = encodingPopover.querySelectorAll('[data-encoding]');
    for (var i = 0; i < options.length; i++) {
      options[i].classList.toggle('active', options[i].getAttribute('data-encoding') === enc);
    }
    // 转换组里当前编码没有意义，置灰防止误点
    var converts = encodingPopover.querySelectorAll('[data-convert-encoding]');
    for (var j = 0; j < converts.length; j++) {
      converts[j].disabled = converts[j].getAttribute('data-convert-encoding') === enc;
    }
  }
  setEncodingUi(currentEncoding);
  window.__setEncoding = setEncodingUi;
  if (btnEncoding && encodingPopover) {
    btnEncoding.addEventListener('click', function() {
      var isOpen = encodingPopover.style.display !== 'none';
      encodingPopover.style.display = isOpen ? 'none' : 'block';
    });
    encodingPopover.addEventListener('click', function(e) {
      var convert = e.target.closest('[data-convert-encoding]');
      if (convert) {
        encodingPopover.style.display = 'none';
        convertEncoding(convert.getAttribute('data-convert-encoding'));
        return;
      }
      if (e.target.closest('#btn-save-as')) {
        encodingPopover.style.display = 'none';
        saveAs();
        return;
      }
      var opt = e.target.closest('[data-encoding]');
      if (opt) {
        var enc = opt.getAttribute('data-encoding');
        encodingPopover.style.display = 'none';
        if (enc && enc !== currentEncoding && window.ipc) {
          // 先把未保存的编辑写回再切换编码重读；Rust 按消息顺序处理，保存一定先落盘
          if (dirty) save();
          window.ipc.postMessage('set-encoding:' + enc);
        }
      }
    });
  }
  document.addEventListener('contextmenu', function(e) {
    if (!inEdit() || e.target !== ta) e.preventDefault();
  });
  function setDirty(d) {
    if (dirty === d) return;
    dirty = d;
    window.ipc.postMessage(d ? 'dirty:1' : 'dirty:0');
  }
	  function cancelPendingAutosave() {
	    if (!pendingAutosaveTimer) return;
	    clearTimeout(pendingAutosaveTimer);
	    pendingAutosaveTimer = 0;
	  }
	  function save() {
	    cancelPendingAutosave();
	    if (!dirty) return;
	    window.ipc.postMessage('save:' + ta.value);
	  }
	  // 转码与另存为都把编辑器全文随消息带上，未保存的编辑不会丢；先取消待触发的自动保存，避免两次写盘交错
	  function convertEncoding(enc) {
	    cancelPendingAutosave();
	    window.ipc.postMessage('convert-encoding:' + enc + '\n' + ta.value);
	  }
	  function saveAs() {
	    if (document.body.classList.contains('empty')) return;
	    cancelPendingAutosave();
	    window.ipc.postMessage('save-as\n' + ta.value);
	  }
	  function scheduleAutosave() {
	    cancelPendingAutosave();
	    if (autosavePaused) return;
	    pendingAutosaveTimer = setTimeout(function() {
	      pendingAutosaveTimer = 0;
	      if (dirty) save();
	    }, AUTOSAVE_DEBOUNCE_MS);
	  }
	  // 关窗前由 Rust 调用：没有脏内容时也要回一句，否则 Rust 会一直等保存结果、窗口关不掉
	  window.__mdPreviewerSave = function() {
	    if (dirty) {
	      save();
	      return;
	    }
	    cancelPendingAutosave();
	    window.ipc.postMessage('save-skipped');
	  };
	  function requestTabAction(action, id) {
	    cancelPendingAutosave();
	    cancelLiveRender();
	    var message = 'tab-action:' + action + ':' + id;
	    if (dirty) message += '\n' + ta.value;
	    window.ipc.postMessage(message);
	  }
	  function openFile() {
	    if (inEdit()) leaveEdit();
	    window.ipc.postMessage('open');
	  }
	  window.__mdPreviewerOpenFile = openFile;
	  function newFile() {
	    if (inEdit()) leaveEdit();
	    window.ipc.postMessage('new-file');
	  }
	  window.__mdPreviewerNewFile = newFile;
	  function showFind() {
	    if (document.body.classList.contains('empty')) return;
	    if (inEdit()) return;
	    document.body.classList.add('finding');
	    setTimeout(function(){ findInput.focus(); findInput.select(); }, 0);
	  }
	  window.__mdPreviewerShowFind = showFind;
	  function hideFind() {
	    document.body.classList.remove('finding');
	    findInput.value = '';
	    clearFindHits();
	    if (pendingFindTimer) { clearTimeout(pendingFindTimer); pendingFindTimer = 0; }
	    var sel = window.getSelection && window.getSelection();
	    if (sel && sel.removeAllRanges) sel.removeAllRanges();
	  }
	  function updateFindState() {
	    findState.textContent = findHits.length ? (currentFindHit + 1) + '/' + findHits.length : '';
	  }
	  function clearFindHits() {
	    findHits.forEach(function(mark) {
	      var parent = mark.parentNode;
	      if (!parent) return;
	      parent.replaceChild(document.createTextNode(mark.textContent), mark);
	      parent.normalize();
	    });
	    findHits = [];
	    currentFindHit = -1;
	    lastFindQuery = '';
	    updateFindState();
	  }
	  function focusFindInput(selectionStart, selectionEnd) {
	    if (!document.body.classList.contains('finding')) return;
	    try {
	      findInput.focus({ preventScroll: true });
	    } catch (_) {
	      findInput.focus();
	    }
	    if (typeof selectionStart === 'number' && typeof selectionEnd === 'number') {
	      try { findInput.setSelectionRange(selectionStart, selectionEnd); } catch (_) {}
	    }
	  }
	  function restoreFindInput(selectionStart, selectionEnd) {
	    setTimeout(function() { focusFindInput(selectionStart, selectionEnd); }, 0);
	    requestAnimationFrame(function() {
	      focusFindInput(selectionStart, selectionEnd);
	      setTimeout(function() { focusFindInput(selectionStart, selectionEnd); }, 80);
	    });
	  }
	  function selectFindHit(index) {
	    if (!findHits.length) {
	      currentFindHit = -1;
	      updateFindState();
	      return;
	    }
	    if (currentFindHit >= 0 && findHits[currentFindHit]) {
	      findHits[currentFindHit].classList.remove('current');
	    }
	    currentFindHit = (index + findHits.length) % findHits.length;
	    var hit = findHits[currentFindHit];
	    hit.classList.add('current');
	    hit.scrollIntoView({ block: 'center', inline: 'nearest' });
	    updateFindState();
	  }
	  function runFindQuery(query) {
	    clearFindHits();
	    query = String(query || '').trim();
	    if (!query) return;
	    lastFindQuery = query;
	    var needle = query.toLowerCase();

	    var walker = document.createTreeWalker(previewEl, NodeFilter.SHOW_TEXT, {
	      acceptNode: function(node) {
	        if (!node.nodeValue || node.nodeValue.toLowerCase().indexOf(needle) < 0) {
	          return NodeFilter.FILTER_REJECT;
	        }
	        var parent = node.parentElement;
	        if (!parent || parent.closest('script,style,svg,mark.search-hit,.katex,.mdp-mermaid')) {
	          return NodeFilter.FILTER_REJECT;
	        }
	        return NodeFilter.FILTER_ACCEPT;
	      }
	    });
	    var nodes = [];
	    while (walker.nextNode()) nodes.push(walker.currentNode);
	    nodes.forEach(function(node) {
	      var text = node.nodeValue;
	      var lower = text.toLowerCase();
	      var fragment = document.createDocumentFragment();
	      var start = 0;
	      var index;
	      while ((index = lower.indexOf(needle, start)) >= 0) {
	        if (index > start) fragment.appendChild(document.createTextNode(text.slice(start, index)));
	        var mark = document.createElement('mark');
	        mark.className = 'search-hit';
	        mark.textContent = text.slice(index, index + query.length);
	        findHits.push(mark);
	        fragment.appendChild(mark);
	        start = index + query.length;
	      }
	      if (start < text.length) fragment.appendChild(document.createTextNode(text.slice(start)));
	      node.parentNode.replaceChild(fragment, node);
	    });
	    selectFindHit(0);
	  }
	  function runFind(backward) {
	    var q = findInput.value;
	    if (!q) { clearFindHits(); return; }
	    var hadFocus = document.activeElement === findInput;
	    var selectionStart = findInput.selectionStart;
	    var selectionEnd = findInput.selectionEnd;
	    var normalized = String(q || '').trim();
	    if (!normalized) {
	      clearFindHits();
	    } else if (normalized !== lastFindQuery) {
	      runFindQuery(normalized);
	    } else {
	      selectFindHit(currentFindHit + (backward ? -1 : 1));
	    }
	    if (hadFocus) restoreFindInput(selectionStart, selectionEnd);
	  }
	  function scheduleFind() {
	    if (pendingFindTimer) clearTimeout(pendingFindTimer);
	    pendingFindTimer = setTimeout(function() {
	      pendingFindTimer = 0;
	      if (!composingFind) runFindQuery(findInput.value);
	    }, FIND_DEBOUNCE_MS);
	  }
  // Grow textarea height to its content so the page (html) owns the sole
  // scrollbar; avoids the double-scrollbar you see if textarea keeps its
  // own internal scroll.
  function autoResize() {
    var x = window.scrollX || document.documentElement.scrollLeft || 0;
    var y = window.scrollY || document.documentElement.scrollTop || 0;
    ta.style.height = 'auto';
    var h = Math.max(ta.scrollHeight, window.innerHeight);
    ta.style.height = h + 'px';
    window.scrollTo(x, y);
  }
	  function enterEdit() {
	    // 空白页和文件缺失页没有可编辑的文档，进入编辑只会把输入保存到虚空
	    if (document.body.classList.contains('empty') || document.body.classList.contains('missing')) return;
	    hideFind();
	    var progress = currentScrollProgress();
	    document.body.classList.add('editing');
	    btnToggle.innerHTML = ICON_VIEW;
	    btnToggle.title = L_VIEW;
	    btnToggle.setAttribute('aria-label', L_VIEW);
	    if (document.body.classList.contains('split-view')) scheduleLiveRender(0);
	    autoResize();
	    try {
	      ta.focus({ preventScroll: true });
	    } catch (_) {
	      ta.focus();
	    }
	    restoreScrollProgress(progress);
	  }
  function leaveEdit() {
    var progress = currentScrollProgress();
    if (dirty) save();
    document.body.classList.remove('editing');
    btnToggle.innerHTML = ICON_EDIT;
    btnToggle.title = L_EDIT;
    btnToggle.setAttribute('aria-label', L_EDIT);
    cancelLiveRender();
    restoreScrollProgress(progress);
  }
  function toggleSplitView() {
    var split = !document.body.classList.contains('split-view');
    document.body.classList.toggle('split-view', split);
    if (btnSplit) btnSplit.setAttribute('aria-pressed', split ? 'true' : 'false');
    if (split) {
      scheduleLiveRender(0);
    }
  }
  function cancelLiveRender() {
    if (pendingLiveRenderTimer) {
      clearTimeout(pendingLiveRenderTimer);
      pendingLiveRenderTimer = 0;
    }
    liveRenderInFlight = false;
  }
  function scheduleLiveRender(delay) {
    if (!document.body.classList.contains('split-view')) return;
    if (pendingLiveRenderTimer) clearTimeout(pendingLiveRenderTimer);
    pendingLiveRenderTimer = setTimeout(function() {
      pendingLiveRenderTimer = 0;
      liveRenderInFlight = true;
      if (window.ipc) window.ipc.postMessage('render-preview:' + ta.value);
    }, typeof delay === 'number' ? delay : LIVE_RENDER_DEBOUNCE_MS);
  }
  // 只接受自己发起且仍在等待的渲染结果；切换标签或退出编辑后迟到的回包直接丢弃，不能盖掉新文档
  // 后两个参数是 Rust 侧的 KaTeX/Mermaid 开关，实时渲染不复用，保留签名便于后续接上
  window.__setLivePreview = function(html, _math, _mermaid) {
    if (!liveRenderInFlight || !inEdit() || !document.body.classList.contains('split-view')) return;
    liveRenderInFlight = false;
    previewEl.innerHTML = html;
    if (typeof hljs !== 'undefined') hljs.highlightAll();
    setupCodeBlockCopyButtons();
    if (sidebarSection === 'outline') renderSidebar();
  };
  window.__mdPreviewerToggleEdit = function() {
    if (inEdit()) leaveEdit(); else enterEdit();
  };
	window.__mdPreviewerEnterEdit = function() {
	  if (!inEdit()) enterEdit();
	};
	window.__mdPreviewerCloseActiveTab = function() {
	  if (activeTabId) requestTabAction('close', activeTabId);
	};

	  btnOpen.addEventListener('click', openFile);
	  tabOpen.addEventListener('click', newFile);
	  if (btnSplit) {
	    btnSplit.addEventListener('click', function() {
	      toggleSplitView();
	    });
	  }
	  btnSearch.addEventListener('click', showFind);
	  document.addEventListener('click', function(e) {
	    closeOverlaysOutside(e.target);
	    var closeTab = hit(e, '#tabs [data-close-tab], .missing-file [data-close-tab]');
	    if (closeTab) {
	      e.preventDefault();
	      e.stopPropagation();
	      requestTabAction('close', closeTab.getAttribute('data-close-tab'));
	      return;
	    }
	    var locateTab = hit(e, '.missing-file [data-locate-tab]');
	    if (locateTab) {
	      e.preventDefault();
	      window.ipc.postMessage('locate-tab:' + locateTab.getAttribute('data-locate-tab'));
	      return;
	    }
	    var tab = hit(e, '#tabs [data-tab-id]');
	    if (tab) {
	      e.preventDefault();
	      requestTabAction('activate', tab.getAttribute('data-tab-id'));
	      return;
	    }
	    var copyBtn = hit(e, '.code-copy-btn');
	    if (copyBtn) {
	      e.preventDefault();
	      e.stopPropagation();
	      var pre = copyBtn.closest('pre');
	      if (!pre) return;
	      var code = pre.querySelector('code');
	      var text = code ? code.innerText : pre.innerText;
	      if (!code && text.endsWith(copyBtn.innerText)) {
	        text = text.slice(0, text.length - copyBtn.innerText.length).trimEnd();
	      }
	      copyText(text, function() {
	        var orig = copyBtn.textContent;
	        copyBtn.textContent = L_COPIED;
	        copyBtn.classList.add('copied');
	        setTimeout(function() {
	          copyBtn.textContent = orig;
	          copyBtn.classList.remove('copied');
	        }, COPY_FLASH_MS);
	      });
	      return;
	    }
	    var img = hit(e, '#preview img');
	    if (img && !inEdit()) {
	      e.preventDefault();
	      openLightbox(img.src, img.alt || img.title || '');
	      return;
	    }
	    var openBtn = hit(e, '.empty [data-open-file]');
	    if (openBtn) {
	      e.preventDefault();
	      openFile();
	      return;
	    }
	    var recentBtn = hit(e, '.empty [data-recent-index]');
	    if (recentBtn) {
	      e.preventDefault();
	      window.ipc.postMessage('open-recent:' + recentBtn.getAttribute('data-recent-index'));
	    }
	  });
	  findInput.addEventListener('compositionstart', function() { composingFind = true; });
	  findInput.addEventListener('compositionend', function() { composingFind = false; scheduleFind(); });
	  findInput.addEventListener('input', function(e) {
	    if (composingFind || e.isComposing) return;
	    scheduleFind();
	  });
	  findInput.addEventListener('keydown', function(e) {
	    if (composingFind || e.isComposing) return;
	    if (e.key === 'Enter') { e.preventDefault(); runFind(e.shiftKey); }
	    if (e.key === 'Escape') { e.preventDefault(); hideFind(); }
	  });
	  findPrev.addEventListener('click', function() { runFind(true); });
	  findNext.addEventListener('click', function() { runFind(false); });
	  findClose.addEventListener('click', hideFind);

	  btnToggle.addEventListener('click', function() {
	    window.__mdPreviewerToggleEdit();
	  });
	  btnZoom.addEventListener('click', function() {
	    settingsControl.classList.remove('open');
	    zoomControl.classList.toggle('open');
	  });
	  btnSettings.addEventListener('click', function() {
	    zoomControl.classList.remove('open');
	    settingsControl.classList.toggle('open');
	  });
	  // 只上报点击，选中态一律等 Rust 存盘后通过 __setSettings 回显，避免界面和实际配置不一致
	  settingsControl.addEventListener('click', function(e) {
	    var btn = hit(e, '[data-setting]');
	    if (!btn) return;
	    e.preventDefault();
	    window.ipc.postMessage('set-setting:' + btn.getAttribute('data-setting') + '=' + btn.getAttribute('data-value'));
	  });
	  // 侧栏空状态提示条，大纲和文件列表共用同一份结构
  function emptyNote(text) {
    var el = document.createElement('div');
    el.className = 'sidebar-empty';
    el.textContent = text;
    return el;
  }
  function renderSidebar() {
	    var sections = sidebarEl.querySelectorAll('[data-sidebar-section]');
	    for (var s = 0; s < sections.length; s++) {
	      sections[s].setAttribute('aria-pressed', sections[s].getAttribute('data-sidebar-section') === sidebarSection ? 'true' : 'false');
	    }
	    sidebarList.textContent = '';
	    hideSidebarTooltip();
	    // 清空按钮只在最近打开分区且有记录时出现，空列表没有可清的内容
	    var hasRecent = sidebarSection === 'recent' && (sidebarData.recent || []).length > 0;
	    if (sidebarFooter) sidebarFooter.style.display = hasRecent ? 'block' : 'none';
	    if (sidebarSection === 'outline') {
	      var preview = previewEl;
	      var headings = preview ? preview.querySelectorAll('h1, h2, h3, h4, h5, h6') : [];
	      if (!headings || !headings.length) {
	        sidebarList.appendChild(emptyNote(SIDEBAR_OUTLINE_EMPTY));
	        return;
	      }
	      for (var i = 0; i < headings.length; i++) {
	        var h = headings[i];
	        if (!h.id) h.id = 'heading-' + i;
	        var level = parseInt(h.tagName.substring(1), 10) || 1;
	        var text = (h.innerText || h.textContent || '').replace(/\s+$/, '');
	        var authorActions = h.querySelector('.author-actions');
	        if (authorActions) {
	          text = text.replace(authorActions.innerText, '').trim();
	        }
	        var btn = document.createElement('button');
	        btn.type = 'button';
	        btn.className = 'sidebar-item outline-item outline-level-' + level;
	        btn.setAttribute('data-outline-id', h.id);
	        btn.title = text;
	        var name = document.createElement('span');
	        name.className = 'sidebar-name';
	        name.textContent = text;
	        btn.appendChild(name);
	        sidebarList.appendChild(btn);
	      }
	      updateOutlineActive();
	      return;
	    }
	    var items = sidebarData[sidebarSection] || [];
	    if (!items.length) {
	      sidebarList.appendChild(emptyNote(SIDEBAR_EMPTY));
	      return;
	    }
	    items.forEach(function(item) {
	      var btn = document.createElement('button');
	      btn.type = 'button';
	      btn.className = 'sidebar-item' + (item.active ? ' active' : '');
	      btn.setAttribute('data-sidebar-path', item.path);
	      var name = document.createElement('span');
	      name.className = 'sidebar-name';
	      name.textContent = item.name;
	      btn.appendChild(name);
	      // 最近打开会跨目录，补一行所在目录才分得清同名文件
	      if (sidebarSection === 'recent') {
	        var dir = document.createElement('span');
	        dir.className = 'sidebar-dir';
	        dir.textContent = item.dir;
	        btn.appendChild(dir);
	      }
	      sidebarList.appendChild(btn);
	    });
	  }
	  function updateOutlineActive() {
	    if (sidebarSection !== 'outline') return;
	    var preview = previewEl;
	    if (!preview) return;
	    var headings = preview.querySelectorAll('h1, h2, h3, h4, h5, h6');
	    if (!headings.length) return;
	    var activeId = null;
	    for (var i = 0; i < headings.length; i++) {
	      var rect = headings[i].getBoundingClientRect();
	      if (rect.top <= OUTLINE_ACTIVE_OFFSET_PX) {
	        activeId = headings[i].id;
	      } else {
	        break;
	      }
	    }
	    if (!activeId && headings.length > 0) activeId = headings[0].id;
	    var items = sidebarList.querySelectorAll('.outline-item');
	    for (var j = 0; j < items.length; j++) {
	      var match = items[j].getAttribute('data-outline-id') === activeId;
	      items[j].classList.toggle('active', match);
	    }
	  }
	  window.addEventListener('scroll', function() {
	    if (sidebarSection === 'outline') updateOutlineActive();
	  }, { passive: true });
	  window.__setSidebar = function(data) {
	    sidebarData = data || { folder: [], recent: [] };
	    renderSidebar();
	  };
	  btnSidebar.addEventListener('click', function() {
	    // 立刻切换视觉状态，落盘交给 Rust；回显时状态一致，不会来回跳
	    var open = !document.body.classList.contains('sidebar-open');
	    document.body.classList.toggle('sidebar-open', open);
	    window.ipc.postMessage('set-setting:sidebar=' + (open ? '1' : '0'));
	  });
	  sidebarEl.addEventListener('click', function(e) {
	    var section = hit(e, '[data-sidebar-section]');
	    if (section) {
	      sidebarSection = section.getAttribute('data-sidebar-section');
	      renderSidebar();
	      return;
	    }
	    var outlineItem = hit(e, '[data-outline-id]');
	    if (outlineItem) {
	      var targetHeading = document.getElementById(outlineItem.getAttribute('data-outline-id'));
	      if (targetHeading) {
	        targetHeading.scrollIntoView({ behavior: 'smooth', block: 'start' });
	      }
	      return;
	    }
	    var item = hit(e, '[data-sidebar-path]');
	    if (item) window.ipc.postMessage('open-doc:' + item.getAttribute('data-sidebar-path'));
	  });
	  renderSidebar();
	  // 完整路径用自定义悬浮框展示：侧栏条目只放得下文件名和目录名，原生 title 样式不可控且长路径会被截断
	  function showSidebarTooltip(item) {
	    if (!sidebarTooltip) return;
	    var rect = item.getBoundingClientRect();
	    sidebarTooltip.textContent = item.getAttribute('data-sidebar-path');
	    sidebarTooltip.style.display = 'block';
	    var width = sidebarTooltip.offsetWidth || 200;
	    var height = sidebarTooltip.offsetHeight || 40;
	    var left = Math.min(rect.right + OVERLAY_MARGIN_PX, window.innerWidth - width - OVERLAY_MARGIN_PX);
	    var top = Math.min(rect.top, window.innerHeight - height - OVERLAY_MARGIN_PX);
	    sidebarTooltip.style.left = Math.max(OVERLAY_MARGIN_PX, left) + 'px';
	    sidebarTooltip.style.top = Math.max(OVERLAY_MARGIN_PX, top) + 'px';
	  }
	  function hideSidebarTooltip() {
	    if (sidebarTooltipTimer) {
	      clearTimeout(sidebarTooltipTimer);
	      sidebarTooltipTimer = 0;
	    }
	    if (sidebarTooltip) sidebarTooltip.style.display = 'none';
	  }
	  sidebarList.addEventListener('mouseover', function(e) {
	    var item = hit(e, '[data-sidebar-path]');
	    if (!item) return;
	    // 在同一条目内部的子元素之间移动不算重新进入
	    if (e.relatedTarget && item.contains(e.relatedTarget)) return;
	    hideSidebarTooltip();
	    sidebarTooltipTimer = setTimeout(function() {
	      sidebarTooltipTimer = 0;
	      showSidebarTooltip(item);
	    }, TOOLTIP_DELAY_MS);
	  });
	  sidebarList.addEventListener('mouseout', function(e) {
	    var item = hit(e, '[data-sidebar-path]');
	    if (!item) return;
	    if (e.relatedTarget && item.contains(e.relatedTarget)) return;
	    hideSidebarTooltip();
	  });
	  sidebarList.addEventListener('scroll', hideSidebarTooltip, { passive: true });
	  function showRecentContextMenu(path, x, y) {
	    if (!recentContextMenu) return;
	    hideSidebarTooltip();
	    hideTabContextMenu();
	    recentContextPath = path;
	    recentContextMenu.style.display = 'block';
	    var menuWidth = recentContextMenu.offsetWidth || 160;
	    var menuHeight = recentContextMenu.offsetHeight || 110;
	    recentContextMenu.style.left = Math.max(OVERLAY_MARGIN_PX, Math.min(x, window.innerWidth - menuWidth - OVERLAY_MARGIN_PX)) + 'px';
	    recentContextMenu.style.top = Math.max(OVERLAY_MARGIN_PX, Math.min(y, window.innerHeight - menuHeight - OVERLAY_MARGIN_PX)) + 'px';
	  }
	  function hideRecentContextMenu() {
	    if (recentContextMenu) recentContextMenu.style.display = 'none';
	    recentContextPath = '';
	  }
	  sidebarList.addEventListener('contextmenu', function(e) {
	    // 右键菜单只针对最近打开的历史条目，当前文件夹列表没有移除语义
	    if (sidebarSection !== 'recent') return;
	    var item = hit(e, '[data-sidebar-path]');
	    if (!item) return;
	    e.preventDefault();
	    e.stopPropagation();
	    showRecentContextMenu(item.getAttribute('data-sidebar-path'), e.clientX, e.clientY);
	  });
	  if (recentContextMenu) {
	    recentContextMenu.addEventListener('click', function(e) {
	      var item = hit(e, '[data-recent-action]');
	      if (!item) return;
	      var action = item.getAttribute('data-recent-action');
	      var path = recentContextPath;
	      hideRecentContextMenu();
	      if (!path) return;
	      if (action === 'reveal') {
	        window.ipc.postMessage('reveal-path:' + path);
	      } else if (action === 'copy-path') {
	        copyText(path);
	      } else if (action === 'remove') {
	        window.ipc.postMessage('forget-recent:' + path);
	      }
	    });
	  }
	  if (sidebarClearRecent) {
	    sidebarClearRecent.addEventListener('click', function() {
	      window.ipc.postMessage('clear-recent');
	    });
	  }
	  var authorDoc = { titleLine: '', title: '' };
	  var authorMode = false;
	  var AUTHOR_LABELS = {
	    'title-line': CFG.copyTitleLineJs,
	    'title': CFG.copyTitleJs,
	    'body': CFG.copyBodyJs
	  };
	  var AUTHOR_COPIED = CFG.copiedJs;

	  function authorButton(kind) {
	    var btn = document.createElement('button');
	    btn.type = 'button';
	    btn.className = 'author-copy';
	    btn.setAttribute('data-author-copy', kind);
	    btn.textContent = AUTHOR_LABELS[kind];
	    return btn;
	  }
	  // 正文按屏幕上看到的取：逐个顶层块读 innerText，段落之间留空行。
	  // 用渲染结果而不是 Markdown 原文，复制出来才不会带 # * ` 这些语法
	  function authorBodyText() {
	    var preview = previewEl;
	    if (!preview) return '';
	    var heading = preview.querySelector('h1, h2, h3, h4, h5, h6');
	    var reached = !heading;
	    var blocks = [];
	    for (var i = 0; i < preview.children.length; i++) {
	      var node = preview.children[i];
	      if (!reached) {
	        if (node === heading) reached = true;
	        continue;
	      }
	      if (node.classList && node.classList.contains('author-body-actions')) continue;
	      var text = (node.innerText || node.textContent || '').replace(/\s+$/, '');
	      if (text) blocks.push(text);
	    }
	    return blocks.join('\n\n');
	  }
	  function applyAuthorMode() {
	    var preview = previewEl;
	    if (!preview) return;
	    var stale = preview.querySelectorAll('.author-actions, .author-body-actions');
	    for (var i = 0; i < stale.length; i++) {
	      stale[i].parentNode.removeChild(stale[i]);
	    }
	    if (!authorMode) return;
	    var heading = preview.querySelector('h1, h2, h3, h4, h5, h6');
	    if (heading && authorDoc.titleLine) {
	      var actions = document.createElement('span');
	      actions.className = 'author-actions';
	      actions.appendChild(authorButton('title-line'));
	      actions.appendChild(authorButton('title'));
	      heading.appendChild(actions);
	    }
	    if (!authorBodyText()) return;
	    var bodyActions = document.createElement('div');
	    bodyActions.className = 'author-body-actions';
	    bodyActions.appendChild(authorButton('body'));
	    if (heading) preview.insertBefore(bodyActions, heading.nextSibling);
	    else preview.insertBefore(bodyActions, preview.firstChild);
	  }
	  window.__applyAuthorMode = applyAuthorMode;
	  window.__setAuthorDoc = function(doc) {
	    authorDoc = doc || { titleLine: '', title: '' };
	    applyAuthorMode();
	  };
	  // 页面由 with_html 载入，不是安全上下文，navigator.clipboard 未必可用，所以留一条 execCommand 退路
	  function authorCopyFallback(text) {
	    var holder = document.createElement('textarea');
	    holder.value = text;
	    holder.setAttribute('readonly', '');
	    holder.style.position = 'fixed';
	    holder.style.top = '-1000px';
	    document.body.appendChild(holder);
	    holder.select();
	    var ok;
	    try {
	      ok = document.execCommand('copy');
	    } catch (err) {
	      ok = false;
	    }
	    document.body.removeChild(holder);
	    return ok;
	  }
	  function copyText(text, callback) {
	    if (!text) return;
	    if (navigator.clipboard && navigator.clipboard.writeText) {
	      navigator.clipboard.writeText(text).then(function() {
	        if (callback) callback();
	      }, function() {
	        if (authorCopyFallback(text) && callback) callback();
	      });
	    } else {
	      if (authorCopyFallback(text) && callback) callback();
	    }
	  }
	  function setupCodeBlockCopyButtons() {
	    var preview = previewEl;
	    if (!preview) return;
	    var pres = preview.querySelectorAll('pre');
	    for (var i = 0; i < pres.length; i++) {
	      var pre = pres[i];
	      if (pre.classList && (pre.classList.contains('front-matter') || pre.classList.contains('mdp-mermaid'))) continue;
	      if (pre.querySelector('.code-copy-btn')) continue;
	      var btn = document.createElement('button');
	      btn.type = 'button';
	      btn.className = 'code-copy-btn';
	      btn.textContent = L_COPY;
	      btn.setAttribute('aria-label', L_COPY);
	      pre.appendChild(btn);
	    }
	  }
	  function showTabContextMenu(id, path, x, y) {
	    if (!tabContextMenu) return;
	    contextMenuTabId = id;
	    contextMenuTabPath = path || '';
	    tabContextMenu.style.display = 'block';
	    var menuWidth = tabContextMenu.offsetWidth || 160;
	    var menuHeight = tabContextMenu.offsetHeight || 130;
	    var posX = Math.min(x, window.innerWidth - menuWidth - OVERLAY_MARGIN_PX);
	    var posY = Math.min(y, window.innerHeight - menuHeight - OVERLAY_MARGIN_PX);
	    tabContextMenu.style.left = Math.max(OVERLAY_MARGIN_PX, posX) + 'px';
	    tabContextMenu.style.top = Math.max(OVERLAY_MARGIN_PX, posY) + 'px';
	  }
	  function hideTabContextMenu() {
	    if (tabContextMenu) tabContextMenu.style.display = 'none';
	    contextMenuTabId = null;
	    contextMenuTabPath = '';
	  }
	  if (tabsEl) {
	    tabsEl.addEventListener('contextmenu', function(e) {
	      var tab = hit(e, '[data-tab-id]');
	      if (!tab) return;
	      e.preventDefault();
	      e.stopPropagation();
	      showTabContextMenu(tab.getAttribute('data-tab-id'), tab.title, e.clientX, e.clientY);
	    });
	    tabsEl.addEventListener('auxclick', function(e) {
	      if (e.button === 1) {
	        var tab = hit(e, '[data-tab-id]');
	        if (tab) {
	          e.preventDefault();
	          e.stopPropagation();
	          requestTabAction('close', tab.getAttribute('data-tab-id'));
	        }
	      }
	    });
	  }
	  if (tabContextMenu) {
	    tabContextMenu.addEventListener('click', function(e) {
	      var item = hit(e, '[data-tab-action]');
	      if (!item) return;
	      var action = item.getAttribute('data-tab-action');
	      var id = contextMenuTabId;
	      var path = contextMenuTabPath;
	      hideTabContextMenu();
	      if (!id) return;
	      if (action === 'close') {
	        requestTabAction('close', id);
	      } else if (action === 'close-others') {
	        requestTabAction('close-others', id);
	      } else if (action === 'copy-path') {
	        if (path) copyText(path);
	      } else if (action === 'reveal') {
	        window.ipc.postMessage('reveal-tab:' + id);
	      }
	    });
	  }
	  function updateLightboxTransform() {
	    if (!lbImg) return;
	    lbImg.style.transform = 'translate(' + lbTranslateX + 'px, ' + lbTranslateY + 'px) scale(' + lbScale + ')';
	    if (lbZoomReset) lbZoomReset.textContent = Math.round(lbScale * 100) + '%';
	  }
	  function openLightbox(src, caption) {
	    if (!lightbox || !lbImg) return;
	    lbImg.src = src;
	    if (lbCaption) lbCaption.textContent = caption || '';
	    lbScale = 1.0;
	    lbTranslateX = 0;
	    lbTranslateY = 0;
	    updateLightboxTransform();
	    lightbox.style.display = 'flex';
	    document.body.classList.add('lightbox-open');
	  }
	  function closeLightbox() {
	    if (!lightbox) return;
	    lightbox.style.display = 'none';
	    document.body.classList.remove('lightbox-open');
	    if (lbImg) lbImg.src = '';
	  }
	  if (lbClose) lbClose.addEventListener('click', function(e) { e.stopPropagation(); closeLightbox(); });
	  if (lbZoomIn) lbZoomIn.addEventListener('click', function(e) {
	    e.stopPropagation();
	    lbScale = Math.min(5.0, lbScale + 0.25);
	    updateLightboxTransform();
	  });
	  if (lbZoomOut) lbZoomOut.addEventListener('click', function(e) {
	    e.stopPropagation();
	    lbScale = Math.max(0.2, lbScale - 0.25);
	    updateLightboxTransform();
	  });
	  if (lbZoomReset) lbZoomReset.addEventListener('click', function(e) {
	    e.stopPropagation();
	    lbScale = 1.0;
	    lbTranslateX = 0;
	    lbTranslateY = 0;
	    updateLightboxTransform();
	  });
	  if (lightbox) {
	    lightbox.addEventListener('click', function(e) {
	      if (e.target === lightbox || (e.target && e.target.classList && (e.target.classList.contains('lightbox-backdrop') || e.target.classList.contains('lightbox-stage')))) {
	        closeLightbox();
	      }
	    });
	    lightbox.addEventListener('wheel', function(e) {
	      e.preventDefault();
	      var delta = e.deltaY < 0 ? 0.2 : -0.2;
	      lbScale = Math.min(5.0, Math.max(0.2, lbScale + delta));
	      updateLightboxTransform();
	    }, { passive: false });
	  }
	  if (lbImg) {
	    lbImg.addEventListener('mousedown', function(e) {
	      if (e.button !== 0) return;
	      e.preventDefault();
	      lbIsDragging = true;
	      lbStartX = e.clientX - lbTranslateX;
	      lbStartY = e.clientY - lbTranslateY;
	      var stage = lbImg.closest('.lightbox-stage');
	      if (stage) stage.classList.add('dragging');
	    });
	  }
	  window.addEventListener('mousemove', function(e) {
	    if (!lbIsDragging) return;
	    lbTranslateX = e.clientX - lbStartX;
	    lbTranslateY = e.clientY - lbStartY;
	    updateLightboxTransform();
	  });
	  window.addEventListener('mouseup', function() {
	    if (!lbIsDragging) return;
	    lbIsDragging = false;
	    if (lbImg) {
	      var stage = lbImg.closest('.lightbox-stage');
	      if (stage) stage.classList.remove('dragging');
	    }
	  });
	  function flashCopied(btn) {
	    var original = btn.textContent;
	    btn.textContent = AUTHOR_COPIED;
	    btn.classList.add('done');
	    setTimeout(function() {
	      btn.textContent = original;
	      btn.classList.remove('done');
	    }, AUTHOR_FLASH_MS);
	  }
	  document.addEventListener('click', function(e) {
	    var btn = hit(e, '.author-actions [data-author-copy], .author-body-actions [data-author-copy]');
	    if (!btn) return;
	    e.preventDefault();
	    var kind = btn.getAttribute('data-author-copy');
	    var text = kind === 'title-line' ? authorDoc.titleLine : (kind === 'title' ? authorDoc.title : authorBodyText());
	    if (!text) return;
	    copyText(text, function() { flashCopied(btn); });
	  });
	  window.__setSettings = function(settings) {
	    settings = settings || {};
	    authorMode = !!settings.authorMode;
	    applyAuthorMode();
	    document.body.classList.toggle('sidebar-open', !!settings.sidebarOpen);
	    document.body.classList.toggle('no-wrap', settings.wordWrap === false);
	    var current = {
	      'open-mode': settings.openMode,
	      'tab-mode': settings.tabMode,
	      'word-wrap': settings.wordWrap === false ? 'off' : 'on',
	      'author-mode': settings.authorMode ? 'on' : 'off',
	      'theme': settings.theme
	    };
	    var buttons = settingsControl.querySelectorAll('[data-setting]');
	    for (var i = 0; i < buttons.length; i++) {
	      var btn = buttons[i];
	      var selected = current[btn.getAttribute('data-setting')] === btn.getAttribute('data-value');
	      btn.setAttribute('aria-pressed', selected ? 'true' : 'false');
	    }
	  };
	  btnZoomOut.addEventListener('click', function() { changeZoom(-ZOOM_STEP); });
	  btnZoomReset.addEventListener('click', function() { applyZoom(100, true); });
	  btnZoomIn.addEventListener('click', function() { changeZoom(ZOOM_STEP); });
  btnPrint.addEventListener('click', function() {
    if (inEdit()) leaveEdit();
    // Route through Rust: WKWebView ignores window.print(); wry's
    // WebView::print() calls the right native API on each platform.
    setTimeout(function(){ window.ipc.postMessage('print'); }, 0);
  });
	  ta.addEventListener('input', function() {
	    setDirty(true);
	    scheduleAutosave();
	    if (document.body.classList.contains('split-view')) scheduleLiveRender();
	    autoResize();
	    updateDocumentStats(ta.value);
	  });
  window.addEventListener('resize', function() { if (inEdit()) autoResize(); });
  // 所有浮层的统一关闭入口：Escape 和点击空白处都走这里，新增浮层只需登记一处
  function closeAllOverlays() {
    var closed = false;
    if (encodingPopover && encodingPopover.style.display !== 'none') { encodingPopover.style.display = 'none'; closed = true; }
    if (zoomControl.classList.contains('open')) { zoomControl.classList.remove('open'); closed = true; }
    if (settingsControl.classList.contains('open')) { settingsControl.classList.remove('open'); closed = true; }
    if (tabContextMenu && tabContextMenu.style.display !== 'none') { hideTabContextMenu(); closed = true; }
    if (recentContextMenu && recentContextMenu.style.display !== 'none') { hideRecentContextMenu(); closed = true; }
    if (lightbox && lightbox.style.display !== 'none') { closeLightbox(); closed = true; }
    if (updateModal && updateModal.style.display !== 'none') { hideUpdateModal(); closed = true; }
    return closed;
  }
  // 点击浮层自身以外的地方就关掉它；各按钮不再 stopPropagation，右键菜单等才会在点击工具栏时一并收起
  function closeOverlaysOutside(target) {
    if (!zoomControl.contains(target)) zoomControl.classList.remove('open');
    if (!settingsControl.contains(target)) settingsControl.classList.remove('open');
    if (encodingPopover && !encodingPopover.contains(target) && (!btnEncoding || !btnEncoding.contains(target))) {
      encodingPopover.style.display = 'none';
    }
    if (tabContextMenu && !tabContextMenu.contains(target)) hideTabContextMenu();
    if (recentContextMenu && !recentContextMenu.contains(target)) hideRecentContextMenu();
  }
  // 文档切换时所有与旧文档绑定的临时状态都要归零：搜索、浮层、菜单、悬浮框和待触发的实时渲染
  function resetTransientUi() {
    hideFind();
    closeAllOverlays();
    hideSidebarTooltip();
    cancelLiveRender();
  }

  document.addEventListener('keydown', function(e) {
	if ((e.metaKey || e.ctrlKey) && (e.key === 'w' || e.key === 'W')) {
	  if (activeTabId) {
	    e.preventDefault();
	    requestTabAction('close', activeTabId);
	  }
	  return;
	}
	if ((e.metaKey || e.ctrlKey) && (e.key === 'n' || e.key === 'N')) {
	  e.preventDefault();
	  newFile();
	  return;
	}
    if ((e.metaKey || e.ctrlKey) && (e.key === '+' || e.key === '=' || e.code === 'NumpadAdd')) {
      e.preventDefault();
      changeZoom(ZOOM_STEP);
      return;
    }
    if ((e.metaKey || e.ctrlKey) && (e.key === '-' || e.code === 'NumpadSubtract')) {
      e.preventDefault();
      changeZoom(-ZOOM_STEP);
      return;
    }
    if ((e.metaKey || e.ctrlKey) && (e.key === '0' || e.code === 'Numpad0')) {
      e.preventDefault();
      applyZoom(100, true);
      return;
    }
    if ((e.metaKey || e.ctrlKey) && (e.key === 'r' || e.key === 'R')) {
      e.preventDefault();
      if (!inEdit()) window.ipc.postMessage('refresh');
      return;
    }
	    if ((e.metaKey || e.ctrlKey) && (e.key === 'o' || e.key === 'O')) {
	      e.preventDefault();
	      openFile();
	      return;
	    }
	    if ((e.metaKey || e.ctrlKey) && (e.key === 'f' || e.key === 'F')) {
	      if (inEdit()) return;
	      e.preventDefault();
	      showFind();
	      return;
	    }
    if ((e.metaKey || e.ctrlKey) && (e.key === 'e' || e.key === 'E')) {
      e.preventDefault();
      if (inEdit()) leaveEdit(); else enterEdit();
      return;
    }
    if ((e.metaKey || e.ctrlKey) && e.shiftKey && (e.key === 's' || e.key === 'S')) {
      e.preventDefault();
      saveAs();
      return;
    }
    if ((e.metaKey || e.ctrlKey) && (e.key === 's' || e.key === 'S')) {
      if (inEdit()) { e.preventDefault(); save(); }
      return;
    }
    if ((e.metaKey || e.ctrlKey) && (e.key === 'p' || e.key === 'P')) {
      e.preventDefault();
      if (inEdit()) leaveEdit();
      setTimeout(function(){ window.ipc.postMessage('print'); }, 0);
      return;
    }
	    if ((e.metaKey || e.ctrlKey) && (e.key === '\\' || e.code === 'Backslash')) {
	      e.preventDefault();
	      if (!inEdit()) enterEdit();
	      toggleSplitView();
	      return;
	    }
	    if (e.key === 'Escape') {
	      if (closeAllOverlays()) return;
	      if (document.body.classList.contains('finding')) { hideFind(); return; }
	      if (inEdit()) leaveEdit();
	    }
  });

  // Called by Rust after a save (only preview is refreshed) or after an
  // external file change (both preview + textarea are refreshed).
  window.__setPreview = function(previewHtml, needsMath, needsMermaid) {
    if (arguments.length > 1 && window.__setFeatureFlags) {
      window.__setFeatureFlags(needsMath, needsMermaid);
    }
    // 旧的搜索高亮节点随 innerHTML 一起没了，命中列表必须重建，否则 n/m 和跳转都指向已脱离的节点
    findHits = [];
    currentFindHit = -1;
    lastFindQuery = '';
    previewEl.innerHTML = previewHtml;
    if (document.body.classList.contains('finding') && findInput.value.trim()) runFindQuery(findInput.value);
    else updateFindState();
    if (window.__applyAuthorMode) window.__applyAuthorMode();
    setupCodeBlockCopyButtons();
    if (sidebarSection === 'outline') renderSidebar();
    (window.requestIdleCallback || function(fn){ return setTimeout(fn, 0); })(function() {
      if (typeof hljs !== 'undefined') hljs.highlightAll();
      if (window.__enhancePreview) window.__enhancePreview();
    });
  };
  window.__setBaseHref = function(baseHref) {
    var base = document.getElementById('base-href');
    if (!base) {
      base = document.createElement('base');
      base.id = 'base-href';
      document.head.insertBefore(base, document.head.firstChild);
    }
    if (baseHref) base.setAttribute('href', baseHref);
    else base.removeAttribute('href');
  };
	window.__markSaved = function(savedRaw) {
	  if (typeof savedRaw === 'string' && ta.value !== savedRaw) {
	    window.ipc.postMessage('dirty:1');
	    return;
	  }
	  autosavePaused = false;
	  setDirty(false);
	};
	window.__mdPreviewerPauseAutosave = function() {
	  cancelPendingAutosave();
	  autosavePaused = true;
	};
	window.__mdPreviewerResolveExternalChange = function() {
	  cancelPendingAutosave();
	  window.ipc.postMessage('external-change:' + (dirty ? 'dirty' : 'clean'));
	};
	window.__setTabs = function(tabs) {
	  tabs = Array.isArray(tabs) ? tabs : [];
	  tabsEl.textContent = '';
	  activeTabId = 0;
	  document.body.classList.toggle('has-tabs', tabs.length > 0);
	  tabs.forEach(function(tab) {
	    var item = document.createElement('div');
	    item.className = 'tab' + (tab.active ? ' active' : '') + (tab.missing ? ' missing' : '') + (tab.dirty ? ' dirty' : '');
	    item.setAttribute('data-tab-id', tab.id);
	    item.setAttribute('role', 'button');
	    item.setAttribute('tabindex', '0');
	    item.title = tab.path;
	    if (tab.active) activeTabId = tab.id;
	    var status = document.createElement('span');
	    status.className = 'tab-status';
	    status.textContent = tab.missing ? '!' : '';
	    var name = document.createElement('span');
	    name.className = 'tab-name';
	    name.textContent = tab.name;
	    var close = document.createElement('button');
	    close.className = 'tab-close';
	    close.type = 'button';
	    close.setAttribute('data-close-tab', tab.id);
	    close.setAttribute('aria-label', 'Close ' + tab.name);
	    close.textContent = '×';
	    item.appendChild(status);
	    item.appendChild(name);
	    item.appendChild(close);
	    tabsEl.appendChild(item);
	    if (tab.active) requestAnimationFrame(function() { item.scrollIntoView({ block: 'nearest', inline: 'nearest' }); });
	  });
	};
	tabsEl.addEventListener('keydown', function(e) {
	  if (e.key !== 'Enter' && e.key !== ' ') return;
	  var tab = hit(e, '[data-tab-id]');
	  if (!tab) return;
	  e.preventDefault();
	  requestTabAction('activate', tab.getAttribute('data-tab-id'));
	});
	  window.__setContent = function(previewHtml, rawMd, baseHref, needsMath, needsMermaid) {
	    document.body.classList.remove('empty');
	    document.body.classList.remove('missing');
	    resetTransientUi();
	    window.__setBaseHref(baseHref);
	    window.__setPreview(previewHtml, needsMath, needsMermaid);
	    // 编辑中且有未保存内容时保留编辑框，只刷新预览；待触发的自动保存也必须留着
	    if (!inEdit() || !dirty) {
	      cancelPendingAutosave();
	      autosavePaused = false;
	      ta.value = rawMd;
	      updateDocumentStats(rawMd);
      setDirty(false);
      if (inEdit()) autoResize();
    }
  };
	  // 空白页与缺失页共用一套复位：退出编辑、收起所有浮层和定时器、清空编辑框
	  function showPlaceholder(state, previewHtml) {
	    document.body.classList.toggle('empty', state === 'empty');
	    document.body.classList.toggle('missing', state === 'missing');
	    document.body.classList.remove('editing');
	    btnToggle.innerHTML = ICON_EDIT;
	    btnToggle.title = L_EDIT;
	    btnToggle.setAttribute('aria-label', L_EDIT);
	    resetTransientUi();
	    cancelPendingAutosave();
	    autosavePaused = false;
	    window.__setBaseHref('');
	    previewEl.innerHTML = previewHtml;
	    ta.value = '';
	    updateDocumentStats('');
	    setDirty(false);
	    window.scrollTo(0, 0);
	  }
	  window.__setEmptyPreview = function(previewHtml) { showPlaceholder('empty', previewHtml); };
	  window.__setMissing = function(previewHtml) { showPlaceholder('missing', previewHtml); };

  // Defer hljs parse + initial highlight to idle time.
  // hljs itself is NOT inlined in this page — Rust injects it via
  // evaluate_script once we tell it we're painted. Until that injection
  // runs, typeof hljs === 'undefined' and highlightAll is skipped; once
  // it lands, hljs.highlightAll() gets called by the injected bootstrap
  // and __setPreview.

  // -------------------------------------------------------------
  // 自动更新检测与弹窗交互逻辑 (Auto-Update Check & Modal)
  // -------------------------------------------------------------
  var UPDATE_API_URL = 'https://api.github.com/repos/ArnoldRedman/MD-Previewer/releases/latest';
  var UPDATE_STORAGE_KEY = 'mdp:update-status';
  var UPDATE_CHECK_INTERVAL_MS = 4 * 60 * 60 * 1000;
  var CURRENT_VERSION = CFG.cargoVersion;
  var L_CHECKING_UPDATE = CFG.updateStatusCheckingJs;
  var L_LATEST_UPDATE = CFG.updateStatusLatestJs;
  var L_FAILED_UPDATE = CFG.updateStatusFailedJs;
  var L_DOWNLOADING_UPDATE = CFG.updateDownloadingJs;
  var L_INSTALLING_UPDATE = CFG.updateInstallingJs;
  var L_UPDATE_FAILED = CFG.updateFailedJs;
  var L_UPDATE_TEXT = CFG.btnUpdateTextJs;
  var L_CHECK_UPDATE = CFG.btnCheckUpdateJs;

  var btnUpdateAvailable = document.getElementById('btn-update-available');
  var btnCheckUpdate = document.getElementById('btn-check-update');
  var updateStatusMsg = document.getElementById('update-status-msg');
  var updateModal = document.getElementById('update-modal');
  var updateBackdrop = document.getElementById('update-backdrop');
  var updateClose = document.getElementById('update-close');
  var updateBadge = document.getElementById('update-badge');
  var updateReleaseName = document.getElementById('update-release-name');
  var updateNotesBox = document.getElementById('update-notes-box');
  var updateProgressTip = document.getElementById('update-progress-tip');
  var updateProgress = document.getElementById('update-progress');
  var updateProgressFill = document.getElementById('update-progress-fill');
  var updateProgressMeta = document.getElementById('update-progress-meta');
  // 下载由 Rust 在后台线程进行；弹窗关掉再打开也要继续显示进度，所以状态放在这里
  var updateDownloading = false;
  var btnDoUpdate = document.getElementById('btn-do-update');
  var btnViewRelease = document.getElementById('btn-view-release');
  var btnDismissUpdate = document.getElementById('btn-dismiss-update');
  var activeReleaseData = null;

  function parseVersion(v) {
    if (!v || typeof v !== 'string') return null;
    var s = v.trim().replace(/^v/i, '');
    var parts = s.split('.');
    var nums = [];
    for (var i = 0; i < parts.length; i++) {
      var n = parseInt(parts[i], 10);
      if (isNaN(n)) return null;
      nums.push(n);
    }
    return nums;
  }

  function isNewerVersion(candidate, current) {
    var next = parseVersion(candidate);
    var now = parseVersion(current);
    if (!next || !now) return false;
    var len = Math.max(next.length, now.length);
    for (var i = 0; i < len; i++) {
      var a = next[i] || 0;
      var b = now[i] || 0;
      if (a > b) return true;
      if (a < b) return false;
    }
    return false;
  }

  var requestedNotesTag = null;

  // 发布说明是 GitHub Markdown，交给 Rust 侧与正文相同的渲染器转成 HTML 后回填；
  // 换了版本才清空旧内容，同一版本重新打开弹窗时不闪烁
  function renderUpdateNotes(release) {
    var tag = release.tag_name || '';
    if (requestedNotesTag !== tag) updateNotesBox.textContent = '';
    requestedNotesTag = tag;
    if (window.ipc) window.ipc.postMessage('render-release-notes:' + (release.body || ''));
  }

  // 去掉渲染结果里的 id：弹窗位于 #preview 之前，重名 id 会让大纲跳转落到弹窗里
  function setUpdateNotes(html) {
    if (!updateNotesBox) return;
    updateNotesBox.innerHTML = html;
    var withIds = updateNotesBox.querySelectorAll('[id]');
    for (var i = 0; i < withIds.length; i++) withIds[i].removeAttribute('id');
  }

  function formatBytes(n) {
    if (n >= 1048576) return (n / 1048576).toFixed(1) + ' MB';
    if (n >= 1024) return Math.round(n / 1024) + ' KB';
    return n + ' B';
  }

  function setUpdateTip(text, failed) {
    if (!updateProgressTip) return;
    updateProgressTip.style.display = 'flex';
    updateProgressTip.textContent = text;
    updateProgressTip.classList.toggle('failed', !!failed);
  }

  function resetUpdateProgress() {
    updateDownloading = false;
    if (updateProgress) updateProgress.style.display = 'none';
    if (updateProgressTip) { updateProgressTip.style.display = 'none'; updateProgressTip.classList.remove('failed'); }
    if (btnDoUpdate) btnDoUpdate.disabled = false;
  }

  function setUpdateProgress(downloaded, total) {
    updateDownloading = true;
    if (btnDoUpdate) btnDoUpdate.disabled = true;
    if (updateProgress) updateProgress.style.display = 'flex';
    var known = typeof total === 'number' && total > 0;
    var pct = known ? Math.min(100, Math.floor(downloaded * 100 / total)) : 0;
    if (updateProgressFill) {
      updateProgressFill.classList.toggle('indeterminate', !known);
      updateProgressFill.style.width = known ? pct + '%' : '';
    }
    if (updateProgressMeta) {
      updateProgressMeta.textContent = known
        ? formatBytes(downloaded) + ' / ' + formatBytes(total) + ' (' + pct + '%)'
        : formatBytes(downloaded);
    }
    setUpdateTip(L_DOWNLOADING_UPDATE, false);
  }

  function setUpdateInstalling() {
    updateDownloading = true;
    if (updateProgressFill) { updateProgressFill.classList.remove('indeterminate'); updateProgressFill.style.width = '100%'; }
    setUpdateTip(L_INSTALLING_UPDATE, false);
  }

  function setUpdateFailed(message) {
    resetUpdateProgress();
    setUpdateTip(L_UPDATE_FAILED + (message || ''), true);
  }

  window.__setUpdateProgress = setUpdateProgress;
  window.__setUpdateInstalling = setUpdateInstalling;
  window.__setUpdateFailed = setUpdateFailed;

  function showUpdateModal(release) {
    if (!release) return;
    activeReleaseData = release;
    if (updateBadge) updateBadge.textContent = release.tag_name || ('v' + CURRENT_VERSION);
    if (updateReleaseName) updateReleaseName.textContent = release.name || '';
    if (updateNotesBox) renderUpdateNotes(release);
    // 下载进行中重新打开弹窗时保留进度条，只有空闲状态才清掉上次的提示
    if (!updateDownloading) resetUpdateProgress();
    if (updateModal) updateModal.style.display = 'flex';
  }

  function hideUpdateModal() {
    if (updateModal) updateModal.style.display = 'none';
  }

  function applyDetectedRelease(release) {
    if (!release || !isNewerVersion(release.tag_name, CURRENT_VERSION)) {
      if (btnUpdateAvailable) btnUpdateAvailable.style.display = 'none';
      if (btnCheckUpdate) {
        btnCheckUpdate.textContent = L_CHECK_UPDATE;
        btnCheckUpdate.classList.remove('has-update');
      }
      return false;
    }
    activeReleaseData = release;
    if (btnUpdateAvailable) {
      btnUpdateAvailable.style.display = 'inline-flex';
      var textEl = btnUpdateAvailable.querySelector('.update-text');
      if (textEl) textEl.textContent = release.tag_name;
    }
    if (btnCheckUpdate) {
      btnCheckUpdate.textContent = '🚀 ' + (release.tag_name || '') + ' ' + L_UPDATE_TEXT;
      btnCheckUpdate.classList.add('has-update');
    }
    return true;
  }

  window.__showUpdateModal = showUpdateModal;
  window.__setUpdateNotes = setUpdateNotes;
  window.__applyDetectedRelease = applyDetectedRelease;
  window.__isNewerVersion = isNewerVersion;
  window.__queryLatestRelease = queryLatestRelease;

  function queryLatestRelease(manual) {
    if (manual && updateStatusMsg) {
      updateStatusMsg.textContent = L_CHECKING_UPDATE;
      updateStatusMsg.style.color = '#1a73e8';
    }

    var controller = typeof AbortController !== 'undefined' ? new AbortController() : null;
    var timer = controller ? setTimeout(function() { controller.abort(); }, 8000) : null;
    var opts = {
      cache: 'no-store',
      headers: { 'Accept': 'application/vnd.github+json' }
    };
    if (controller) opts.signal = controller.signal;

    fetch(UPDATE_API_URL, opts)
      .then(function(res) {
        if (!res.ok) throw new Error('status ' + res.status);
        return res.json();
      })
      .then(function(data) {
        if (timer) clearTimeout(timer);
        var isNewer = isNewerVersion(data.tag_name, CURRENT_VERSION);
        try {
          localStorage.setItem(UPDATE_STORAGE_KEY, JSON.stringify({
            checkedAt: Date.now(),
            tag_name: data.tag_name,
            name: data.name,
            body: data.body,
            html_url: data.html_url,
            assets: data.assets
          }));
        } catch (e) {}

        if (isNewer) {
          applyDetectedRelease(data);
          if (manual) {
            if (updateStatusMsg) {
              updateStatusMsg.textContent = data.tag_name;
              updateStatusMsg.style.color = '#1a73e8';
            }
            showUpdateModal(data);
          }
        } else {
          if (btnUpdateAvailable) btnUpdateAvailable.style.display = 'none';
          if (btnCheckUpdate) {
            btnCheckUpdate.textContent = L_CHECK_UPDATE;
            btnCheckUpdate.classList.remove('has-update');
          }
          if (manual && updateStatusMsg) {
            updateStatusMsg.textContent = L_LATEST_UPDATE;
            updateStatusMsg.style.color = '#2e7d32';
          }
        }
      })
      .catch(function() {
        if (timer) clearTimeout(timer);
        if (manual && updateStatusMsg) {
          updateStatusMsg.textContent = L_FAILED_UPDATE;
          updateStatusMsg.style.color = '#c62828';
        }
      });
  }

  if (btnUpdateAvailable) {
    btnUpdateAvailable.addEventListener('click', function(e) {
      e.stopPropagation();
      showUpdateModal(activeReleaseData);
    });
  }
  if (btnCheckUpdate) {
    btnCheckUpdate.addEventListener('click', function(e) {
      e.stopPropagation();
      if (activeReleaseData && isNewerVersion(activeReleaseData.tag_name, CURRENT_VERSION)) {
        showUpdateModal(activeReleaseData);
      } else {
        queryLatestRelease(true);
      }
    });
  }
  if (updateClose) updateClose.addEventListener('click', hideUpdateModal);
  if (updateBackdrop) updateBackdrop.addEventListener('click', hideUpdateModal);
  if (btnDismissUpdate) btnDismissUpdate.addEventListener('click', hideUpdateModal);

  if (btnViewRelease) {
    btnViewRelease.addEventListener('click', function() {
      var url = (activeReleaseData && activeReleaseData.html_url) || 'https://github.com/ArnoldRedman/MD-Previewer/releases';
      window.location.href = url;
    });
  }

  if (btnDoUpdate) {
    btnDoUpdate.addEventListener('click', function() {
      if (!activeReleaseData) return;
      var assets = activeReleaseData.assets || [];
      var chosenAsset = null;
      // 优先下载免安装单文件 EXE，实现原地静默替换；若无则降级匹配安装包
      for (var i = 0; i < assets.length; i++) {
        if (/MD-Previewer-windows-x64\.exe$/i.test(assets[i].name || '')) {
          chosenAsset = assets[i];
          break;
        }
      }
      if (!chosenAsset) {
        for (var j = 0; j < assets.length; j++) {
          if (/MD-Previewer-Setup\.exe$/i.test(assets[j].name || '')) {
            chosenAsset = assets[j];
            break;
          }
        }
      }
      var downloadUrl = chosenAsset ? chosenAsset.browser_download_url : activeReleaseData.html_url;
      if (downloadUrl && downloadUrl.indexOf('.exe') !== -1 && window.ipc) {
        if (updateDownloading) return;
        // 先按未知总量显示动画，Rust 拿到 Content-Length 后会推真实进度
        setUpdateProgress(0, null);
        window.ipc.postMessage('self-update:' + downloadUrl);
      } else {
        var fallbackUrl = (activeReleaseData && activeReleaseData.html_url) || 'https://github.com/ArnoldRedman/MD-Previewer/releases';
        window.location.href = fallbackUrl;
      }
    });
  }

  function scheduleUpdateCheck() {
    try {
      var cached = JSON.parse(localStorage.getItem(UPDATE_STORAGE_KEY) || 'null');
      if (cached && cached.tag_name && isNewerVersion(cached.tag_name, CURRENT_VERSION)) {
        applyDetectedRelease(cached);
      }
      var lastChecked = cached && cached.checkedAt ? cached.checkedAt : 0;
      var now = Date.now();
      if (now - lastChecked >= UPDATE_CHECK_INTERVAL_MS) {
        setTimeout(function() { queryLatestRelease(false); }, 2500);
      }
    } catch(e) {
      setTimeout(function() { queryLatestRelease(false); }, 2500);
    }

    setInterval(function() {
      try {
        var cached = JSON.parse(localStorage.getItem(UPDATE_STORAGE_KEY) || 'null');
        var lastChecked = cached && cached.checkedAt ? cached.checkedAt : 0;
        if (Date.now() - lastChecked >= UPDATE_CHECK_INTERVAL_MS) {
          queryLatestRelease(false);
        }
      } catch(e) {}
    }, 60 * 60 * 1000);
  }

  scheduleUpdateCheck();

  setupCodeBlockCopyButtons();

  // Signal Rust after first paint (triggers hljs inject; bench mode exits).
  requestAnimationFrame(function() {
    requestAnimationFrame(function() {
      if (window.ipc) window.ipc.postMessage('ready');
    });
  });
})();
