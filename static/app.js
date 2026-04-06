// ── i18n helper ─────────────────────────────────────────────

function _t(key, fallback, vars) {
  var text = (window.i18n && window.i18n[key]) || fallback;
  if (vars) {
    for (var k in vars) {
      text = text.replace('{' + k + '}', vars[k]);
    }
  }
  return text;
}

// ── Language Switcher ──────────────────────────────────────

function setLang(lang) {
  var form = new FormData();
  form.append('lang', lang);
  form.append('redirect', window.location.pathname);
  fetch('/lang/set', { method: 'POST', body: new URLSearchParams(form) })
    .then(function() { location.reload(); });
}

// ── Token Management (localStorage) ─────────────────────────

const STORAGE_KEY = 'telegraph_hub_tokens';

function getTokens() {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY)) || {};
  } catch {
    return {};
  }
}

function saveToken(name, token) {
  const tokens = getTokens();
  tokens[name] = token;
  localStorage.setItem(STORAGE_KEY, JSON.stringify(tokens));
  refreshTokenSelect();
}

function deleteToken(name) {
  const tokens = getTokens();
  delete tokens[name];
  localStorage.setItem(STORAGE_KEY, JSON.stringify(tokens));
  refreshTokenSelect();
  renderSavedTokens();
}

function getActiveToken() {
  const select = document.getElementById('token-select');
  return select ? select.value : '';
}

// ── Token Select Dropdown ───────────────────────────────────

function refreshTokenSelect() {
  const select = document.getElementById('token-select');
  if (!select) return;

  const currentValue = select.value;
  const tokens = getTokens();

  // Clear all options
  while (select.firstChild) select.removeChild(select.firstChild);

  // Add placeholder
  const placeholder = document.createElement('option');
  placeholder.value = '';
  placeholder.textContent = _t('token_select_placeholder', '-- Select a token --');
  select.appendChild(placeholder);

  for (const [name, token] of Object.entries(tokens)) {
    const opt = document.createElement('option');
    opt.value = token;
    opt.textContent = name;
    if (token === currentValue) opt.selected = true;
    select.appendChild(opt);
  }
}

function onTokenChange() {
  const token = getActiveToken();
  const infoSection = document.getElementById('account-info-section');
  if (infoSection) {
    infoSection.style.display = token ? 'block' : 'none';
  }

  // Automatically load pages when a token is selected
  if (token) {
    loadPages();
  }
}

// ── Import Token ────────────────────────────────────────────

function importToken(e) {
  e.preventDefault();
  const name = document.getElementById('import_name').value.trim();
  const token = document.getElementById('import_token').value.trim();
  if (!name || !token) return;

  saveToken(name, token);
  document.getElementById('import_name').value = '';
  document.getElementById('import_token').value = '';
  renderSavedTokens();
  showToast(_t('token_imported', 'Token imported successfully!'), 'success');
}

// Save token from the account creation result
function saveTokenFromResult(name, token) {
  saveToken(name || 'Unnamed Account', token);
  renderSavedTokens();
  showToast(_t('token_saved', 'Token saved to browser!'), 'success');
}

// ── Saved Tokens List ───────────────────────────────────────

