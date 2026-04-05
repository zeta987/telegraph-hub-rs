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

// ── Page Operations ─────────────────────────────────────────

function loadPages() {
  const token = getActiveToken();
  if (!token) {
    showToast('Please select a token first.', 'error');
    return;
  }

  htmx.ajax('POST', '/pages/list', {
    target: '#main-content',
    swap: 'innerHTML',
    values: { access_token: token, limit: 200 }
  });
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

// ── Filter Pages ────────────────────────────────────────────

function filterPages() {
  const query = document.getElementById('page-search').value.toLowerCase();
  const rows = document.querySelectorAll('.page-row');

  rows.forEach(row => {
    const title = (row.querySelector('.page-title')?.textContent || '').toLowerCase();
    const path = (row.querySelector('.page-path')?.textContent || '').toLowerCase();
    row.style.display = (title.includes(query) || path.includes(query)) ? '' : 'none';
  });
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

// ── Initialization ──────────────────────────────────────────

document.addEventListener('DOMContentLoaded', function() {
  loadTheme();
  refreshTokenSelect();
  renderSavedTokens();
});
