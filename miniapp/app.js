function qs(name) {
  const url = new URL(window.location.href);
  return url.searchParams.get(name);
}

function b64urlToUtf8(b64url) {
  const b64 = b64url.replace(/-/g, '+').replace(/_/g, '/');
  const pad = '='.repeat((4 - (b64.length % 4)) % 4);
  const bin = atob(b64 + pad);
  const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
  return new TextDecoder('utf-8').decode(bytes);
}

function safeFolderName(name) {
  if (!name) return false;
  if (name.includes('..')) return false;
  if (name.includes('/') || name.includes('\\')) return false;
  return true;
}

const tg = window.Telegram?.WebApp;
if (tg) {
  tg.ready();
  tg.expand();
}

let folders = [];
try {
  const raw = qs('folders');
  if (raw) {
    const json = b64urlToUtf8(raw);
    const parsed = JSON.parse(json);
    if (Array.isArray(parsed)) folders = parsed.filter((x) => typeof x === 'string');
  }
} catch (e) {
  console.warn('failed to parse folders', e);
}

folders = folders.filter(safeFolderName);

const filterEl = document.getElementById('filter');
const listEl = document.getElementById('folders');
const startEl = document.getElementById('start');
const hintEl = document.getElementById('hint');

let selected = null;

function currentEngine() {
  const el = document.querySelector('input[name="engine"]:checked');
  return el?.value || 'opencode';
}

function render() {
  const q = (filterEl.value || '').toLowerCase();
  const visible = folders.filter((f) => f.toLowerCase().includes(q));

  listEl.innerHTML = '';
  if (visible.length === 0) {
    hintEl.textContent = folders.length === 0 ? 'No folders provided by bot.' : 'No matching folders.';
  } else {
    hintEl.textContent = '';
  }

  for (const name of visible) {
    const div = document.createElement('div');
    div.className = 'item' + (selected === name ? ' active' : '');
    div.innerHTML = `<span>${name}</span><span class="badge">~/Projects/${name}</span>`;
    div.onclick = () => {
      selected = name;
      startEl.disabled = false;
      render();
    };
    listEl.appendChild(div);
  }

  startEl.disabled = !selected;
}

filterEl.addEventListener('input', render);

startEl.addEventListener('click', () => {
  if (!selected) return;
  const payload = {
    engine: currentEngine(),
    folder: selected,
  };
  const data = JSON.stringify(payload);
  if (tg) {
    tg.sendData(data);
  } else {
    alert(data);
  }
});

render();