function renderSavedTokens() {
  const container = document.getElementById('saved-tokens-list');
  if (!container) return;

  const tokens = getTokens();
  const entries = Object.entries(tokens);

  // Clear existing content safely
  while (container.firstChild) container.removeChild(container.firstChild);

  if (entries.length === 0) {
    const p = document.createElement('p');
    p.className = 'text-muted';
    p.textContent = _t('no_saved_tokens', 'No saved tokens yet.');
    container.appendChild(p);
    return;
  }

  entries.forEach(([name, token]) => {
    const item = document.createElement('div');
    item.className = 'token-item';

    const info = document.createElement('div');
    const nameSpan = document.createElement('span');
    nameSpan.className = 'token-item-name';
    nameSpan.textContent = name;
    const valueSpan = document.createElement('span');
    valueSpan.className = 'token-item-value';
    valueSpan.textContent = token.length > 6
      ? token.slice(0, 3) + '***' + token.slice(-3)
      : '***';
    info.appendChild(nameSpan);
    info.appendChild(document.createTextNode(' '));
    info.appendChild(valueSpan);

    const actions = document.createElement('div');
    actions.className = 'btn-group';

    const copyBtn = document.createElement('button');
    copyBtn.className = 'btn btn-xs btn-outline';
    copyBtn.textContent = _t('copy', 'Copy');
    copyBtn.addEventListener('click', () => copyToClipboard(token));

    const removeBtn = document.createElement('button');
    removeBtn.className = 'btn btn-xs btn-danger';
    removeBtn.textContent = _t('remove', 'Remove');
    removeBtn.addEventListener('click', () => deleteToken(name));

    actions.appendChild(copyBtn);
    actions.appendChild(removeBtn);

    item.appendChild(info);
    item.appendChild(actions);
    container.appendChild(item);
  });
}

// ── Token Export / Import (JSON file) ───────────────────────

