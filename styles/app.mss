/* Legion Control — тёмная тема */

:root {
    --bg: #0b0d12;
    --bg-2: #10131a;
    --surface: #151922;
    --surface-2: #1c212c;
    --surface-3: #252b38;
    --border: #262c39;
    --border-strong: #363e4f;
    --accent: #7c5cff;
    --accent-hover: #927aff;
    --accent-soft: rgba(124, 92, 255, 0.15);
    --accent-line: rgba(124, 92, 255, 0.55);
    --cyan: #00d0ff;
    --text: #eceff5;
    --text-2: #b6bdcb;
    --muted: #7a8394;
    --ok: #34d399;
    --warn: #fbbf24;
    --bad: #f87171;
    --info: #60a5fa;
    --radius: 16px;

    /* цвета режимов питания (индикатор кнопки питания Legion) */
    --quiet: #38bdf8;
    --balanced: #e5e7eb;
    --perf: #f43f5e;
    --extreme: #d946ef;
    --custom: #a855f7;

    --popup-background: #1c212c;
    --popup-color: #eceff5;
    --popup-border: #363e4f;
    --popup-accent: #7c5cff;
    --popup-hover-background: #262d3a;
    --popup-hover-color: #ffffff;
    --popup-selected-background: rgba(124, 92, 255, 0.2);
    --popup-selected-color: #ffffff;
}

.grow { flex-grow: 1; }

Text { color: var(--text); font-size: 14px; }
Icon { color: var(--text-2); icon-size: 20px; }

.root { background: var(--bg); }

.tone-ok { color: var(--ok); }
.tone-bad { color: var(--bad); }
.tone-info { color: var(--info); }
.tone-quiet { color: var(--quiet); }
.tone-balanced { color: var(--balanced); }
.tone-perf { color: var(--perf); }
.tone-extreme { color: var(--extreme); }
.tone-custom { color: var(--custom); }

/* ---------- шапка ---------- */

.header {
    background: var(--bg-2);
    border-bottom-width: 1px;
    border-bottom-color: var(--border);
    padding: 14px 22px;
}

.brand-icon { width: 46px; height: 46px; }
.brand-kicker { color: var(--accent-hover); font-size: 11px; font-weight: bold; letter-spacing: 3px; }
.brand-title { color: var(--text); font-size: 18px; font-weight: bold; }

.pill {
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 18px;
    padding: 7px 14px;
}
.pill-text { color: var(--text-2); font-size: 13px; }
.pill-icon { icon-size: 17px; color: var(--text-2); }

.dot { width: 9px; height: 9px; border-radius: 5px; background: var(--muted); }
.dot-warn { background: var(--warn); box-shadow: 0 0 8px rgba(251, 191, 36, 0.7); }
.dot-idle { background: var(--muted); }
.dot-quiet { background: var(--quiet); box-shadow: 0 0 8px rgba(56, 189, 248, 0.8); }
.dot-balanced { background: var(--balanced); box-shadow: 0 0 8px rgba(229, 231, 235, 0.7); }
.dot-perf { background: var(--perf); box-shadow: 0 0 8px rgba(244, 63, 94, 0.8); }
.dot-extreme { background: var(--extreme); box-shadow: 0 0 8px rgba(217, 70, 239, 0.8); }
.dot-custom { background: var(--custom); box-shadow: 0 0 8px rgba(168, 85, 247, 0.8); }

/* ---------- навигация ---------- */

.nav {
    background: var(--bg-2);
    border-right-width: 1px;
    border-right-color: var(--border);
    padding: 18px 14px;
    width: 250px;
}
.nav-caption { color: var(--muted); font-size: 11px; font-weight: bold; letter-spacing: 2px; padding: 0px 10px 6px 10px; }

.nav-item {
    width: 100%;
    background: transparent;
    border-radius: 10px;
    padding: 11px 14px;
    transition: background-color 150ms ease-out;
    &:hover { background: var(--surface-2); }
}
.nav-item-on {
    background: var(--accent-soft);
    border-left: 3px solid var(--accent);
    &:hover { background: var(--accent-soft); }
}
.nav-icon { icon-size: 20px; color: var(--muted); }
.nav-icon-on { color: var(--accent-hover); }
.nav-text { color: var(--text-2); font-size: 15px; }
.nav-text-on { color: #ffffff; font-weight: 600; }

.nav-footer {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 12px 14px;
}
.nav-foot-strong { color: var(--text-2); font-size: 13px; font-weight: bold; }
.nav-foot { color: var(--muted); font-size: 12px; }

/* ---------- контент ---------- */

.content {
    background: var(--bg);
    scrollbar-width: 8px;
    scrollbar-color: #2b3240;
    scrollbar-thumb-hover-color: #3b4456;
    scrollbar-track-color: transparent;
    scrollbar-radius: 4px;
}

.page { width: 100%; }
.page-title { color: var(--text); font-size: 28px; font-weight: bold; }
.page-sub { color: var(--muted); font-size: 14px; }

.card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 20px 22px;
}
.card-title { color: var(--text); font-size: 16px; font-weight: bold; }
.card-hint { color: var(--muted); font-size: 13px; }
.card-icon { color: var(--accent-hover); icon-size: 20px; }

