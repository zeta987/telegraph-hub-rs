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
  placeholder.textContent = '-- Select a token --';
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
  // Update editor hidden field if present
  const editorToken = document.getElementById('editor-token');
  if (editorToken) editorToken.value = token;

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
  showToast('Token imported successfully!', 'success');
}

// Save token from the account creation result
function saveTokenFromResult(name, token) {
  saveToken(name || 'Unnamed Account', token);
  renderSavedTokens();
  showToast('Token saved to browser!', 'success');
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
    p.textContent = 'No saved tokens yet.';
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
    valueSpan.textContent = token;
    info.appendChild(nameSpan);
    info.appendChild(document.createTextNode(' '));
    info.appendChild(valueSpan);

    const actions = document.createElement('div');
    actions.className = 'btn-group';

    const copyBtn = document.createElement('button');
    copyBtn.className = 'btn btn-xs btn-outline';
    copyBtn.textContent = 'Copy';
    copyBtn.addEventListener('click', () => copyToClipboard(token));

    const removeBtn = document.createElement('button');
    removeBtn.className = 'btn btn-xs btn-danger';
    removeBtn.textContent = 'Remove';
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
    showToast('No tokens to export.', 'error');
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
  showToast('Tokens exported!', 'success');
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
          showToast('Invalid token file format.', 'error');
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
        showToast('Imported ' + count + ' token(s)!', 'success');
      } catch {
        showToast('Failed to parse token file.', 'error');
      }
    };
    reader.readAsText(file);
  });
  input.click();
}

// ── Batch Selection ─────────────────────────────────────────

const selectedPaths = new Set();

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
  checkboxes.forEach(cb => {
    cb.checked = selectAll.checked;
    if (selectAll.checked) {
      selectedPaths.add(cb.dataset.path);
    } else {
      selectedPaths.delete(cb.dataset.path);
    }
  });
  updateBatchBar();
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
  document.querySelectorAll('.page-checkbox').forEach(cb => { cb.checked = false; });
  const selectAll = document.getElementById('select-all-checkbox');
  if (selectAll) {
    selectAll.checked = false;
    selectAll.indeterminate = false;
  }
  updateBatchBar();
}

function updateBatchBar() {
  const bar = document.getElementById('batch-action-bar');
  const count = document.getElementById('batch-count');
  if (!bar) return;

  if (selectedPaths.size > 0) {
    bar.style.display = 'flex';
    count.textContent = selectedPaths.size + ' selected';
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

  if (count > 50) {
    showToast('Maximum batch size is 50 pages. Please deselect some pages.', 'error');
    return;
  }

  if (!confirm('Delete ' + count + ' page(s)? This action cannot be undone.')) {
    return;
  }

  const token = getActiveToken();
  if (!token) {
    showToast('Please select a token first.', 'error');
    return;
  }

  // Show loading overlay
  const overlay = document.createElement('div');
  overlay.className = 'batch-loading-overlay';
  overlay.id = 'batch-loading-overlay';
  const spinner = document.createElement('div');
  spinner.className = 'batch-loading-content';
  spinner.textContent = 'Deleting ' + count + ' page(s)...';
  overlay.appendChild(spinner);
  document.body.appendChild(overlay);

  const paths = Array.from(selectedPaths).join(',');

  fetch('/pages/batch-delete', {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: 'access_token=' + encodeURIComponent(token) + '&paths=' + encodeURIComponent(paths)
  })
  .then(response => {
    if (!response.ok) {
      return response.text().then(text => { throw new Error(text); });
    }
    return response.json();
  })
  .then(result => {
    // Remove overlay
    const ol = document.getElementById('batch-loading-overlay');
    if (ol) ol.remove();

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
        span.textContent = 'Deleted';
        actionsCell.appendChild(span);
      }
      cb.remove();
    });

    // Show result toast
    if (result.failed.length === 0) {
      showToast(result.succeeded.length + ' page(s) deleted successfully.', 'success');
    } else {
      showToast(
        result.succeeded.length + ' succeeded, ' + result.failed.length + ' failed: ' +
        result.failed.map(f => f.path).join(', '),
        'error'
      );
    }

    clearSelection();
  })
  .catch(err => {
    const ol = document.getElementById('batch-loading-overlay');
    if (ol) ol.remove();
    showToast('Batch delete failed: ' + err.message, 'error');
  });
}

// ── Page Operations ─────────────────────────────────────────

let currentOffset = 0;
let currentLimit = 50;
let currentSearchQuery = '';
let isSearchMode = false;

function loadPages(offset, limit) {
  const token = getActiveToken();
  if (!token) {
    showToast('Please select a token first.', 'error');
    return;
  }

  if (offset !== undefined) currentOffset = offset;
  if (limit !== undefined) currentLimit = limit;

  htmx.ajax('POST', '/pages/list', {
    target: '#main-content',
    swap: 'innerHTML',
    values: { access_token: token, offset: currentOffset, limit: currentLimit }
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
    showToast('Please select a token first.', 'error');
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
      access_token: token,
      query: currentSearchQuery,
      offset: currentOffset,
      limit: currentLimit
    }
  });
}

function clearSearch() {
  currentSearchQuery = '';
  isSearchMode = false;
  loadPages(0, currentLimit);
}

function loadAccountInfo() {
  const token = getActiveToken();
  if (!token) {
    showToast('Please select a token first.', 'error');
    return;
  }

  htmx.ajax('POST', '/account/info', {
    target: '#account-info-content',
    swap: 'innerHTML',
    values: { access_token: token }
  });
}

function revokeToken() {
  const token = getActiveToken();
  if (!token) {
    showToast('Please select a token first.', 'error');
    return;
  }

  if (!confirm('Are you sure? This will invalidate the current token and generate a new one.')) {
    return;
  }

  htmx.ajax('POST', '/account/revoke', {
    target: '#account-result',
    swap: 'innerHTML',
    values: { access_token: token }
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
    showToast('Invalid JSON: ' + e.message, 'error');
  }
}


// ── Theme Toggle ────────────────────────────────────────────

function toggleTheme() {
  const html = document.documentElement;
  const current = html.getAttribute('data-theme');
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
    showToast('Copied to clipboard!', 'success');
  }).catch(() => {
    // Fallback for older browsers
    const ta = document.createElement('textarea');
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand('copy');
    ta.remove();
    showToast('Copied to clipboard!', 'success');
  });
}

// ── HTMX Event Hooks ───────────────────────────────────────

// Inject access_token before HTMX requests that need it
document.addEventListener('htmx:configRequest', function(e) {
  const token = getActiveToken();
  if (token && e.detail.path.startsWith('/pages/')) {
    // For pages endpoints, add token to the request
    if (!e.detail.parameters.access_token) {
      e.detail.parameters.access_token = token;
    }
  }
});

// Restore checkbox selection state after HTMX swaps new page list content
document.addEventListener('htmx:afterSettle', function(e) {
  if (document.querySelector('.page-checkbox')) {
    restoreSelectionState();
  }
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
});