function exportTokens() {
  const tokens = getTokens();
  if (Object.keys(tokens).length === 0) {
    showToast(_t('no_tokens_export', 'No tokens to export.'), 'error');
    return;
  }
  const blob = new Blob([JSON.stringify(tokens, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = 'telegraph-hub-tokens.json';
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
  showToast(_t('tokens_exported', 'Tokens exported!'), 'success');
}

function importTokensFromFile() {
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = '.json';
  input.addEventListener('change', function() {
    const file = input.files[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = function(e) {
      try {
        const imported = JSON.parse(e.target.result);
        if (typeof imported !== 'object' || Array.isArray(imported)) {
          showToast(_t('invalid_file_format', 'Invalid token file format.'), 'error');
          return;
        }
        const existing = getTokens();
        let count = 0;
        for (const [name, token] of Object.entries(imported)) {
          if (typeof token === 'string' && token.length > 0) {
            existing[name] = token;
            count++;
          }
        }
        localStorage.setItem(STORAGE_KEY, JSON.stringify(existing));
        refreshTokenSelect();
        renderSavedTokens();
        showToast(_t('imported_count', 'Imported {count} token(s)!', {count: count}), 'success');
      } catch {
        showToast(_t('parse_failed', 'Failed to parse token file.'), 'error');
      }
    };
    reader.readAsText(file);
  });
  input.click();
}

// ── Batch Selection ─────────────────────────────────────────

const selectedPaths = new Set();
let isAllPagesSelected = false;

function togglePageSelection(path, checkbox) {
  if (checkbox.checked) {
    selectedPaths.add(path);
  } else {
    selectedPaths.delete(path);
  }
  updateSelectAllCheckbox();
  updateBatchBar();
}

function toggleSelectAll() {
  const selectAll = document.getElementById('select-all-checkbox');
  const checkboxes = document.querySelectorAll('.page-checkbox');

  if (!selectAll.checked) {
    // Unchecking: clear all cross-page selections too
    clearSelection();
    return;
  }

  // Checking: select all on current page
  checkboxes.forEach(cb => {
    cb.checked = true;
    selectedPaths.add(cb.dataset.path);
  });
  updateBatchBar();

  // If more pages exist beyond current page, show banner
  const banner = document.getElementById('select-all-banner');
  if (banner && checkboxes.length > 0) {
    const totalCount = parseInt(banner.dataset.totalCount, 10) || 0;
    if (totalCount > checkboxes.length) {
      banner.style.display = '';
      const prompt = document.getElementById('banner-select-prompt');
      const allSel = document.getElementById('banner-all-selected');
      if (prompt) prompt.style.display = '';
      if (allSel) allSel.style.display = 'none';
    }
  }
}

function updateSelectAllCheckbox() {
  const selectAll = document.getElementById('select-all-checkbox');
  if (!selectAll) return;
  const checkboxes = document.querySelectorAll('.page-checkbox');
  if (checkboxes.length === 0) {
    selectAll.checked = false;
    return;
  }
  const allChecked = Array.from(checkboxes).every(cb => cb.checked);
  const someChecked = Array.from(checkboxes).some(cb => cb.checked);
  selectAll.checked = allChecked;
  selectAll.indeterminate = someChecked && !allChecked;
}

function clearSelection() {
  selectedPaths.clear();
  isAllPagesSelected = false;
  document.querySelectorAll('.page-checkbox').forEach(cb => { cb.checked = false; });
  const selectAll = document.getElementById('select-all-checkbox');
  if (selectAll) {
    selectAll.checked = false;
    selectAll.indeterminate = false;
  }
  const banner = document.getElementById('select-all-banner');
  if (banner) banner.style.display = 'none';
  updateBatchBar();
}

function selectAllPages() {
  const token = getActiveToken();
  if (!token) return;

  const body = isSearchMode ? 'query=' + encodeURIComponent(currentSearchQuery) : '';

  fetch('/pages/paths', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/x-www-form-urlencoded',
      'Authorization': 'Bearer ' + token
    },
    body: body
  })
  .then(r => r.json())
  .then(result => {
    // Cache still building — show loading message and auto-retry
    if (result.building && result.paths.length === 0) {
      showToast(_t('cache_building', 'Page cache is still building, retrying...'), 'success');
      setTimeout(selectAllPages, 2000);
      return;
    }

    result.paths.forEach(p => selectedPaths.add(p));
    isAllPagesSelected = true;

    // Switch banner to "all selected" state
    const prompt = document.getElementById('banner-select-prompt');
    const allSel = document.getElementById('banner-all-selected');
    if (prompt) prompt.style.display = 'none';
    if (allSel) allSel.style.display = '';

    // Check all visible checkboxes
    document.querySelectorAll('.page-checkbox').forEach(cb => { cb.checked = true; });
    updateSelectAllCheckbox();
    updateBatchBar();
  })
  .catch(() => {
    showToast(_t('cache_building', 'Page cache is still building, please try again.'), 'error');
  });
}

function updateBatchBar() {
  const bar = document.getElementById('batch-action-bar');
  const count = document.getElementById('batch-count');
  if (!bar) return;

  if (selectedPaths.size > 0) {
    bar.style.display = 'flex';
    count.textContent = _t('selected_count', '{count} selected', {count: selectedPaths.size});
  } else {
    bar.style.display = 'none';
  }
}

function restoreSelectionState() {
  document.querySelectorAll('.page-checkbox').forEach(cb => {
    cb.checked = selectedPaths.has(cb.dataset.path);
  });
  updateSelectAllCheckbox();
  updateBatchBar();
}

function batchDelete() {
  const count = selectedPaths.size;
  if (count === 0) return;

  if (!confirm(_t('confirm_batch_delete', 'Delete {count} page(s)? This action cannot be undone.', {count: count}))) {
    return;
  }

  const token = getActiveToken();
  if (!token) {
    showToast(_t('select_token', 'Please select a token first.'), 'error');
    return;
  }

  // Show loading overlay
  const overlay = document.createElement('div');
  overlay.className = 'batch-loading-overlay';
  overlay.id = 'batch-loading-overlay';
  const spinner = document.createElement('div');
  spinner.className = 'batch-loading-content';
  spinner.textContent = _t('deleting_count', 'Deleting {count} page(s)...', {count: count});
  overlay.appendChild(spinner);
  document.body.appendChild(overlay);

  // Chunk paths into small batches for progressive UI updates.
  // Server limit is 50/request; chunk size of 5 gives ~2s feedback cycles.
  const allPaths = Array.from(selectedPaths);
  const chunkSize = 5;
  const chunks = [];
  for (let i = 0; i < allPaths.length; i += chunkSize) {
    chunks.push(allPaths.slice(i, i + chunkSize));
  }

  let totalSucceeded = [];
  let totalFailed = [];

  function processChunk(index) {
    if (index >= chunks.length) {
      // All chunks done — remove overlay
      const ol = document.getElementById('batch-loading-overlay');
      if (ol) ol.remove();

      // Show result toast
      if (totalFailed.length === 0) {
        showToast(_t('delete_success', '{count} page(s) deleted successfully.', {count: totalSucceeded.length}), 'success');
      } else {
        showToast(
          _t('delete_partial', '{succeeded} succeeded, {failed} failed: {paths}', {
            succeeded: totalSucceeded.length,
            failed: totalFailed.length,
            paths: totalFailed.map(f => f.path).join(', ')
          }),
          'error'
        );
      }

      clearSelection();
      return;
    }

    // Update overlay progress
    const spinnerEl = document.querySelector('.batch-loading-content');
    if (spinnerEl) {
      const processed = totalSucceeded.length + totalFailed.length;
      spinnerEl.textContent = _t('deleting_count', 'Deleting {count} page(s)...', {count: count}) +
        ' (' + processed + '/' + allPaths.length + ')';
    }

    const paths = chunks[index].join(',');
    fetch('/pages/batch-delete', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/x-www-form-urlencoded',
        'Authorization': 'Bearer ' + token
      },
      body: 'paths=' + encodeURIComponent(paths)
    })
    .then(response => {
      if (!response.ok) {
        return response.text().then(text => { throw new Error(text); });
      }
      return response.json();
    })
    .then(result => {
      // Update DOM in-place for succeeded paths
      result.succeeded.forEach(path => {
        const cb = document.querySelector('.page-checkbox[data-path="' + CSS.escape(path) + '"]');
        if (!cb) return;
        const row = cb.closest('tr');
        if (!row) return;
        row.classList.add('row-deleted');
        const titleCell = row.querySelector('.page-title');
        if (titleCell) titleCell.textContent = '[DELETED]';
        const actionsCell = row.querySelector('.actions');
        if (actionsCell) {
          while (actionsCell.firstChild) actionsCell.removeChild(actionsCell.firstChild);
          const span = document.createElement('span');
          span.className = 'text-muted';
          span.textContent = _t('deleted', 'Deleted');
          actionsCell.appendChild(span);
        }
        cb.remove();
      });

      totalSucceeded = totalSucceeded.concat(result.succeeded);
      totalFailed = totalFailed.concat(result.failed);

      // Process next chunk
      processChunk(index + 1);
    })
    .catch(err => {
      const ol = document.getElementById('batch-loading-overlay');
      if (ol) ol.remove();
      showToast(_t('batch_delete_failed', 'Batch delete failed: {message}', {message: err.message}), 'error');
    });
  }

  processChunk(0);
}