.row-title { color: var(--text); font-size: 15px; font-weight: 600; }
.row-desc { color: var(--muted); font-size: 13px; }
.field-label { color: var(--text-2); font-size: 13px; font-weight: 600; }
.field-value { color: var(--text); font-size: 13px; font-weight: bold; }
.scale-label { color: var(--muted); font-size: 12px; }
.muted { color: var(--muted); font-size: 13px; }
.muted-center { color: var(--muted); font-size: 12px; text-align: center; }
.changed { color: var(--warn); }
.big-line { color: var(--text); font-size: 18px; font-weight: bold; }

.kv-row { padding: 7px 0px; border-bottom-width: 1px; border-bottom-color: #1e2330; }
.kv-key { color: var(--muted); font-size: 13px; }
.kv-value { color: var(--text); font-size: 13px; font-weight: 600; }

/* ---------- полосы ---------- */

.bar-track { background: #232936; border-radius: 4px; height: 8px; }
.bar-fill { height: 8px; border-radius: 4px; background: var(--accent); }
.bar-accent { background: linear-gradient(90deg, #7c5cff, #00d0ff); }
.bar-fan { background: linear-gradient(90deg, #00b4e6, #7c5cff); }
.bar-ok { background: var(--ok); }
.bar-warn { background: var(--warn); }
.bar-bad { background: var(--bad); }

/* ---------- главная ---------- */

.hero {
    background: linear-gradient(120deg, #1a1540, #101522 55%, #0c1a26);
    border: 1px solid #2c2a55;
    border-radius: 20px;
    padding: 26px 28px;
}
.hero-kicker { color: var(--cyan); font-size: 12px; font-weight: bold; letter-spacing: 4px; }
.hero-title { color: #ffffff; font-size: 34px; font-weight: bold; }
.hero-sub { color: var(--text-2); font-size: 14px; }

.mode-chip {
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    padding: 12px 14px;
    transition: background-color 150ms ease-out, border-color 150ms ease-out;
    &:hover { background: rgba(255, 255, 255, 0.08); border-color: rgba(255, 255, 255, 0.16); }
}
.mode-chip-on { background: rgba(255, 255, 255, 0.1); }
.chip-quiet { border-color: var(--quiet); }
.chip-balanced { border-color: var(--balanced); }
.chip-perf { border-color: var(--perf); }
.chip-extreme { border-color: var(--extreme); }
.chip-custom { border-color: var(--custom); }
.mode-chip-icon { icon-size: 20px; }
.mode-chip-text { color: var(--text); font-size: 14px; font-weight: 600; }

.stat-card { padding: 18px 18px 20px 18px; width: 100%; }
.stat-line { padding: 2px 0px; }
.stat-label { color: var(--muted); font-size: 13px; }
.stat-value { color: var(--text); font-size: 13px; font-weight: bold; }
.ring-value { color: #ffffff; font-size: 30px; font-weight: bold; }
.ring-caption { color: var(--muted); font-size: 12px; }

/* ---------- производительность ---------- */

.mode-tile {
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 16px 14px;
    min-height: 150px;
    transition: background-color 150ms ease-out, border-color 150ms ease-out;
    &:hover { background: var(--surface-3); border-color: var(--border-strong); }
}
.mode-tile-on { background: #1f2231; }
.tile-quiet { border: 2px solid var(--quiet); box-shadow: 0 0 18px rgba(56, 189, 248, 0.18); }
.tile-balanced { border: 2px solid var(--balanced); box-shadow: 0 0 18px rgba(229, 231, 235, 0.12); }
.tile-perf { border: 2px solid var(--perf); box-shadow: 0 0 18px rgba(244, 63, 94, 0.2); }
.tile-extreme { border: 2px solid var(--extreme); box-shadow: 0 0 18px rgba(217, 70, 239, 0.2); }
.tile-custom { border: 2px solid var(--custom); box-shadow: 0 0 18px rgba(168, 85, 247, 0.2); }
.tile-accent { border: 2px solid var(--accent); box-shadow: 0 0 18px rgba(124, 92, 255, 0.2); }

.mode-icon-badge {
    width: 42px;
    height: 42px;
    border-radius: 12px;
    background: var(--surface-3);
}
.badge-quiet { background: rgba(56, 189, 248, 0.18); }
.badge-balanced { background: rgba(229, 231, 235, 0.14); }
.badge-perf { background: rgba(244, 63, 94, 0.18); }
.badge-extreme { background: rgba(217, 70, 239, 0.18); }
.badge-custom { background: rgba(168, 85, 247, 0.18); }
.badge-on { background: var(--accent-soft); }
.mode-icon { icon-size: 24px; color: var(--text); }
.mode-label { color: var(--text); font-size: 14px; font-weight: bold; }
.mode-hint { color: var(--muted); font-size: 12px; line-height: 17px; }

.fan-icon { icon-size: 22px; color: var(--cyan); }

.banner {
    background: rgba(248, 113, 113, 0.08);
    border: 1px solid rgba(248, 113, 113, 0.4);
    border-radius: 14px;
    padding: 16px 20px;
}
.banner-info {
    background: rgba(124, 92, 255, 0.08);
    border: 1px solid rgba(124, 92, 255, 0.4);
}
.banner-icon { icon-size: 28px; color: var(--bad); }
.banner-icon-info { icon-size: 26px; color: var(--accent-hover); }
.code-box { background: #07090d; border: 1px solid var(--border); border-radius: 10px; padding: 10px 14px; }
.code { color: #9fe3b8; font-size: 13px; font-family: monospace; }

/* ---------- подсветка ---------- */

.profile-btn {
    background: var(--surface-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 12px 0px;
    min-width: 52px;
    font-size: 17px;
    font-weight: bold;
    &:hover { background: var(--surface-3); color: var(--text); }
}
.profile-btn-on {
    background: var(--accent);
    color: #ffffff;
    border-color: var(--accent-hover);
    box-shadow: 0 4px 16px rgba(124, 92, 255, 0.35);
    &:hover { background: var(--accent-hover); }
}

.kb-body {
    background: linear-gradient(180deg, #0e1117, #07080c);
    border: 1px solid #232a38;
    border-radius: 18px;
    padding: 18px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.45);
}

.zone {
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 6px;
    transition: border-color 120ms ease-out;
    &:hover { border: 2px solid rgba(255, 255, 255, 0.55); }
}
.zone-case { border-radius: 4px; }
.zone-on {
    border: 2px solid #ffffff;
    box-shadow: 0 0 10px rgba(255, 255, 255, 0.6);
    &:hover { border: 2px solid #ffffff; }
}
.zone-label {
    color: #ffffff;
    font-size: 11px;
    font-weight: bold;
    text-shadow: 0 1px 3px rgba(0, 0, 0, 0.95);
}
.zone-label-sm { font-size: 9px; }

.zone-case-sample { width: 18px; height: 6px; border-radius: 3px; background: #3a4150; }
.zone-on-sample { width: 14px; height: 14px; border-radius: 4px; border: 2px solid #ffffff; }

.logo-chip {
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 16px;
    padding: 6px 14px;
    &:hover { border-color: rgba(255, 255, 255, 0.5); }
}
.logo-chip-on { border: 2px solid #ffffff; }
.logo-dot { width: 14px; height: 14px; border-radius: 7px; }
.logo-text { color: var(--text-2); font-size: 12px; font-weight: bold; letter-spacing: 1px; }

.layers-col { width: 420px; }

.layer-row {
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 10px 12px;
    &:hover { border-color: var(--border-strong); }
}
.layer-row-on { background: var(--accent-soft); border-color: var(--accent); &:hover { border-color: var(--accent-hover); } }
.layer-icon { icon-size: 22px; color: var(--accent-hover); }
.mini-dot { width: 12px; height: 12px; border-radius: 6px; border: 1px solid rgba(255, 255, 255, 0.2); }
.mini-rainbow { background: conic-gradient(from 0deg at center, #ff0040, #ffd000, #30ff60, #00d0ff, #7c5cff, #ff0040); }

.fx-tile {
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 12px 6px;
    min-width: 118px;
    transition: background-color 150ms ease-out, border-color 150ms ease-out;
    &:hover { background: var(--surface-3); border-color: var(--border-strong); }
}
.fx-tile-on { background: var(--accent-soft); border-color: var(--accent); &:hover { background: var(--accent-soft); } }
.fx-icon { icon-size: 24px; color: var(--text-2); }
.fx-label { color: var(--text); font-size: 12px; text-align: center; }

.color-swatch {
    width: 28px;
    height: 28px;
    border-radius: 14px;
    border: 2px solid rgba(255, 255, 255, 0.14);
}
.color-swatch-on {
    border: 3px solid #ffffff;
    box-shadow: 0 0 10px rgba(255, 255, 255, 0.4);
}

.save-bar {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 12px 16px;
}

/* ---------- устройство ---------- */

.row-icon-badge { width: 40px; height: 40px; border-radius: 12px; background: var(--surface-3); }
.row-icon { icon-size: 22px; color: var(--accent-hover); }

/* ---------- заглушка ---------- */

.ph-card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 18px;
    padding: 36px 44px;
}
.ph-badge {
    width: 72px;
    height: 72px;
    border-radius: 36px;
    background: var(--accent-soft);
    border: 1px solid var(--accent-line);
}
.ph-icon { color: var(--accent-hover); icon-size: 36px; }
.ph-title { color: var(--text); font-size: 22px; font-weight: bold; }
.ph-hint { color: var(--text-2); font-size: 14px; text-align: center; line-height: 22px; }

/* ---------- стандартные контролы ---------- */

Button {
    background: var(--surface-2);
    color: var(--text);
    border-radius: 10px;
    padding: 9px 16px;
    font-size: 14px;
    icon-size: 18px;
    transition: background-color 150ms ease-out;
    &:hover { background: var(--surface-3); }
    &:pressed { background: var(--border); }
    &:disabled { background: var(--surface); color: #535a68; icon-color: #535a68; }
}

.btn-primary {
    background: var(--accent);
    color: #ffffff;
    font-weight: bold;
    padding: 10px 20px;
    &:hover { background: var(--accent-hover); }
    &:pressed { background: #6547f0; }
    &:disabled { background: #252a36; color: #6b7280; icon-color: #6b7280; box-shadow: none; }
}

.btn-ghost {
    background: transparent;
    color: var(--text-2);
    border: 1px solid var(--border-strong);
    &:hover { background: var(--surface-2); color: var(--text); }
    &:disabled { background: transparent; color: #4b5260; border-color: var(--border); icon-color: #4b5260; }
}

.chip {
    background: var(--surface-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 16px;
    padding: 6px 14px;
    font-size: 13px;
    icon-size: 16px;
    &:hover { background: var(--surface-3); color: var(--text); }
}

ToolButton {
    background: transparent;
    color: var(--text-2);
    border-radius: 8px;
    padding: 5px;
    icon-size: 18px;
    &:hover { background: var(--surface-3); color: var(--text); }
    &:disabled { color: #3e4452; icon-color: #3e4452; }
}

Slider {
    height: 6px;
    background: #2a3040;
    color: var(--accent);
    accent-color: var(--accent);
    border-radius: 3px;
    label-color: var(--text);
    value-font-size: 13px;
    caret-color: var(--accent);
}
.wide-slider { width: 100%; }

Toggle {
    width: 46px;
    height: 26px;
    background: #2e3444;
    color: #ffffff;
    accent-color: var(--accent);
    border-radius: 13px;
    transition: background-color 200ms ease-out;
    &:checked { background: var(--accent); }
}
.toggle-off { opacity: 0.45; }

ColorPicker {
    background: var(--surface-2);
    color: var(--text);
    accent-color: var(--accent);
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    font-size: 13px;
}

SegmentedButton {
    background: var(--surface-2);
    color: var(--text-2);
    accent-color: var(--accent);
    border-color: var(--border-strong);
    border-radius: 10px;
    font-size: 13px;
    height: 36px;
    --selected-background: #7c5cff;
    --selected-color: #ffffff;
}

Tooltip { background: #2a303c; color: var(--text); font-size: 12px; border-radius: 6px; }

/* ---------- нижняя панель ---------- */

.footer {
    background: var(--bg-2);
    border-top-width: 1px;
    border-top-color: var(--border);
    padding: 10px 22px;
}
.footer-note { color: var(--muted); font-size: 12px; }
.status-text { color: var(--text-2); font-size: 13px; }
.status-icon { icon-size: 18px; }

CircularProgress { color: var(--accent); accent-color: var(--accent); track-color: #2a2f3b; }
