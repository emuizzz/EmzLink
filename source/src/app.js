(() => {
  const tauri = window.__TAURI__;
  let invoke = tauri?.core?.invoke;
  let listen = tauri?.event?.listen;

  // Browser preview fallback mock
  if (!invoke) {
    const mockState = {
      installed: true,
      running: false,
      game_mode: 'off',
      ipset: 'none',
      version: '1.2.0',
      settings: {
        zapret_dir: 'C:\\ProgramData\\zapret',
        style: 'win11',
        mode: 'system',
        accent: '#0067C0',
        strategy: 'general (ALT).bat',
        bypass_autostart: false,
        app_autostart: false,
        services: { youtube: true, discord: true },
        custom_domains: [],
        tg_proxy: false,
        tg_autostart: false,
        alt_results: {
          'general (ALT).bat': { status: 'good', ms: 22 },
          'general (ALT8).bat': { status: 'good', ms: 28 },
          'general (ALT2).bat': { status: 'good', ms: 34 },
          'general (FAKE TLS AUTO).bat': { status: 'good', ms: 38 },
          'general (ALT3).bat': { status: 'good', ms: 42 },
          'general (ALT4).bat': { status: 'good', ms: 48 },
          'general (ALT5).bat': { status: 'mid', ms: 68 },
          'general.bat': { status: 'mid', ms: 75 },
          'general (ALT6).bat': { status: 'mid', ms: 85 },
          'general (ALT7).bat': { status: 'mid', ms: 92 },
          'general (SIMPLE FAKE).bat': { status: 'mid', ms: 110 },
          'general (ALT9).bat': { status: 'bad', ms: 160 },
          'general (ALT10).bat': { status: 'bad', ms: 195 }
        }
      },
      strategies: [
        { file: 'general (ALT).bat', name: 'ALT 1' },
        { file: 'general (ALT2).bat', name: 'ALT 2' },
        { file: 'general (ALT3).bat', name: 'ALT 3' },
        { file: 'general (ALT4).bat', name: 'ALT 4' },
        { file: 'general (ALT5).bat', name: 'ALT 5' },
        { file: 'general (ALT6).bat', name: 'ALT 6' },
        { file: 'general (ALT7).bat', name: 'ALT 7' },
        { file: 'general (ALT8).bat', name: 'ALT 8' },
        { file: 'general (ALT9).bat', name: 'ALT 9' },
        { file: 'general (ALT10).bat', name: 'ALT 10' },
        { file: 'general.bat', name: 'Стандартный' },
        { file: 'general (FAKE TLS AUTO).bat', name: 'FAKE TLS AUTO' },
        { file: 'general (SIMPLE FAKE).bat', name: 'SIMPLE FAKE' }
      ],
      catalog: [
        { id: 'youtube', name: 'YouTube', group: 'main' },
        { id: 'discord', name: 'Discord', group: 'main' },
        { id: 'twitch', name: 'Twitch', group: 'exp' },
        { id: 'steam', name: 'Steam', group: 'exp' },
        { id: 'hltv', name: 'HLTV', group: 'exp' },
        { id: 'modrinth', name: 'Modrinth', group: 'exp' },
        { id: 'roblox', name: 'Roblox', group: 'exp' },
        { id: 'epic', name: 'Epic Games', group: 'exp' },
        { id: 'x', name: 'X (Twitter)', group: 'exp' }
      ],
      tg: { installed: true, running: false, version: '1.4.1', link: 'tg://proxy?server=127.0.0.1&port=1443&secret=ee...' }
    };
    invoke = async (cmd, args) => {
      if (cmd === 'get_state') return JSON.parse(JSON.stringify(mockState));
      if (cmd === 'get_running') return mockState.running;
      if (cmd === 'start_bypass') { mockState.running = true; return true; }
      if (cmd === 'stop_bypass') { mockState.running = false; return false; }
      if (cmd === 'select_strategy') { mockState.settings.strategy = args.file; return true; }
      if (cmd === 'set_theme') { Object.assign(mockState.settings, args); return true; }
      if (cmd === 'set_service') { mockState.settings.services[args.id] = args.enabled; return true; }
      if (cmd === 'add_domain') { mockState.settings.custom_domains.push(args.domain); return mockState.settings.custom_domains; }
      if (cmd === 'remove_domain') { mockState.settings.custom_domains = mockState.settings.custom_domains.filter(d => d !== args.domain); return mockState.settings.custom_domains; }
      if (cmd === 'set_game_mode') { mockState.game_mode = args.mode; return args.mode; }
      if (cmd === 'set_ipset_mode') { mockState.ipset = args.mode; return args.mode; }
      if (cmd === 'update_ipset') return 'loaded';
      if (cmd === 'tg_state') return mockState.tg;
      if (cmd === 'tg_toggle') { mockState.tg.running = args.enabled; mockState.settings.tg_proxy = args.enabled; return mockState.tg; }
      if (cmd === 'test_strategies') return true;
      return null;
    };
    listen = () => Promise.resolve(() => {});
  }

  const $ = (id) => document.getElementById(id);
  const root = document.documentElement;

  const SWATCHES = ['#6750A4', '#1A73E8', '#0067C0', '#00897B', '#2E7D32', '#E8710A', '#D81B60', '#C62828'];
  const DEFAULT_ACCENT = { material: '#6750A4', win11: '#0067C0', terminal: '#33FF66', nothing: '#D71921' };
  const THEME_VARS = ['--bg','--nav','--surface','--surface2','--text','--text2','--outline','--primary','--on-primary','--pc','--on-pc','--tert',
    '--b1','--b2','--b3','--g1','--g2','--g3','--glass-edge','--glass-hi','--glass-lo','--app-bg','--win-bg','--good','--led','--thumb','--track','--frame'];

  let S = null;
  let busy = false;
  let tgBusy = false;
  let testingFile = null;
  let restartTimer = null;
  let toastTimer = null;

  const toast = (msg) => {
    let text = String(msg == null ? '' : msg).replace(/^Error:\s*/, '').trim();
    if (text.includes('CLIXML')) text = 'Не удалось выполнить действие в PowerShell. Повторите попытку.';
    const full = text;
    if (text.length > 220) text = text.slice(0, 217) + '…';
    const isErr = /не удалось|ошибк|error|exception|не найден|невозможно|отказано|denied|нет доступа/i.test(full);
    const t = $('toast');
    t.textContent = text;
    t.title = full;
    t.classList.toggle('err', isErr);
    t.classList.add('show');
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => t.classList.remove('show'), isErr ? 6500 : 4200);
  };

  // ───────── dot-matrix text (Nothing OS Ndot LED matrix) ─────────
  const NDOT_5X7 = {
    'А': [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11], 'Б': [0x1e, 0x10, 0x1e, 0x11, 0x11, 0x11, 0x1e],
    'В': [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e], 'Г': [0x1f, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10],
    'Д': [0x0e, 0x0a, 0x0a, 0x0a, 0x1f, 0x11, 0x11], 'Е': [0x1f, 0x10, 0x1e, 0x10, 0x10, 0x10, 0x1f],
    'Ё': [0x0a, 0x1f, 0x10, 0x1e, 0x10, 0x10, 0x1f], 'Ж': [0x15, 0x15, 0x0e, 0x04, 0x0e, 0x15, 0x15],
    'З': [0x1e, 0x01, 0x01, 0x0e, 0x01, 0x01, 0x1e], 'И': [0x11, 0x13, 0x15, 0x19, 0x11, 0x11, 0x11],
    'Й': [0x0a, 0x04, 0x11, 0x13, 0x15, 0x19, 0x11], 'К': [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
    'Л': [0x07, 0x09, 0x09, 0x09, 0x11, 0x11, 0x11], 'М': [0x11, 0x1b, 0x15, 0x11, 0x11, 0x11, 0x11],
    'Н': [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11], 'О': [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
    'П': [0x1f, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11], 'Р': [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
    'С': [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e], 'Т': [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
    'У': [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04], 'Ф': [0x04, 0x0e, 0x15, 0x15, 0x0e, 0x04, 0x04],
    'Х': [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11], 'Ц': [0x11, 0x11, 0x11, 0x11, 0x1f, 0x01, 0x01],
    'Ч': [0x11, 0x11, 0x11, 0x0f, 0x01, 0x01, 0x01], 'Ш': [0x15, 0x15, 0x15, 0x15, 0x1f, 0x11, 0x11],
    'Щ': [0x15, 0x15, 0x15, 0x1f, 0x01, 0x01, 0x01], 'Ъ': [0x18, 0x08, 0x0e, 0x09, 0x09, 0x0e, 0x00],
    'Ы': [0x12, 0x12, 0x1e, 0x12, 0x12, 0x1e, 0x12], 'Ь': [0x10, 0x10, 0x1e, 0x11, 0x11, 0x11, 0x1e],
    'Э': [0x0e, 0x01, 0x01, 0x07, 0x01, 0x01, 0x0e], 'Ю': [0x12, 0x17, 0x15, 0x1d, 0x15, 0x15, 0x17],
    'Я': [0x0f, 0x11, 0x11, 0x0f, 0x05, 0x09, 0x11],
    // Latin
    'A': [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11], 'B': [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
    'C': [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e], 'D': [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
    'E': [0x1f, 0x10, 0x1e, 0x10, 0x10, 0x10, 0x1f], 'F': [0x1f, 0x10, 0x1e, 0x10, 0x10, 0x10, 0x10],
    'G': [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f], 'H': [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
    'I': [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e], 'J': [0x01, 0x01, 0x01, 0x01, 0x11, 0x11, 0x0e],
    'K': [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11], 'L': [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
    'M': [0x11, 0x1b, 0x15, 0x11, 0x11, 0x11, 0x11], 'N': [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
    'O': [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e], 'P': [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
    'Q': [0x0e, 0x11, 0x11, 0x11, 0x15, 0x09, 0x16], 'R': [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
    'S': [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e], 'T': [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
    'U': [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e], 'V': [0x11, 0x11, 0x11, 0x11, 0x0a, 0x0a, 0x04],
    'W': [0x11, 0x11, 0x11, 0x15, 0x15, 0x1b, 0x11], 'X': [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
    'Y': [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04], 'Z': [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
    // Digits
    '0': [0x0e, 0x13, 0x15, 0x19, 0x11, 0x11, 0x0e], '1': [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
    '2': [0x0e, 0x11, 0x01, 0x06, 0x08, 0x10, 0x1f], '3': [0x1e, 0x01, 0x02, 0x0c, 0x01, 0x11, 0x0e],
    '4': [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02], '5': [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e],
    '6': [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e], '7': [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
    '8': [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e], '9': [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c],
    '-': [0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00], '.': [0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x0c],
    ':': [0x00, 0x0c, 0x0c, 0x00, 0x0c, 0x0c, 0x00], '…': [0x00, 0x00, 0x00, 0x00, 0x00, 0x15, 0x15],
    ' ': [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
  };

  const isDots = () => root.dataset.style === 'nothing';

  function drawDots(el) {
    if (!isDots()) {
      const oldCv = el.querySelector('canvas');
      if (oldCv) oldCv.remove();
      return;
    }
    const text = el.dataset.t || el.textContent || '';
    if (!text) return;
    const p = parseFloat(el.dataset.p || '3.5');
    const colsPerChar = 5, rows = 7, charSpacing = 1;
    const totalCols = Math.max(1, text.length * (colsPerChar + charSpacing) - charSpacing);
    const w = totalCols, h = rows;
    let cv = el.querySelector('canvas');
    if (!cv) {
      cv = document.createElement('canvas');
      el.appendChild(cv);
    }
    const dpr = Math.max(2, window.devicePixelRatio || 1);
    cv.width = Math.round(w * p * dpr);
    cv.height = Math.round(h * p * dpr);
    cv.style.width = (w * p) + 'px';
    cv.style.height = (h * p) + 'px';
    const g = cv.getContext('2d');
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, w * p, h * p);

    const activeColor = '#D71921';

    let colOffset = 0;
    for (let i = 0; i < text.length; i++) {
      const ch = text[i];
      const upper = ch.toUpperCase();
      const glyph = NDOT_5X7[upper] || NDOT_5X7[ch];
      if (glyph) {
        for (let r = 0; r < rows; r++) {
          const rowBits = glyph[r];
          for (let c = 0; c < 5; c++) {
            if (rowBits & (1 << (4 - c))) {
              g.beginPath();
              g.arc((colOffset + c) * p + p / 2, r * p + p / 2, p * 0.38, 0, Math.PI * 2);
              g.fillStyle = activeColor;
              g.fill();
            }
          }
        }
      }
      colOffset += colsPerChar + charSpacing;
    }
    el.classList.add('has-dots');
  }

  function setDT(el, text) {
    if (!el) return;
    el.dataset.t = text;
    el.textContent = text;
    if (isDots()) {
      drawDots(el);
    } else {
      const cv = el.querySelector('canvas');
      if (cv) cv.remove();
      el.classList.remove('has-dots');
    }
  }

  function renderAllDots() {
    document.querySelectorAll('.dt').forEach((el) => {
      const t = el.dataset.t || el.textContent;
      el.dataset.t = t;
      el.textContent = t;
      if (isDots()) {
        drawDots(el);
      } else {
        const cv = el.querySelector('canvas');
        if (cv) cv.remove();
        el.classList.remove('has-dots');
      }
    });
  }

  // ───────── colors / theme ─────────
  const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
  function hexToHsl(hex) {
    const n = parseInt(hex.replace('#', ''), 16);
    const r = ((n >> 16) & 255) / 255, g = ((n >> 8) & 255) / 255, b = (n & 255) / 255;
    const mx = Math.max(r, g, b), mn = Math.min(r, g, b), d = mx - mn;
    let h = 0;
    if (d) { if (mx === r) h = ((g - b) / d) % 6; else if (mx === g) h = (b - r) / d + 2; else h = (r - g) / d + 4; }
    h = Math.round((h * 60 + 360) % 360);
    const l = (mx + mn) / 2;
    const s = d ? d / (1 - Math.abs(2 * l - 1)) : 0;
    return { h, s: Math.round(s * 100), l: Math.round(l * 100) };
  }
  const hsl = (h, s, l) => `hsl(${(h + 360) % 360} ${s}% ${l}%)`;
  function hslHex(h, s, l) {
    h = ((h % 360) + 360) % 360; s /= 100; l /= 100;
    const k = (n) => (n + h / 30) % 12, a = s * Math.min(l, 1 - l);
    const f = (n) => Math.round(255 * (l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)))));
    return '#' + [f(0), f(8), f(4)].map((v) => v.toString(16).padStart(2, '0')).join('');
  }

  function materialVars(accent, dark) {
    const { h, s: s0 } = hexToHsl(accent);
    const s = clamp(s0, 45, 95);
    if (!dark) return {
      '--bg': hsl(h, 48, 93), '--nav': hsl(h, 42, 90), '--surface': '#ffffff', '--surface2': hsl(h, 45, 90),
      '--text': '#141218', '--text2': hsl(h, 32, 36), '--outline': hsl(h, 32, 70),
      '--primary': hsl(h, Math.max(s, 72), 36), '--on-primary': '#ffffff', '--pc': hsl(h, Math.max(s, 68), 84), '--on-pc': hsl(h, 85, 12),
      '--tert': hsl(h + 55, 68, 84),
      '--app-bg': hsl(h, 48, 93), '--win-bg': hslHex(h, 48, 93),
    };
    return {
      '--bg': hsl(h, 24, 9), '--nav': hsl(h, 20, 11), '--surface': hsl(h, 22, 14), '--surface2': hsl(h, 20, 21),
      '--text': hsl(h, 16, 95), '--text2': hsl(h, 14, 72), '--outline': hsl(h, 14, 34),
      '--primary': hsl(h, Math.min(s + 10, 90), 76), '--on-primary': hsl(h, s, 15), '--pc': hsl(h, Math.min(s, 58), 28), '--on-pc': hsl(h, 80, 91),
      '--tert': hsl(h + 55, 38, 27),
      '--app-bg': hsl(h, 24, 9), '--win-bg': hslHex(h, 24, 9),
    };
  }
  function win11Vars(accent, dark) {
    const { h, s: s0, l } = hexToHsl(accent);
    const s = clamp(s0, 30, 100);
    if (!dark) return {
      '--bg': '#f3f3f3', '--nav': 'transparent', '--surface': '#fbfbfb', '--surface2': '#f0f0f0', '--text': '#1a1a1a', '--text2': '#616161', '--outline': '#e3e3e3',
      '--primary': accent, '--on-primary': l > 62 ? '#000' : '#fff', '--pc': hsl(h, s, 92), '--on-pc': '#1a1a1a',
      '--app-bg': '#f3f3f3', '--frame': 'rgba(0,0,0,.28)',
    };
    return {
      '--bg': '#1e1e1e', '--nav': '#181818', '--surface': '#252525', '--surface2': '#2d2d2d', '--text': '#ffffff', '--text2': '#a4a4a4', '--outline': 'rgba(255,255,255,0.08)',
      '--primary': hsl(h, clamp(s, 40, 100), clamp(l < 55 ? 68 : l, 62, 76)), '--on-primary': '#000', '--pc': hsl(h, 40, 24), '--on-pc': '#fff',
      '--app-bg': '#1e1e1e', '--frame': 'rgba(255,255,255,.16)',
    };
  }
  function terminalVars(accent, dark) {
    const { h, s } = hexToHsl(accent || '#33FF66');
    const sat = clamp(s, 40, 100);
    if (dark) return {
      '--bg': hsl(h, 40, 4), '--nav': 'transparent', '--surface': hsl(h, 38, 7), '--surface2': hsl(h, 34, 11),
      '--text': hsl(h, 70, 82), '--text2': hsl(h, 32, 52), '--outline': hsl(h, 38, 21),
      '--primary': hsl(h, sat, 58), '--on-primary': hsl(h, 40, 5), '--pc': hsl(h, 45, 13), '--on-pc': hsl(h, sat, 66),
      '--app-bg': hsl(h, 40, 4), '--frame': hsl(h, sat, 30),
    };
    return {
      '--bg': hsl(h, 24, 92), '--nav': 'transparent', '--surface': hsl(h, 24, 96), '--surface2': hsl(h, 22, 88),
      '--text': hsl(h, 55, 13), '--text2': hsl(h, 24, 34), '--outline': hsl(h, 22, 68),
      '--primary': hsl(h, sat, 27), '--on-primary': '#ffffff', '--pc': hsl(h, 28, 85), '--on-pc': hsl(h, sat, 22),
      '--app-bg': hsl(h, 24, 92), '--frame': hsl(h, 30, 45),
    };
  }

  // native blur behind the window (acrylic); only re-sent when something changed
  let effectKey = null;
  function syncWindowEffect(on, dark) {
    if (!tauri || S.settings.transparency === undefined) return;
    const key = `${on}|${dark}`;
    if (key === effectKey) return;
    effectKey = key;
    invoke('set_transparency', { enabled: on, dark, persist: false }).catch(() => {});
  }

  const mq = window.matchMedia('(prefers-color-scheme: dark)');
  function applyTheme() {
    const { style, mode, accent } = S.settings;
    const dark = mode === 'dark' || (mode === 'system' && mq.matches);
    root.dataset.style = style;
    root.dataset.mode = dark ? 'dark' : 'light';
    THEME_VARS.forEach((k) => root.style.removeProperty(k));
    let v = {};
    if (style === 'material') v = materialVars(accent, dark);
    else if (style === 'win11') v = win11Vars(accent, dark);
    else if (style === 'terminal') v = terminalVars(accent, dark);
    const transp = style === 'win11' && !!S.settings.transparency;
    if (transp) {
      // one translucent level for every surface (cards, buttons, chips, inputs...)
      v['--app-bg'] = `color-mix(in srgb, ${v['--app-bg']} 55%, transparent)`;
      v['--surface'] = `color-mix(in srgb, ${v['--surface']} 55%, transparent)`;
      v['--surface2'] = `color-mix(in srgb, ${v['--surface2']} 55%, transparent)`;
    }
    Object.entries(v).forEach(([k, val]) => root.style.setProperty(k, val));
    $('accentField').hidden = style === 'nothing';
    $('rowTransp').hidden = style !== 'win11';
    $('swTransp').checked = !!S.settings.transparency;
    root.dataset.transp = transp ? '1' : '0';
    syncWindowEffect(transp, dark);
    document.querySelectorAll('#segStyle button').forEach((b) => b.classList.toggle('on', b.dataset.v === style));
    document.querySelectorAll('#segMode button').forEach((b) => b.classList.toggle('on', b.dataset.v === mode));
    document.querySelectorAll('.sw[data-c]').forEach((b) => b.classList.toggle('on', b.dataset.c.toLowerCase() === accent.toLowerCase()));
    try { localStorage.setItem('emz-theme', JSON.stringify({ style, mode, accent })); } catch (_) {}
    renderAllDots();
    refreshStatusColor();
  }
  mq.addEventListener('change', () => S && applyTheme());

  function saveTheme() {
    applyTheme();
    const { style, mode, accent } = S.settings;
    invoke('set_theme', { style, mode, accent }).catch(() => {});
  }

  // ───────── status / power ─────────
  let powerState = 'off';
  function refreshStatusColor() {
    const el = $('statusTitle');
    el.style.color = powerState === 'on' && isDots() ? 'var(--led)' : '';
    if (isDots()) drawDots(el);
  }
  function spawnPowerBurst() {
    const burst = $('powerBurst');
    if (!burst) return;
    burst.innerHTML = '';
    const style = root.dataset.style || 'win11';
    const count = style === 'nothing' ? 18 : style === 'material' ? 14 : 12;

    for (let i = 0; i < count; i++) {
      const el = document.createElement('div');
      el.className = 'burst-shape';
      const angle = (i / count) * Math.PI * 2 + (Math.random() - 0.5) * 0.45;
      const dist = 72 + Math.random() * 80;
      const dx = Math.round(Math.cos(angle) * dist);
      const dy = Math.round(Math.sin(angle) * dist);
      const rot = Math.round((Math.random() - 0.5) * 180);

      el.style.setProperty('--dx', `${dx}px`);
      el.style.setProperty('--dy', `${dy}px`);
      el.style.setProperty('--rot', `${rot}deg`);

      if (style === 'material') {
        const shapes = ['circle', 'pill', 'star'];
        const sType = shapes[i % shapes.length];
        const colors = ['var(--primary)', 'var(--pc)', 'color-mix(in srgb,var(--primary) 75%,#fff)', '#a8c7fa', '#c4eed0'];
        const col = colors[i % colors.length];

        if (sType === 'circle') {
          const sz = 8 + Math.random() * 8;
          el.style.width = `${sz}px`;
          el.style.height = `${sz}px`;
          el.style.borderRadius = '50%';
          el.style.background = col;
          el.style.boxShadow = `0 3px 10px ${col}`;
        } else if (sType === 'pill') {
          const w = 15 + Math.random() * 9;
          const h = 7 + Math.random() * 3;
          el.style.width = `${w}px`;
          el.style.height = `${h}px`;
          el.style.borderRadius = '99px';
          el.style.background = col;
        } else {
          el.innerHTML = `<svg viewBox="0 0 24 24" width="16" height="16" fill="${col}"><path d="M12 2c1.2 5.5 4.5 8.8 10 10-5.5 1.2-8.8 4.5-10 10-1.2-5.5-4.5-8.8-10-10 5.5-1.2 8.8-4.5 10-10z"/></svg>`;
        }
        el.style.setProperty('--end-scale', (0.8 + Math.random() * 0.4).toFixed(2));
      } else if (style === 'nothing') {
        const isRed = Math.random() > 0.35;
        const col = isRed ? 'var(--led)' : 'var(--text)';
        const isSquare = i % 2 === 0;

        if (isSquare) {
          const sz = 6 + (i % 3) * 2;
          el.style.width = `${sz}px`;
          el.style.height = `${sz}px`;
          el.style.background = col;
          el.style.borderRadius = '0';
          if (isRed) el.style.boxShadow = '0 0 8px var(--led)';
        } else {
          const sz = 6 + (i % 2) * 2;
          el.style.width = `${sz}px`;
          el.style.height = `${sz}px`;
          el.style.borderRadius = '50%';
          el.style.background = col;
          if (isRed) el.style.boxShadow = '0 0 10px var(--led)';
        }
        el.style.setProperty('--rot', '0deg');
        el.style.setProperty('--end-scale', '1');
      } else if (style === 'win11') {
        const sz = 7 + Math.random() * 7;
        el.style.width = `${sz}px`;
        el.style.height = `${sz}px`;
        el.style.borderRadius = i % 2 === 0 ? '50%' : '3px';
        el.style.background = 'var(--primary)';
        el.style.boxShadow = '0 0 12px color-mix(in srgb,var(--primary) 70%,transparent)';
        el.style.setProperty('--end-scale', '0.5');
      } else {
        const sz = 9 + Math.random() * 7;
        el.style.width = `${sz}px`;
        el.style.height = `${sz}px`;
        el.style.borderRadius = '50%';
        el.style.background = 'color-mix(in srgb,var(--primary) 40%,var(--surface))';
        el.style.border = '1.5px solid var(--outline)';
        el.style.boxShadow = '0 4px 12px rgba(0,0,0,0.15)';
      }

      burst.appendChild(el);
    }

    setTimeout(() => {
      burst.innerHTML = '';
    }, 850);
  }

  function setPower(state) {
    const prev = powerState;
    powerState = state;
    $('powerWrap').dataset.state = state;
    const text = state === 'on' ? 'Включено' : state === 'busy' ? 'Запуск…' : state === 'error' ? 'Ошибка' : 'Выключено';
    setDT($('statusTitle'), text);
    $('navStatus').textContent = text;
    $('navLed').classList.toggle('on', state === 'on');
    refreshStatusColor();
    if (state === 'on' && prev !== 'on') {
      spawnPowerBurst();
    }
  }

  // ───────── modes list ─────────
  const resOf = (file) => (S.settings.alt_results || {})[file];

  function statusOf(r) {
    if (!r) return 'none';
    // the backend judges the mode (good = every site opened, mid = some, bad = none); ms is only a speed value
    if (r.ms === 0) return r.status === 'bad' ? 'bad' : 'none'; // 0 ms = nothing was measured
    if (r.status) return r.status;
    if (r.ms > 0) return r.ms > 140 ? 'bad' : r.ms > 60 ? 'mid' : 'good';
    return 'none';
  }

  function ranked() {
    const idx = new Map(S.strategies.map((s, i) => [s.file, i]));
    const isGood = (s) => {
      const r = resOf(s.file);
      const st = statusOf(r);
      return st === 'good' || st === 'mid';
    };
    const rec = S.strategies.filter(isGood).sort((a, b) => {
      const ra = resOf(a.file), rb = resOf(b.file);
      const sa = statusOf(ra), sb = statusOf(rb);
      if (sa !== sb) return sa === 'good' ? -1 : 1;
      const ma = (ra && ra.ms != null) ? ra.ms : 999;
      const mb = (rb && rb.ms != null) ? rb.ms : 999;
      return ma - mb;
    });
    const rest = S.strategies.filter((s) => !isGood(s)).sort((a, b) => {
      const ba = resOf(a.file) ? 1 : 0, bb = resOf(b.file) ? 1 : 0;
      return ba - bb || idx.get(a.file) - idx.get(b.file);
    });
    return { rec, rest };
  }

  function dotClass(file) {
    if (testingFile === file) return 'testing';
    const r = resOf(file);
    return statusOf(r);
  }

  function pingText(file) {
    const r = resOf(file);
    return r && r.ms > 0 && r.status !== 'bad' ? `${r.ms} мс` : '';
  }

  function modeRow(st, isRec = false) {
    const b = document.createElement('button');
    b.className = 'mode-row' + (st.file === S.settings.strategy ? ' on' : '');
    b.innerHTML = '<span class="sdot"></span><span class="mname"></span><span class="mping"></span><span class="radio"></span>';
    b.children[0].className = 'sdot ' + dotClass(st.file);
    
    const nameEl = b.children[1];
    nameEl.textContent = st.name;
    if (isRec) {
      const star = document.createElement('span');
      star.className = 'rec-star';
      star.title = 'Рекомендованный режим';
      star.innerHTML = '<svg viewBox="0 0 24 24"><path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/></svg>';
      nameEl.appendChild(star);
    }

    const pingEl = b.children[2];
    const pt = pingText(st.file);
    pingEl.textContent = pt;
    const stCls = dotClass(st.file);
    if (stCls && stCls !== 'none') pingEl.classList.add(stCls);

    b.onclick = () => changeStrategy(st.file);
    return b;
  }

  let altsOpen = false;
  function updateAltsVisibility() {
    const grid = $('homeGrid');
    if (grid) grid.classList.toggle('alts-collapsed', !altsOpen);
    const btn = $('btnToggleAlts');
    if (btn) btn.setAttribute('aria-expanded', String(altsOpen));
    if ($('altsToggleLabel')) {
      $('altsToggleLabel').textContent = altsOpen ? '' : 'Режимы обхода';
    }
  }

  function renderModes() {
    const box = $('modeList');
    const keep = box.scrollTop;
    box.innerHTML = '';
    const { rec, rest } = ranked();
    const label = (t) => { const d = document.createElement('div'); d.className = 'sec-label'; d.textContent = t; box.appendChild(d); };
    if (rec.length) {
      label('Рекомендуемые');
      const top3Files = new Set(rec.slice(0, 3).map((s) => s.file));
      rec.forEach((s) => box.appendChild(modeRow(s, top3Files.has(s.file))));
      if (rest.length) label('Остальные');
      rest.forEach((s) => box.appendChild(modeRow(s, false)));
    } else {
      rest.forEach((s) => box.appendChild(modeRow(s, false)));
    }
    box.scrollTop = keep;
    const cur = S.strategies.find((x) => x.file === S.settings.strategy) || S.strategies[0];
    const curName = cur ? cur.name : 'ALT 1';
    $('curName').textContent = curName;
    if ($('altBadge')) $('altBadge').textContent = curName;
    $('curDot').className = 'sdot ' + (cur ? dotClass(cur.file) : 'good');
    $('curPing').textContent = cur ? pingText(cur.file) : '';
    $('btnTest').textContent = cancelling ? 'Остановка…' : (S.testing ? 'Стоп' : 'Автоподбор');
    if ($('btnTestAll')) $('btnTestAll').hidden = !!S.testing;
    $('btnTest').classList.toggle('primary', !S.testing);
    updateAltsVisibility();
  }

  // ───────── brand icons ─────────
  // one outline style for all service icons (24x24, 1.9 stroke)
  const ic = (body) => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round">${body}</svg>`;
  const SVC_ICONS = {
    youtube: ic('<rect x="2.5" y="5.5" width="19" height="13" rx="4"/><path d="M10 9.4v5.2l4.6-2.6z"/>'),
    discord: ic('<path d="M5 4.5h14A2.5 2.5 0 0 1 21.5 7v8.5A2.5 2.5 0 0 1 19 18h-6l-4 3v-3H5a2.5 2.5 0 0 1-2.5-2.5V7A2.5 2.5 0 0 1 5 4.5z"/><path d="M8.6 11h.01M15.4 11h.01"/>'),
    twitch: ic('<rect x="3" y="4" width="18" height="12.5" rx="2.5"/><path d="M8 20.5h8M12 16.5v4M10.5 8.5v4M14 8.5v4"/>'),
    steam: ic('<path d="M6.5 8h11a4.5 4.5 0 0 1 4.4 3.6l.6 3.4a2.6 2.6 0 0 1-4.5 2.1L16 15.5H8L6 17.1a2.6 2.6 0 0 1-4.5-2.1l.6-3.4A4.5 4.5 0 0 1 6.5 8z"/><path d="M8 10.5v3M6.5 12h3M16 11.3h.01M18 13.1h.01"/>'),
    hltv: ic('<circle cx="12" cy="12" r="7.5"/><circle cx="12" cy="12" r="2.8"/><path d="M12 1.8v4M12 18.2v4M1.8 12h4M18.2 12h4"/>'),
    modrinth: ic('<path d="M12 2.8l8 4.2v10l-8 4.2-8-4.2V7z"/><path d="M4.2 7.2L12 11.4l7.8-4.2M12 11.4v9.6"/>'),
    roblox: ic('<rect x="5" y="5" width="14" height="14" rx="2.6" transform="rotate(12 12 12)"/><rect x="10.2" y="10.2" width="3.6" height="3.6" rx=".6" transform="rotate(12 12 12)"/>'),
    epic: ic('<path d="M12 2.8l7.5 2.7v6c0 4.6-3.2 8.3-7.5 9.7-4.3-1.4-7.5-5.1-7.5-9.7v-6z"/><path d="M8.8 12l2.3 2.3 4.2-4.4"/>'),
    x: ic('<path d="M5 5l14 14M19 5L5 19"/>')
  };
  const DEFAULT_ICON = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>';

  // ───────── other pages ─────────
  const openDnsActive = () => !!(S && S.tweaks_applied) && antiInput.dns === 'opendns';
  async function toggleOpenDns() {
    try {
      if (openDnsActive()) {
        await invoke('tweaks_reset');
        S.tweaks_applied = false; antiInput.dns = 'dhcp'; saveAntiInput();
        toast('Прежние настройки сети возвращены');
      } else {
        const on = !!antiInput.enabled;
        antiInput.dns = 'opendns'; saveAntiInput();
        await invoke('tweaks_apply', { cfg: { tcp: on && !!antiInput.tcpNoDelay, throttle: on && !!antiInput.netThrottle, tcpset: on && !!antiInput.tcpSet, power: on && !!antiInput.power, nicpower: on && !!antiInput.nicPower, gamedvr: on && !!antiInput.gameDvr, dopt: on && !!antiInput.dopt, mtu: on ? antiInput.mtu : 'default', dns: 'opendns' } });
        S.tweaks_applied = true;
        toast('OpenDNS включён. Полностью закройте Roblox и откройте заново');
      }
    } catch (e) { toast(String(e)); }
  }

  function renderServices() {
    const mk = (box, group) => {
      box.innerHTML = '';
      S.catalog.filter((c) => c.group === group).forEach((c) => {
        const on = c.id in S.settings.services ? S.settings.services[c.id] : (c.id === 'youtube' || c.id === 'discord');
        const l = document.createElement('label');
        l.className = 'svc';
        // Clean layout: icon badge + bold title + toggle switch (no subtitle hint)
        l.innerHTML = '<span class="badge"></span><span class="sw-text"><b></b></span><input type="checkbox"><span class="switch"></span>';
        l.children[0].innerHTML = SVC_ICONS[c.id] || DEFAULT_ICON;
        l.querySelector('b').textContent = c.name;
        const inp = l.querySelector('input');
        inp.checked = on;
        inp.onchange = async () => {
          S.settings.services[c.id] = inp.checked;
          try { await invoke('set_service', { id: c.id, enabled: inp.checked }); scheduleRestart(); }
          catch (e) { inp.checked = !inp.checked; toast(String(e)); }
        };
        if (c.id === 'roblox') {
          const link = document.createElement('a');
          link.className = 'svc-link'; link.href = '#';
          l.querySelector('.sw-text').appendChild(link);
          const paint = () => {
            link.hidden = !inp.checked;
            link.textContent = openDnsActive() ? 'OpenDNS включён · вернуть как было' : 'Не грузятся картинки? Включить OpenDNS';
          };
          paint();
          inp.addEventListener('change', paint);
          link.onclick = async (e) => { e.preventDefault(); e.stopPropagation(); await toggleOpenDns(); paint(); };
        }
        box.appendChild(l);
      });
    };
    mk($('listMain'), 'main');
    mk($('listExp'), 'exp');
    const chips = $('domChips');
    chips.innerHTML = '';
    (S.settings.custom_domains || []).forEach((d) => {
      const c = document.createElement('span');
      c.className = 'chip';
      c.innerHTML = '<span></span><button aria-label="Удалить"><svg viewBox="0 0 24 24"><path d="m6 6 12 12M18 6 6 18" stroke-width="2.4" stroke-linecap="round"/></svg></button>';
      c.firstChild.textContent = d;
      c.querySelector('button').onclick = async () => {
        S.settings.custom_domains = await invoke('remove_domain', { domain: d }).catch(() => S.settings.custom_domains);
        renderServices(); scheduleRestart();
      };
      chips.appendChild(c);
    });
  }

  const GAME_NOTES = {
    off: '<b>Выкл (рекомендуется)</b>: игровые порты не трогаются, поэтому пинг и нагрузка минимальные. Подходит для большинства игр.',
    all: '<b>TCP + UDP</b>: zapret дополнительно обрабатывает все порты 1024–65535. Включайте, только если игра, лаунчер или голосовой чат не работают без обхода. Может повышать пинг.',
    tcp: '<b>TCP</b>: дополнительно обрабатываются TCP-порты 1024–65535 (лаунчеры, загрузки). Может немного повышать пинг.',
    udp: '<b>UDP</b>: дополнительно обрабатываются UDP-порты 1024–65535 (голосовые чаты, часть игр). Может повышать пинг.'
  };
  const IPSET_NOTES = {
    none: 'Выкл: обход только по спискам доменов. Минимальная нагрузка.',
    loaded: 'Список: обход для подсетей из загруженного списка. Умеренная нагрузка.',
    any: 'Все: обход для любых IP-адресов. Самая высокая нагрузка и пинг, включайте только если ничего другого не помогает.'
  };

  function renderZapret() {
    document.querySelectorAll('#segGame button').forEach((b) => b.classList.toggle('on', b.dataset.v === S.game_mode));
    document.querySelectorAll('#segIpset button').forEach((b) => b.classList.toggle('on', b.dataset.v === S.ipset));
    $('dirPath').textContent = S.settings.zapret_dir;
    $('verText').textContent = S.version || '—';
    if ($('gameNote')) $('gameNote').innerHTML = GAME_NOTES[S.game_mode] || GAME_NOTES.off;
    if ($('ipsetNote')) $('ipsetNote').textContent = IPSET_NOTES[S.ipset] || IPSET_NOTES.none;
  }

  function renderTg() {
    const on = !!S.settings.tg_proxy;
    $('swTg').checked = on;
    $('swTg').disabled = tgBusy;
    $('swTgAuto').checked = !!S.settings.tg_autostart;
    $('rowTgAuto').hidden = !on;
    $('tgCard').hidden = !(on && S.tg.installed);
    $('tgDot').className = 'sdot ' + (S.tg.running ? 'good' : 'bad');
    $('tgAddr').textContent = (() => {
      const m = /server=([^&]+)&port=(\d+)/.exec(S.tg.link || '');
      return m ? `${m[1]}:${m[2]}` : '127.0.0.1:1443';
    })();
    $('tgSub').textContent = tgBusy ? 'Подождите…' : (on && !S.tg.running ? 'Запускается…' : (S.tg.version ? `tg-ws-proxy ${S.tg.version}` : ''));
  }

  function renderSwatches() {
    const box = $('swatches');
    box.innerHTML = '';
    SWATCHES.forEach((c) => {
      const b = document.createElement('button');
      b.className = 'sw'; b.dataset.c = c; b.style.background = c; b.setAttribute('aria-label', c);
      b.onclick = () => { S.settings.accent = c; saveTheme(); };
      box.appendChild(b);
    });
    const custom = document.createElement('div');
    custom.className = 'sw custom';
    custom.innerHTML = '<input type="color" aria-label="Свой цвет">';
    custom.firstChild.value = S.settings.accent.length === 7 ? S.settings.accent : '#6750A4';
    custom.firstChild.oninput = (e) => { S.settings.accent = e.target.value; saveTheme(); };
    box.appendChild(custom);
  }

  // ───────── Optimization (network tweaks) ─────────
  const DEFAULT_ANTI_INPUT = {
    enabled: false,
    tcpNoDelay: true,
    netThrottle: true,
    tcpSet: true,
    power: true,
    nicPower: true,
    gameDvr: true,
    dopt: true,
    mtu: '1500',
    dns: 'cloudflare'
  };

  let antiInput = (() => {
    try {
      const saved = localStorage.getItem('emz-anti-input');
      return saved ? { ...DEFAULT_ANTI_INPUT, ...JSON.parse(saved) } : { ...DEFAULT_ANTI_INPUT };
    } catch (_) {
      return { ...DEFAULT_ANTI_INPUT };
    }
  })();

  function saveAntiInput() {
    try { localStorage.setItem('emz-anti-input', JSON.stringify(antiInput)); } catch (_) {}
    renderAntiInput();
  }

  const EXTRA_TWEAKS = [['swTcpSet', 'tcpSet'], ['swPower', 'power'], ['swNicPower', 'nicPower'], ['swGameDvr', 'gameDvr'], ['swDopt', 'dopt']];
  function renderAntiInput() {
    EXTRA_TWEAKS.forEach(([id, key]) => { if ($(id)) $(id).checked = !!antiInput[key]; });
    const tagA = $('tagApplied');
    if (tagA) {
      const on = !!(S && S.tweaks_applied);
      tagA.className = 'tweak-tag ' + (on ? 'good' : 'mid');
      tagA.textContent = on ? 'Применено в Windows' : 'Не применено';
    }
    const swMaster = $('swOptMaster');
    if (swMaster) swMaster.checked = !!antiInput.enabled;
    const details = $('antiInputDetails');
    if (details) details.hidden = !antiInput.enabled;

    const swTcp = $('swTcpNoDelay');
    if (swTcp) swTcp.checked = !!antiInput.tcpNoDelay;
    const tagTcp = $('tagTcpStatus');
    if (tagTcp) {
      tagTcp.className = 'tweak-tag ' + (antiInput.tcpNoDelay ? 'good' : 'mid');
      tagTcp.textContent = antiInput.tcpNoDelay 
        ? 'Включено' 
        : 'Стандартный стек Windows (буферизация ACK до 200 мс)';
    }

    const swNet = $('swNetThrottle');
    if (swNet) swNet.checked = !!antiInput.netThrottle;
    const tagNet = $('tagThrottleStatus');
    if (tagNet) {
      tagNet.className = 'tweak-tag ' + (antiInput.netThrottle ? 'good' : 'mid');
      tagNet.textContent = antiInput.netThrottle 
        ? 'Троттлинг отключен (0xFFFFFFFF · SystemResponsiveness = 0)' 
        : 'Стандартное ограничение медиа/сети Windows';
    }

    document.querySelectorAll('#segMtu button').forEach((b) => {
      b.classList.toggle('on', b.dataset.v === antiInput.mtu);
    });
    const mtuNote = $('mtuNote');
    if (mtuNote) {
      if (antiInput.mtu === '1500') mtuNote.textContent = '1500 байт — оптимально для большинства Ethernet/Wi-Fi соединений без потерь пакетов.';
      else if (antiInput.mtu === '1492') mtuNote.textContent = '1492 байт — оптимизировано для PPPoE / DSL соединений (предотвращает фрагментацию).';
      else mtuNote.textContent = 'Стандартное значение MTU операционной системы Windows.';
    }

    document.querySelectorAll('#segDns button').forEach((b) => {
      b.classList.toggle('on', b.dataset.v === antiInput.dns);
    });
    const dnsNote = $('dnsNote');
    if (dnsNote) {
      if (antiInput.dns === 'cloudflare') dnsNote.textContent = 'Cloudflare DNS (1.1.1.1 / 1.0.0.1) — минимальный пинг и защита от перехвата запросов.';
      else if (antiInput.dns === 'google') dnsNote.textContent = 'Google Public DNS (8.8.8.8 / 8.8.4.4) — высокая глобальная стабильность и скорость.';
      else if (antiInput.dns === 'opendns') dnsNote.textContent = 'OpenDNS (208.67.222.222 / 208.67.220.220) — часто помогает, когда не грузятся картинки Roblox.';
      else if (antiInput.dns === 'quad9') dnsNote.textContent = 'Quad9 DNS (9.9.9.9 / 149.112.112.112) — высокая скорость со встроенным фильтром.';
      else dnsNote.textContent = 'Стандартный DNS вашего интернет-провайдера (автоматически по DHCP).';
    }
  }

  function generateTweaksScript() {
    let script = `@echo off\r\nchcp 65001 >nul\r\ntitle EmzLink - Оптимизация сети и Анти-инпут\r\necho ========================================================\r\necho    EmzLink: Применение игровых твиков сети Windows\r\necho ========================================================\r\necho.\r\n\r\nnet session >nul 2>&1\r\nif %errorlevel% neq 0 (\r\n    echo [ОШИБКА] Пожалуйста, запустите этот файл от имени Администратора!\r\n    pause\r\n    exit /b 1\r\n)\r\n\r\n`;

    if (antiInput.tcpNoDelay) {
      script += `echo [*] Применение твиков TCP NoDelay и AckFrequency...\r\nfor /f "tokens=*" %%i in ('reg query "HKLM\\SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters\\Interfaces"') do (\r\n    reg add "%%i" /v TcpAckFrequency /t REG_DWORD /d 1 /f >nul 2>&1\r\n    reg add "%%i" /v TCPNoDelay /t REG_DWORD /d 1 /f >nul 2>&1\r\n    reg add "%%i" /v TcpDelAckTicks /t REG_DWORD /d 0 /f >nul 2>&1\r\n)\r\n\r\n`;
    }

    if (antiInput.netThrottle) {
      script += `echo [*] Отключение Network Throttling Index Windows...\r\nreg add "HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile" /v NetworkThrottlingIndex /t REG_DWORD /d 4294967295 /f >nul 2>&1\r\nreg add "HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile" /v SystemResponsiveness /t REG_DWORD /d 0 /f >nul 2>&1\r\n\r\n`;
    }

    if (antiInput.mtu !== 'default') {
      const val = antiInput.mtu === '1492' ? '1492' : '1500';
      script += `echo [*] Настройка MTU ${val}...\r\nfor /f "skip=3 tokens=4,5*" %%a in ('netsh interface show interface') do (\r\n    netsh interface ipv4 set subinterface "%%c" mtu=${val} store=persistent >nul 2>&1\r\n)\r\n\r\n`;
    }

    if (antiInput.dns === 'cloudflare') {
      script += `echo [*] Установка Anycast DNS (Cloudflare 1.1.1.1)...\r\nfor /f "skip=3 tokens=4,5*" %%a in ('netsh interface show interface') do (\r\n    netsh interface ip set dns name="%%c" static 1.1.1.1 primary >nul 2>&1\r\n    netsh interface ip add dns name="%%c" 1.0.0.1 index=2 >nul 2>&1\r\n)\r\n\r\n`;
    } else if (antiInput.dns === 'google') {
      script += `echo [*] Установка Anycast DNS (Google 8.8.8.8)...\r\nfor /f "skip=3 tokens=4,5*" %%a in ('netsh interface show interface') do (\r\n    netsh interface ip set dns name="%%c" static 8.8.8.8 primary >nul 2>&1\r\n    netsh interface ip add dns name="%%c" 8.8.4.4 index=2 >nul 2>&1\r\n)\r\n\r\n`;
    } else if (antiInput.dns === 'quad9') {
      script += `echo [*] Установка Anycast DNS (Quad9 9.9.9.9)...\r\nfor /f "skip=3 tokens=4,5*" %%a in ('netsh interface show interface') do (\r\n    netsh interface ip set dns name="%%c" static 9.9.9.9 primary >nul 2>&1\r\n    netsh interface ip add dns name="%%c" 149.112.112.112 index=2 >nul 2>&1\r\n)\r\n\r\n`;
    } else if (antiInput.dns === 'dhcp') {
      script += `echo [*] Сброс DNS к провайдеру (DHCP)...\r\nfor /f "skip=3 tokens=4,5*" %%a in ('netsh interface show interface') do (\r\n    netsh interface ip set dns name="%%c" dhcp >nul 2>&1\r\n)\r\n\r\n`;
    }

    script += `ipconfig /flushdns >nul 2>&1\r\necho.\r\necho [УСПЕХ] Все выбранные твики сети успешно применены!\r\npause\r\n`;
    return script;
  }

  function downloadTweaksBat() {
    const text = generateTweaksScript();
    const blob = new Blob([text], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'EmzLink_AntiInput_Optimize.bat';
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    toast('Файл EmzLink_AntiInput_Optimize.bat скачан');
  }

  async function resetTweaks() {
    const btn = $('btnResetTweaks');
    if (btn) btn.disabled = true;
    try {
      if (S && S.tweaks_applied) { await invoke('tweaks_reset'); S.tweaks_applied = false; }
      antiInput = { ...DEFAULT_ANTI_INPUT, enabled: false, mtu: 'default', dns: 'dhcp' };
      saveAntiInput();
      toast('Настройки сети возвращены как были');
    } catch (e) { toast(String(e)); }
    if (btn) btn.disabled = false;
  }

  async function applyTweaks() {
    const btn = $('btnApplyTweaks');
    btn.disabled = true; const old = btn.textContent; btn.textContent = 'Применяем…';
    try {
      const r = await invoke('tweaks_apply', { cfg: { tcp: !!antiInput.tcpNoDelay, throttle: !!antiInput.netThrottle, tcpset: !!antiInput.tcpSet, power: !!antiInput.power, nicpower: !!antiInput.nicPower, gamedvr: !!antiInput.gameDvr, dopt: !!antiInput.dopt, mtu: antiInput.mtu, dns: antiInput.dns } });
      S.tweaks_applied = true;
      renderAntiInput();
      toast(r && r.includes('предупрежд') ? 'Применено, но часть параметров не удалось изменить' : 'Применено. Некоторые твики вступят в силу после перезагрузки');
    } catch (e) { toast(String(e)); }
    btn.disabled = false; btn.textContent = old;
  }

  function render() {
    const ok = S.installed && S.strategies.length > 0;
    $('missingCard').hidden = ok;
    if (!ok && $('missingText')) $('missingText').textContent = (S.problem ? S.problem + '. ' : '') + 'Искали в: ' + S.settings.zapret_dir;
    $('btnPower').disabled = !ok || S.testing;
    const cur = S.strategies.find((x) => x.file === S.settings.strategy) || S.strategies[0];
    if (cur && cur.file !== S.settings.strategy) S.settings.strategy = cur.file;
    $('swBypass').checked = !!S.settings.bypass_autostart;
    $('swApp').checked = !!S.settings.app_autostart;
    if ($('swNotify')) $('swNotify').checked = S.settings.notify !== false;
    if ($('swHeal')) $('swHeal').checked = !!S.settings.auto_heal;
    $('prog').hidden = !S.testing;
    if (!busy) setPower(ok && S.running ? 'on' : 'off');
    renderModes(); renderServices(); renderZapret(); renderTg(); renderAntiInput(); renderBeta();
  }

  function renderBeta() {
    const on = !!S.settings.experimental;
    $('swExperimental').checked = on;
    $('navOpt').hidden = !on;
    if (!on && document.getElementById('v-anti-input').classList.contains('on')) go('settings');
  }

  // ───────── actions ─────────
  async function togglePower() {
    if (busy || S.testing) return;
    busy = true;
    const turningOn = !S.running;
    setPower('busy');
    try {
      if (turningOn) await invoke('start_bypass', { file: S.settings.strategy });
      else await invoke('stop_bypass');
      S.running = await invoke('get_running');
      setPower(S.running ? 'on' : 'off');
    } catch (e) {
      S.running = await invoke('get_running').catch(() => false);
      setPower(S.running ? 'on' : 'error');
      toast(String(e));
      if (!S.running) setTimeout(() => !busy && setPower('off'), 2600);
    } finally { busy = false; }
  }

  function scheduleRestart() {
    if (!S.running) return;
    clearTimeout(restartTimer);
    setPower('busy');
    restartTimer = setTimeout(async () => {
      busy = true;
      try { await invoke('start_bypass', { file: S.settings.strategy }); S.running = true; }
      catch (e) { S.running = await invoke('get_running').catch(() => false); toast(String(e)); }
      busy = false; setPower(S.running ? 'on' : 'off');
    }, 1000);
  }

  function changeStrategy(file) {
    if (file === S.settings.strategy) return;
    S.settings.strategy = file;
    renderModes();
    invoke('select_strategy', { file }).catch((e) => toast(String(e)));
    scheduleRestart();
  }

  let cancelling = false;
  async function runTest(quick = true) {
    if (S.testing) {
      if (tauri) {
        // the backend finishes the current mode and restores the previous state; wait for test-done
        invoke('cancel_test').catch(() => {});
        cancelling = true;
        $('btnTest').disabled = true;
        $('btnTest').textContent = 'Остановка…';
        return;
      }
      S.testing = false;
      $('btnTest').disabled = false;
      $('prog').hidden = true;
      $('progText').textContent = '';
      render();
      return;
    }

    S.testing = true;
    S.wasRunning = S.running;
    $('prog').hidden = false;
    $('progBar').style.width = '0%';
    $('progText').textContent = '0/' + S.strategies.length;
    render();

    if (!tauri) {
      // Interactive browser test simulation
      const total = S.strategies.length;
      for (let i = 0; i < total; i++) {
        if (!S.testing) break;
        const st = S.strategies[i];
        testingFile = st.file;
        $('progText').textContent = `${i + 1}/${total}`;
        $('progBar').style.width = ((i + 1) / total) * 100 + '%';
        renderModes();
        await new Promise((r) => setTimeout(r, 140));
        let ms = 24 + Math.floor(Math.random() * 50);
        if (st.name === 'ALT 1') ms = 18 + Math.floor(Math.random() * 5); // optimal
        else if (st.name === 'ALT 8') ms = 25 + Math.floor(Math.random() * 6);
        else if (st.name.includes('ALT 9') || st.name.includes('ALT 10')) ms = 145 + Math.floor(Math.random() * 60);
        const status = ms < 60 ? 'good' : ms <= 140 ? 'mid' : 'bad';
        S.settings.alt_results[st.file] = { status, ms };
        testingFile = null;
        renderModes();
      }
      S.testing = false;
      $('prog').hidden = true;
      $('progText').textContent = '';
      const best = bestMode();
      render();
      if (best) {
        const ping = (S.settings.alt_results[best.file] || {}).ms || 20;
        toast(`Лучший режим подобран: ${best.name} (${ping} мс)`);
        changeStrategy(best.file);
      } else {
        toast('Тестирование завершено');
      }
      return;
    }

    try {
      await invoke('test_strategies', { quick });
      if (S.running) { S.running = false; setPower('off'); }
      render();
    } catch (e) {
      S.testing = false;
      $('prog').hidden = true;
      toast(String(e));
    }
  }

  function bestMode() {
    const { rec } = ranked();
    return rec.length ? rec[0] : null;
  }

  async function resetToDefaults() {
    ['emz-theme', 'emz-anti-input', 'emz-first-run-done'].forEach((k) => { try { localStorage.removeItem(k); } catch (_) {} });
    antiInput = { ...DEFAULT_ANTI_INPUT, enabled: false };
    let fresh = null;
    if (tauri) {
      try { fresh = await invoke('reset_all'); }
      catch (e) { toast(String(e)); return; }
    }
    if (fresh && fresh.settings) {
      S = fresh;
    } else {
      S.settings.strategy = 'general (ALT).bat';
      S.settings.bypass_autostart = false;
      S.settings.app_autostart = false;
      S.settings.services = { youtube: true, discord: true };
      S.settings.experimental = false;
      S.settings.transparency = true;
      S.settings.custom_domains = [];
      S.settings.tg_proxy = false;
      S.settings.tg_autostart = false;
      S.game_mode = 'off';
      S.ipset = 'none';
      S.tweaks_applied = false;
    }
    S.settings.style = 'win11';
    S.settings.mode = 'system';
    S.settings.accent = '#0067C0';
    altsOpen = false;
    updateAltsVisibility();
    saveAntiInput();
    try { await invoke('set_theme', { style: 'win11', mode: 'system', accent: '#0067C0' }); } catch (_) {}
    applyTheme();
    renderSwatches();
    render();
    renderAntiInput();
    toast('Все настройки сброшены до заводских');
  }

  async function pickDir() {
    const r = await invoke('choose_dir').catch((e) => { toast(String(e)); return null; });
    if (r) { S = r; applyTheme(); render(); }
  }
  async function install(btn) {
    btn.disabled = true;
    const old = btn.textContent;
    btn.textContent = 'Загрузка…';
    try {
      S = await invoke('update_zapret');
      applyTheme(); render();
      toast('Готово: zapret ' + (S.version || ''));
    } catch (e) { toast(String(e)); }
    btn.textContent = old; btn.disabled = false;
  }
  let pendingUpdate = false;
  async function checkUpdates() {
    const btn = $('btnCheck'), note = $('updNote');
    if (pendingUpdate) { await install(btn); note.textContent = 'Версия ' + S.version; pendingUpdate = false; btn.textContent = 'Проверить обновления'; return; }
    btn.disabled = true; note.textContent = 'Проверяем…';
    try {
      const r = await invoke('check_updates');
      if (r.has_update) { pendingUpdate = true; note.textContent = `Доступна ${r.remote} (у вас ${r.local || '—'})`; btn.textContent = 'Обновить'; }
      else note.textContent = 'Последняя версия';
    } catch (e) { note.textContent = String(e); }
    btn.disabled = false;
  }
  async function toggleAutostart(cmd, el, key) {
    const want = el.checked;
    try { await invoke(cmd, { enabled: want }); S.settings[key] = want; }
    catch (e) { el.checked = !want; toast(String(e)); }
  }

  async function addDomain() {
    const v = $('domIn').value.trim();
    if (!v) return;
    try {
      S.settings.custom_domains = await invoke('add_domain', { domain: v });
      $('domIn').value = '';
      renderServices(); scheduleRestart();
    } catch (e) { toast(String(e)); }
  }

  async function seg(cmd, arg, btn, field) {
    try {
      const r = await invoke(cmd, { mode: btn.dataset.v });
      S[field] = typeof r === 'string' ? r : btn.dataset.v;
      renderZapret(); scheduleRestart();
    } catch (e) { toast(String(e)); }
  }

  function copy(text) {
    if (navigator.clipboard) return navigator.clipboard.writeText(text);
    const t = document.createElement('textarea'); t.value = text; document.body.appendChild(t); t.select(); document.execCommand('copy'); t.remove();
  }

  function go(view) {
    document.querySelectorAll('.nav-item').forEach((b) => b.classList.toggle('on', b.dataset.view === view));
    document.querySelectorAll('.view').forEach((v) => v.classList.toggle('on', v.id === 'v-' + view));
    if (view === 'tg') invoke('tg_state').then((t) => { S.tg = t; renderTg(); }).catch(() => {});
  }

  // ───────── wiring ─────────
  function wire() {
    document.querySelectorAll('.nav-item').forEach((b) => (b.onclick = () => go(b.dataset.view)));
    $('btnMax').onclick = async () => {
      try { const m = await invoke('window_toggle_max'); root.dataset.max = m ? '1' : '0'; } catch (_) {}
    };
    $('btnPower').onclick = togglePower;
    $('btnTest').onclick = () => runTest(true);
    if ($('btnTestAll')) $('btnTestAll').onclick = () => runTest(false);
    $('btnToggleAlts').onclick = () => { altsOpen = !altsOpen; updateAltsVisibility(); };
    $('curChip').onclick = () => { altsOpen = !altsOpen; updateAltsVisibility(); };
    $('btnCloseAlts').onclick = () => { altsOpen = false; updateAltsVisibility(); };
    if ($('btnResetDefaults')) $('btnResetDefaults').onclick = resetToDefaults;
    document.addEventListener('click', (e) => { const l = e.target.closest && e.target.closest('a[data-url]'); if (l) { e.preventDefault(); invoke('open_url', { url: l.dataset.url }).catch(() => {}); } });
    if ($('btnScanConf')) $('btnScanConf').onclick = () => openConflictModal();
    if ($('btnRunWizard')) $('btnRunWizard').onclick = openFirstRunModal;
    if ($('btnOnboardStart')) $('btnOnboardStart').onclick = startOnboardTesting;
    if ($('btnOnboardSkip')) $('btnOnboardSkip').onclick = closeFirstRunModal;
    if ($('btnOnboardCancel')) $('btnOnboardCancel').onclick = () => { if (tauri && onboardTesting) invoke('cancel_test').catch(() => {}); closeFirstRunModal(); };
    if ($('btnOnboardFinish')) $('btnOnboardFinish').onclick = closeFirstRunModal;
    if ($('btnMinOnboard')) $('btnMinOnboard').onclick = () => invoke('window_minimize');
    if ($('btnCloseOnboard')) $('btnCloseOnboard').onclick = closeFirstRunModal;
    $('btnMin').onclick = () => invoke('window_minimize');
    $('btnClose').onclick = () => invoke('window_hide');
    $('btnQuit').onclick = () => invoke('quit_app');
    $('btnChoose1').onclick = pickDir;
    $('btnChoose2').onclick = pickDir;
    $('btnOpenDir').onclick = () => invoke('open_dir');
    $('btnInstall').onclick = (e) => install(e.currentTarget);
    $('btnCheck').onclick = checkUpdates;
    $('domAdd').onclick = addDomain;
    $('domIn').onkeydown = (e) => { if (e.key === 'Enter') addDomain(); };
    $('swBypass').onchange = (e) => toggleAutostart('set_bypass_autostart', e.target, 'bypass_autostart');
    $('swApp').onchange = (e) => toggleAutostart('set_app_autostart', e.target, 'app_autostart');
    document.querySelectorAll('#segGame button').forEach((b) => (b.onclick = () => seg('set_game_mode', 'mode', b, 'game_mode')));
    document.querySelectorAll('#segIpset button').forEach((b) => (b.onclick = () => seg('set_ipset_mode', 'mode', b, 'ipset')));
    $('btnIpset').onclick = async () => {
      const b = $('btnIpset'); b.disabled = true; b.textContent = 'Загрузка…';
      try { S.ipset = await invoke('update_ipset'); renderZapret(); toast('Список обновлён'); scheduleRestart(); }
      catch (e) { toast(String(e)); }
      b.disabled = false; b.textContent = 'Обновить список';
    };
    document.querySelectorAll('#segStyle button').forEach((b) => (b.onclick = () => {
      const next = b.dataset.v;
      const accents = Object.values(DEFAULT_ACCENT).map((x) => x.toLowerCase());
      if (accents.includes(S.settings.accent.toLowerCase())) S.settings.accent = DEFAULT_ACCENT[next];
      S.settings.style = next; saveTheme(); renderSwatches(); applyTheme();
    }));
    document.querySelectorAll('#segMode button').forEach((b) => (b.onclick = () => { S.settings.mode = b.dataset.v; saveTheme(); }));

    $('swTg').onchange = async (e) => {
      const want = e.target.checked;
      tgBusy = true; renderTg();
      if (want && !S.tg.installed) toast('Скачиваем прокси…');
      try {
        S.tg = await invoke('tg_toggle', { enabled: want });
        S.settings.tg_proxy = want;
        if (!want) S.settings.tg_autostart = false;
      } catch (err) {
        e.target.checked = !want; toast(String(err));
        S.tg = await invoke('tg_state').catch(() => S.tg);
      }
      tgBusy = false; renderTg();
    };
    $('swTgAuto').onchange = async (e) => {
      const want = e.target.checked;
      try { await invoke('tg_set_autostart', { enabled: want }); S.settings.tg_autostart = want; }
      catch (err) { e.target.checked = !want; toast(String(err)); }
    };
    $('btnTgOpen').onclick = () => S.tg.link ? invoke('open_url', { url: S.tg.link }) : toast('Нет ссылки');
    $('btnTgCopy').onclick = async () => { if (S.tg.link) { await copy(S.tg.link); toast('Ссылка скопирована'); } };

    $('btnChannel').onclick = () => invoke('open_url', { url: 'https://t.me/EmzProject' });

    // BETA: experimental features (shows the "Оптимизация" section)
    $('swExperimental').onchange = async (e) => {
      const want = e.target.checked;
      try {
        await invoke('set_experimental', { enabled: want });
        S.settings.experimental = want;
        renderBeta();
        toast(want ? 'Экспериментальные функции включены' : 'Экспериментальные функции выключены');
      } catch (err) { e.target.checked = !want; toast(String(err)); }
    };

    // Transparency (Windows 11 style only)
    $('swTransp').onchange = async (e) => {
      const want = e.target.checked;
      S.settings.transparency = want;
      try { await invoke('set_transparency', { enabled: want, dark: root.dataset.mode === 'dark', persist: true }); }
      catch (err) { toast(String(err)); }
      effectKey = null;
      applyTheme();
    };

    // Optimization wiring
    if ($('swOptMaster')) $('swOptMaster').onchange = (e) => {
      antiInput.enabled = e.target.checked;
      if (!antiInput.enabled && S.tweaks_applied) resetTweaks();
      saveAntiInput();
      toast(antiInput.enabled ? 'Оптимизация сети включена' : 'Оптимизация сети выключена');
    };
    if ($('swTcpNoDelay')) $('swTcpNoDelay').onchange = (e) => {
      antiInput.tcpNoDelay = e.target.checked;
      saveAntiInput();
      toast(antiInput.tcpNoDelay ? 'TCP NoDelay включён (задержка буфера отключена)' : 'TCP NoDelay выключен');
    };
    if ($('swNetThrottle')) $('swNetThrottle').onchange = (e) => {
      antiInput.netThrottle = e.target.checked;
      saveAntiInput();
      toast(antiInput.netThrottle ? 'Network Throttling отключён (100% приоритет сети)' : 'Сетевой троттлинг Windows включён');
    };
    document.querySelectorAll('#segMtu button').forEach((b) => (b.onclick = () => {
      antiInput.mtu = b.dataset.v;
      saveAntiInput();
      toast(`MTU установлен: ${b.textContent}`);
    }));
    document.querySelectorAll('#segDns button').forEach((b) => (b.onclick = () => {
      antiInput.dns = b.dataset.v;
      saveAntiInput();
      toast(`Игровой DNS выбран: ${b.textContent}`);
    }));
    if ($('btnCopyTweaks')) $('btnCopyTweaks').onclick = async () => {
      await copy(generateTweaksScript());
      toast('Скрипт твиков скопирован в буфер обмена');
    };
    if ($('btnDownloadBat')) $('btnDownloadBat').onclick = downloadTweaksBat;
    if ($('btnResetTweaks')) $('btnResetTweaks').onclick = resetTweaks;
    if ($('btnApplyTweaks')) $('btnApplyTweaks').onclick = applyTweaks;
    EXTRA_TWEAKS.forEach(([id, key]) => { if ($(id)) $(id).onchange = (e) => { antiInput[key] = e.target.checked; saveAntiInput(); }; });
    if ($('btnFlushDns')) $('btnFlushDns').onclick = async () => {
      try { await invoke('flush_dns'); toast('Кэш DNS очищен'); } catch (e) { toast(String(e)); }
    };
    if ($('swNotify')) $('swNotify').onchange = async (e) => {
      const want = e.target.checked;
      try { await invoke('set_notify', { enabled: want }); S.settings.notify = want; toast(want ? 'Уведомления включены' : 'Уведомления выключены'); }
      catch (err) { e.target.checked = !want; toast(String(err)); }
    };
    if ($('swHeal')) $('swHeal').onchange = async (e) => {
      const want = e.target.checked;
      try { await invoke('set_auto_heal', { enabled: want }); S.settings.auto_heal = want; toast(want ? 'Автоподбор при сбое включён' : 'Автоподбор при сбое выключен'); }
      catch (err) { e.target.checked = !want; toast(String(err)); }
    };
    listen('bypass-dropped', (e) => {
      if (!S) return;
      if (e.payload === 'tg') {
        invoke('tg_state').then((t) => { S.tg = t; S.settings.tg_proxy = false; renderTg(); }).catch(() => {});
        toast('Прокси Telegram остановился');
      } else {
        S.running = false; busy = false; setPower('off');
        toast('Обход отключился');
      }
    });

    document.addEventListener('keydown', (e) => { if (e.key === 'Escape') go('home'); });

    listen('status', (e) => { if (!busy && S && !S.testing) { S.running = !!e.payload; setPower(S.installed && S.running ? 'on' : 'off'); } });
    listen('test-progress', (e) => {
      const p = e.payload; testingFile = p.file;
      $('progText').textContent = `${p.index + 1}/${p.total}`;
      $('progBar').style.width = (p.index / p.total) * 100 + '%';
      renderModes();
    });
    listen('test-result', (e) => {
      const p = e.payload; testingFile = null;
      const ms = p.ms > 0 ? p.ms : null;
      const status = p.status;
      S.settings.alt_results[p.file] = { status, ms };
      $('progBar').style.width = ((p.index + 1) / p.total) * 100 + '%';
      if (onboardTesting) {
        const pct = Math.round(((p.index + 1) / p.total) * 100);
        $('onboardProgFill').style.width = pct + '%';
        $('onboardProgPercent').textContent = pct + '%';
        $('onboardProgCount').textContent = `${p.index + 1} / ${p.total}`;
        const stName = (S.strategies.find((x) => x.file === p.file) || {}).name || p.file;
        const live = $('onboardLiveAlt');
        if (live) live.innerHTML = `<span class="live-name">${stName}</span> <span class="live-ping ${status}">${ms ? ms + ' мс' : '—'}</span>`;
      }
      renderModes();
    });
    listen('test-done', (e) => {
      testingFile = null; S.testing = false; cancelling = false; $('btnTest').disabled = false;
      const pl = (e.payload && typeof e.payload === 'object') ? e.payload : { running: !!e.payload };
      S.running = !!pl.running;
      $('prog').hidden = true; $('progText').textContent = '';
      if (pl.picked) S.settings.strategy = pl.picked;
      const best = pl.picked ? S.strategies.find((x) => x.file === pl.picked) : bestMode();
      render();
      if (onboardTesting) {
        onboardTesting = false;
        if (factTimer) { clearInterval(factTimer); factTimer = null; }
        const bestPing = (best && S.settings.alt_results[best.file]) ? S.settings.alt_results[best.file].ms : 0;
        $('onboardStepTesting').hidden = true;
        $('onboardStepDone').hidden = false;
        $('onboardBestPill').textContent = best ? `★ ${best.name}${bestPing ? ' — ' + bestPing + ' мс' : ''}` : 'Рабочий режим не найден';
      }
      if (best) { toast(`Лучший режим: ${best.name}`); changeStrategy(best.file); }
      else toast('Рабочих режимов не найдено');
    });

    const syncPaused = () => { root.dataset.paused = document.hidden ? '1' : '0'; };
    document.addEventListener('visibilitychange', syncPaused);
    window.addEventListener('blur', () => { root.dataset.paused = '1'; });
    window.addEventListener('focus', syncPaused);
    document.addEventListener('visibilitychange', async () => {
      if (document.hidden || !S || !S.installed || busy || S.testing) return;
      const r = await invoke('get_running').catch(() => null);
      if (r !== null && r !== S.running) { S.running = r; setPower(r ? 'on' : 'off'); }
    });

    setInterval(async () => {
      if (busy || !S || !S.installed || S.testing || document.hidden) return;
      const r = await invoke('get_running').catch(() => null);
      if (r !== null && r !== S.running) { S.running = r; setPower(r ? 'on' : 'off'); }
      if ($('v-tg').classList.contains('on') && !tgBusy) { S.tg = await invoke('tg_state').catch(() => S.tg); renderTg(); }
    }, 3000);
  }

  // ───────── Launch Splash & First Run Wizard ─────────

  let factTimer = null;
  let onboardTesting = false;
  let factIdx = 0;

  function startLaunchAnimation(onComplete) {
    const brand = $('titleBrand') || document.querySelector('.titlebar .brand');
    const appEl = $('app');

    if (!brand || !appEl) {
      if (appEl) appEl.classList.remove('launching');
      if (onComplete) onComplete();
      return;
    }

    // Measure the brand at its natural destination in the titlebar
    const targetRect = brand.getBoundingClientRect();
    const winW = window.innerWidth || document.documentElement.clientWidth || 800;
    const winH = window.innerHeight || document.documentElement.clientHeight || 560;

    const brandCenterTargetX = targetRect.left + (targetRect.width || 90) / 2;
    const brandCenterTargetY = targetRect.top + (targetRect.height || 26) / 2;

    const screenCenterX = winW / 2;
    const screenCenterY = winH * 0.44;

    const dx = Math.round(screenCenterX - brandCenterTargetX);
    const dy = Math.round(screenCenterY - brandCenterTargetY);
    const splashScale = 1.85;

    // Position brand in center of screen
    brand.classList.add('brand-splash-floating');
    brand.style.transformOrigin = 'center center';
    brand.style.transition = 'none';
    brand.style.transform = `translate3d(${dx}px, ${dy}px, 0) scale(${splashScale})`;

    // Force synchronous browser layout reflow
    void brand.offsetHeight;

    // Hold at center for a brief moment (~340ms) so user perceives the logo & name
    setTimeout(() => {
      // Smoothly fly back to natural destination (0, 0) scale(1)
      brand.style.transition = 'transform 0.82s cubic-bezier(0.16, 1, 0.3, 1)';
      brand.style.transform = 'translate3d(0, 0, 0) scale(1)';

      // Unblur interface in sync
      appEl.classList.remove('launching');

      // Once arrival animation finishes, clean up inline styles completely
      setTimeout(() => {
        brand.style.transition = '';
        brand.style.transform = '';
        brand.style.transformOrigin = '';
        brand.classList.remove('brand-splash-floating');
        if (onComplete) onComplete();
      }, 840);
    }, 340);
  }

  function openFirstRunModal() {
    const m = $('firstRunModal');
    if (!m) return;
    $('onboardStepPrompt').hidden = false;
    $('onboardStepTesting').hidden = true;
    $('onboardStepDone').hidden = true;
    m.hidden = false;
  }

  function closeFirstRunModal() {
    const m = $('firstRunModal');
    if (!m) return;
    try { localStorage.setItem('emz-first-run-done', '1'); } catch (_) {}
    if (factTimer) { clearInterval(factTimer); factTimer = null; }
    onboardTesting = false;
    m.classList.add('closing');
    setTimeout(() => {
      m.hidden = true;
      m.classList.remove('closing');
    }, 350);
  }

  async function startOnboardTesting() {
    if (onboardTesting) return;
    onboardTesting = true;
    $('onboardStepPrompt').hidden = true;
    $('onboardStepTesting').hidden = false;
    $('onboardStepDone').hidden = true;

    $('onboardProgFill').style.width = '0%';
    $('onboardProgPercent').textContent = '0%';
    const total = S.strategies.length || 11;
    $('onboardProgCount').textContent = `0 / ${total}`;

    // Pick random starting fact and rotate every 2.6 seconds
        if (factTimer) clearInterval(factTimer);

    const live = $('onboardLiveAlt');
    const updateLive = (name, ms, status) => {
      if (!live) return;
      live.innerHTML = `<span class="live-name">${name}</span> <span class="live-ping ${status}">${ms ? ms + ' мс' : '—'}</span>`;
    };

    if (!tauri) {
      for (let i = 0; i < total; i++) {
        if (!onboardTesting) break;
        const st = S.strategies[i];
        const pct = Math.round(((i + 1) / total) * 100);
        let ms = 24 + Math.floor(Math.random() * 45);
        if (st.name === 'ALT 1') ms = 18 + Math.floor(Math.random() * 4);
        else if (st.name.includes('ALT 9') || st.name.includes('ALT 10')) ms = 150 + Math.floor(Math.random() * 50);
        const status = ms < 60 ? 'good' : ms <= 140 ? 'mid' : 'bad';
        S.settings.alt_results[st.file] = { status, ms };

        updateLive(st.name, ms, status);
        $('onboardProgFill').style.width = pct + '%';
        $('onboardProgPercent').textContent = pct + '%';
        $('onboardProgCount').textContent = `${i + 1} / ${total}`;
        renderModes();
        await new Promise((r) => setTimeout(r, 200));
      }
    } else {
      try {
        await invoke('test_strategies', { quick: false });
        // the check runs in the background: the "test-done" event shows the result
        S.testing = true;
        S.running = false;
        render();
        return;
      } catch (err) {
        onboardTesting = false;
        toast(String(err));
        $('onboardStepTesting').hidden = true;
        $('onboardStepPrompt').hidden = false;
        return;
      }
    }

    if (!onboardTesting) return;
    onboardTesting = false;
    if (factTimer) { clearInterval(factTimer); factTimer = null; }

    const best = bestMode();
    const bestPing = (best && S.settings.alt_results[best.file]) ? S.settings.alt_results[best.file].ms : 0;
    if (best) changeStrategy(best.file);

    $('onboardStepTesting').hidden = true;
    $('onboardStepDone').hidden = false;
    $('onboardBestPill').textContent = best ? `★ ${best.name}${bestPing ? ' — ' + bestPing + ' мс' : ''}` : 'Рабочий режим не найден';
  }

  // ───────── compatibility check (other zapret / TgWsProxy already installed or running) ─────────
  let conflictOpen = false;
  function openConflictModal(list) {
    if (!tauri || conflictOpen) return Promise.resolve(false);
    return new Promise(async (resolve) => {
      if (!Array.isArray(list)) { try { list = await invoke('scan_conflicts'); } catch (_) { list = []; } }
      if (!Array.isArray(list) || !list.length) { toast('Конфликтов не найдено'); resolve(true); return; }
      conflictOpen = true;
      const box = $('confList');
      box.innerHTML = '';
      list.forEach((c) => {
        const d = document.createElement('div');
        d.className = 'conf-item';
        d.innerHTML = '<b></b><small></small>';
        d.querySelector('b').textContent = c.title;
        d.querySelector('small').textContent = c.detail || '';
        box.appendChild(d);
      });
      const folders = [...new Set(list.map((c) => c.folder).filter(Boolean))];
      $('confRemoveRow').hidden = !folders.length;
      $('confRemove').checked = false;
      $('confRemovePath').textContent = folders.join(', ');
      const m = $('conflictModal');
      m.classList.remove('closing');
      m.hidden = false;
      const close = (ok) => {
        m.classList.add('closing');
        setTimeout(() => { m.hidden = true; m.classList.remove('closing'); conflictOpen = false; resolve(ok); }, 350);
      };
      $('btnConfSkip').onclick = async () => {
        // a foreign winws.exe is always stopped: two of them fight over the same driver
        const hadWinws = list.some((c) => c.id === 'winws');
        if (hadWinws) {
          try { await invoke('stop_foreign_winws'); S = await invoke('get_state'); applyTheme(); render(); } catch (_) {}
        }
        toast(hadWinws ? 'Чужой winws остановлен. Остальные конфликты не решены' : 'Конфликт не решён: обход может работать неправильно');
        close(false);
      };
      $('btnConfFix').onclick = async () => {
        const b = $('btnConfFix');
        b.disabled = true; const old = b.textContent; b.textContent = 'Отключаем…';
        try {
          const lines = await invoke('resolve_conflicts', { removeFiles: !!$('confRemove').checked && !$('confRemoveRow').hidden, migrate: !!$('confMigrate').checked });
          try { S = await invoke('get_state'); applyTheme(); render(); } catch (_) {}
          toast(lines.slice(0, 3).join('. '));
          b.disabled = false; b.textContent = old;
          close(true);
        } catch (e) {
          b.disabled = false; b.textContent = old;
          toast(String(e));
        }
      };
    });
  }

  async function checkConflicts() {
    if (!tauri) return;
    let list = [];
    try { list = await invoke('scan_conflicts'); } catch (_) {}
    if (Array.isArray(list) && list.length) await openConflictModal(list);
  }

  function checkFirstRun() {
    let done = false;
    try { done = !!localStorage.getItem('emz-first-run-done'); } catch (_) {}
    if (!done) {
      openFirstRunModal();
    }
  }

  async function init() {
    try { await Promise.all([document.fonts.load('8px PS', 'Абвгд'), document.fonts.load('8px PS', 'Abc012')]); } catch (_) {}
    try {
      const nd = new FontFace('Ndot47', 'url(fonts/ndot-47.otf), local("Ndot 47"), local("NDot 47"), local("Ndot57"), local("Nothing")');
      await nd.load();
      document.fonts.add(nd);
    } catch (_) {}
    document.querySelectorAll('.dt').forEach((el) => el.style.setProperty('--p', el.dataset.p || '3'));
    try {
      const c = JSON.parse(localStorage.getItem('emz-theme') || 'null'); if (c && c.style === 'glass') c.style = 'terminal';
      if (c) { S = { settings: { ...c, services: {}, alt_results: {} }, strategies: [], catalog: [], tg: {} }; applyTheme(); }
    } catch (_) {}
    if (tauri) {
      const rawInvoke = invoke;
      invoke = (cmd, args) => rawInvoke(cmd, args).catch((e) => {
        const msg = String(e);
        if (msg.includes('CONFLICT')) {
          setTimeout(() => openConflictModal(), 50);
          throw msg.replace(/^.*CONFLICT:\s*/, '');
        }
        throw e;
      });
    }
    S = await invoke('get_state');
    if (S.settings.style === 'glass') { S.settings.style = 'terminal'; S.settings.accent = DEFAULT_ACCENT.terminal; invoke('set_theme', { style: 'terminal', mode: S.settings.mode, accent: S.settings.accent }).catch(() => {}); }
    renderSwatches();
    applyTheme();
    wire();
    render();
    if (S.testing) $('prog').hidden = false;
    invoke('ui_ready');
    startLaunchAnimation(async () => { await new Promise((r) => setTimeout(r, 700)); await checkConflicts(); checkFirstRun(); });
  }
  init().catch((e) => { document.body.textContent = 'Ошибка запуска: ' + e; invoke('ui_ready').catch(() => {}); });
})();