// ── Page Operations ─────────────────────────────────────────

let currentOffset = 0;
let currentLimit = 50;
let currentSearchQuery = '';
let isSearchMode = false;
let currentSort = 'default';

function loadPages(offset, limit) {
  const token = getActiveToken();
  if (!token) {
    showToast(_t('select_token', 'Please select a token first.'), 'error');
    return;
  }

  if (offset !== undefined) currentOffset = offset;
  if (limit !== undefined) currentLimit = limit;

  htmx.ajax('POST', '/pages/list', {
    target: '#main-content',
    swap: 'innerHTML',
    values: { offset: currentOffset, limit: currentLimit, sort: currentSort }
  });

  isSearchMode = false;
  currentSearchQuery = '';
}

function nextPage() {
  if (isSearchMode) {
    searchPages(currentSearchQuery, currentOffset + currentLimit, currentLimit);
  } else {
    loadPages(currentOffset + currentLimit, currentLimit);
  }
}

function prevPage() {
  const newOffset = Math.max(0, currentOffset - currentLimit);
  if (isSearchMode) {
    searchPages(currentSearchQuery, newOffset, currentLimit);
  } else {
    loadPages(newOffset, currentLimit);
  }
}

function goToPage(page) {
  var bar = document.querySelector('.pagination-bar');
  var totalPages = bar ? Number(bar.dataset.totalPages) : 1;
  page = Math.max(1, Math.min(totalPages, page));
  var newOffset = (page - 1) * currentLimit;
  if (isSearchMode) {
    searchPages(currentSearchQuery, newOffset, currentLimit);
  } else {
    loadPages(newOffset, currentLimit);
  }
}

function jumpPages(delta) {
  var bar = document.querySelector('.pagination-bar');
  var currentPage = bar ? Number(bar.dataset.currentPage) : 1;
  goToPage(currentPage + delta);
}

function changePageSize(newLimit) {
  if (isSearchMode) {
    searchPages(currentSearchQuery, 0, newLimit);
  } else {
    loadPages(0, newLimit);
  }
}

function searchPages(query, offset, limit) {
  const token = getActiveToken();
  if (!token) {
    showToast(_t('select_token', 'Please select a token first.'), 'error');
    return;
  }

  query = query || '';
  if (!query.trim()) {
    clearSearch();
    return;
  }

  currentSearchQuery = query;
  isSearchMode = true;
  if (offset !== undefined) currentOffset = offset; else currentOffset = 0;
  if (limit !== undefined) currentLimit = limit;

  htmx.ajax('POST', '/pages/search', {
    target: '#main-content',
    swap: 'innerHTML',
    values: {
      query: currentSearchQuery,
      offset: currentOffset,
      limit: currentLimit,
      sort: currentSort
    }
  });
}

function changeSortOrder(sort) {
  currentSort = sort;
  if (isSearchMode) {
    searchPages(currentSearchQuery, 0, currentLimit);
  } else {
    loadPages(0, currentLimit);
  }
}

function clearSearch() {
  currentSearchQuery = '';
  isSearchMode = false;
  loadPages(0, currentLimit);
}

function loadAccountInfo() {
  const token = getActiveToken();
  if (!token) {
    showToast(_t('select_token', 'Please select a token first.'), 'error');
    return;
  }

  htmx.ajax('POST', '/account/info', {
    target: '#account-info-content',
    swap: 'innerHTML'
  });
}

function revokeToken() {
  const token = getActiveToken();
  if (!token) {
    showToast(_t('select_token', 'Please select a token first.'), 'error');
    return;
  }

  if (!confirm(_t('confirm_revoke', 'Are you sure? This will invalidate the current token and generate a new one.'))) {
    return;
  }

  htmx.ajax('POST', '/account/revoke', {
    target: '#account-result',
    swap: 'innerHTML'
  });
}

// ── Content Editor Helpers ──────────────────────────────────

function formatContent() {
  const textarea = document.getElementById('page-content');
  if (!textarea) return;

  try {
    const parsed = JSON.parse(textarea.value);
    textarea.value = JSON.stringify(parsed, null, 2);
  } catch (e) {
    showToast(_t('invalid_json', 'Invalid JSON: {message}', {message: e.message}), 'error');
  }
}


// ── Theme Toggle ────────────────────────────────────────────

function toggleTheme() {
  const html = document.documentElement;
  // Dark is the baked-in default from base.html; fall back to it if the
  // attribute is somehow missing so the first toggle always flips to light.
  const current = html.getAttribute('data-theme') || 'dark';
  const next = current === 'dark' ? 'light' : 'dark';
  html.setAttribute('data-theme', next);
  localStorage.setItem('telegraph_hub_theme', next);
}

function loadTheme() {
  const saved = localStorage.getItem('telegraph_hub_theme');
  if (saved) {
    document.documentElement.setAttribute('data-theme', saved);
  }
}

// ── Toast Helper (DOM-safe, no innerHTML) ───────────────────

function showToast(message, variant) {
  const container = document.getElementById('toast-container');
  if (!container) return;

  const toast = document.createElement('div');
  toast.className = 'toast toast-' + (variant || 'success');
  toast.setAttribute('role', 'alert');

  const strong = document.createElement('strong');
  strong.textContent = message;
  toast.appendChild(strong);

  const closeBtn = document.createElement('button');
  closeBtn.className = 'toast-close';
  closeBtn.textContent = '\u00d7';
  closeBtn.addEventListener('click', () => toast.remove());
  toast.appendChild(closeBtn);

  container.appendChild(toast);

  // Auto-dismiss after 5 seconds
  setTimeout(() => toast.remove(), 5000);
}

// ── Clipboard ───────────────────────────────────────────────

function copyToClipboard(text) {
  navigator.clipboard.writeText(text).then(() => {
    showToast(_t('copied', 'Copied to clipboard!'), 'success');
  }).catch(() => {
    // Fallback for older browsers
    const ta = document.createElement('textarea');
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand('copy');
    ta.remove();
    showToast(_t('copied', 'Copied to clipboard!'), 'success');
  });
}

// ── Row Height Normalization ────────────────────────────
// Keep every row in #pages-table at the same height as the tallest one
// so multi-line titles don't create visually uneven pagination rows.

function normalizeRowHeights() {
  const table = document.getElementById('pages-table');
  if (!table) return;
  const rows = table.querySelectorAll('tbody tr');
  if (rows.length === 0) return;

  // Reset inline height before re-measurement to avoid stale values
  rows.forEach(function (r) { r.style.height = ''; });

  // Defer measurement until the browser has re-laid-out with the cleared heights
  requestAnimationFrame(function () {
    var maxH = 0;
    rows.forEach(function (r) {
      var h = r.offsetHeight;
      if (h > maxH) maxH = h;
    });
    if (maxH <= 0) return;
    rows.forEach(function (r) { r.style.height = maxH + 'px'; });
  });
}

// Debounced variant for window resize — title column wrap point shifts
// with viewport width, so re-measure once the user stops dragging.
var _rowHeightResizeTimer = null;
function scheduleRowHeightNormalize() {
  if (_rowHeightResizeTimer) clearTimeout(_rowHeightResizeTimer);
  _rowHeightResizeTimer = setTimeout(normalizeRowHeights, 150);
}

window.addEventListener('resize', scheduleRowHeightNormalize);

// ── HTMX Event Hooks ───────────────────────────────────────

// Attach `Authorization: Bearer <token>` to every HTMX request bound for a
// token-gated endpoint. The Authorization header is chosen over a form field
// or query parameter because reverse-proxy log redaction (nginx, Caddy,
// Datadog, New Relic, etc.) masks `Authorization` by default but logs form
// bodies and query strings verbatim. Header transport also works identically
// for GET, POST, PUT, PATCH and DELETE — no verb branching required.
document.addEventListener('htmx:configRequest', function(e) {
  const token = getActiveToken();
  if (!token) return;
  const path = e.detail.path;
  if (path.startsWith('/pages/') || path.startsWith('/account/')) {
    e.detail.headers['Authorization'] = 'Bearer ' + token;
  }
});

// Restore checkbox selection state and banner after HTMX swaps new page list content
document.addEventListener('htmx:afterSettle', function(e) {
  if (document.querySelector('.page-checkbox')) {
    restoreSelectionState();
    // Restore select-all banner state if cross-page selection is active
    if (isAllPagesSelected) {
      const banner = document.getElementById('select-all-banner');
      if (banner) {
        banner.style.display = '';
        const prompt = document.getElementById('banner-select-prompt');
        const allSel = document.getElementById('banner-all-selected');
        if (prompt) prompt.style.display = 'none';
        if (allSel) allSel.style.display = '';
      }
    }
  }

  // Re-normalize row heights after any swap that may replace pages-table rows
  normalizeRowHeights();
});

// ── Preview Panel ──────────────────────────────────────

function openPreview() {
  var panel = document.getElementById('preview-panel');
  var main = document.querySelector('main.container');
  if (panel) panel.style.display = 'flex';
  if (main) main.classList.add('preview-active');
}

function closePreview() {
  var panel = document.getElementById('preview-panel');
  var main = document.querySelector('main.container');
  if (panel) {
    panel.style.display = 'none';
    while (panel.firstChild) panel.removeChild(panel.firstChild);
  }
  if (main) main.classList.remove('preview-active');
}

function openInNewTab(url) {
  window.open(url, '_blank', 'noopener');
}

// ── Initialization ──────────────────────────────────────────

document.addEventListener('DOMContentLoaded', function() {
  loadTheme();
  refreshTokenSelect();
  renderSavedTokens();
  normalizeRowHeights();
});
