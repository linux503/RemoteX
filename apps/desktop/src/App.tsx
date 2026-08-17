import { useEffect, useMemo, useState, type ReactNode } from "react";
import { previewSnapshot, type AppSettings, type PermissionsStatus, type Snapshot } from "./types";
import { resolveLocale, t, translateError, type Locale, type MessageKey } from "./i18n";

const isTauri = () => "__TAURI_INTERNALS__" in window;

function formatMbps(kbps: number) {
  if (!kbps) return "—";
  if (kbps < 1000) return `${kbps} kbps`;
  return `${(kbps / 1000).toFixed(1)} Mbps`;
}

function latencyTone(ms: number) {
  if (!ms) return "";
  if (ms < 45) return "good";
  if (ms < 90) return "ok";
  return "bad";
}

const mockPermissions = (): PermissionsStatus => ({
  screen_recording: "denied",
  accessibility: "denied",
  input_monitoring: "unknown",
  platform: "macos",
  all_granted: false,
});

async function fetchPermissions(): Promise<PermissionsStatus | null> {
  if (!isTauri()) return mockPermissions();
  try {
    return await invoke<PermissionsStatus>("permissions_status");
  } catch {
    return null;
  }
}

function permLabel(locale: Locale, state: PermissionsStatus[keyof PermissionsStatus]) {
  if (state === "granted") return t(locale, "permGranted");
  if (state === "denied") return t(locale, "permDenied");
  return t(locale, "permUnknown");
}

function permDotClass(state: string) {
  if (state === "granted") return "ready";
  if (state === "denied") return "offline";
  return "warn";
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

const previewScene =
  typeof window !== "undefined" ? new URLSearchParams(window.location.search).get("scene") : null;

export default function App() {
  const [snap, setSnap] = useState<Snapshot>(() => {
    const next = previewSnapshot(previewScene);
    const lang = typeof window !== "undefined" ? new URLSearchParams(window.location.search).get("lang") : null;
    if (previewScene && (lang === "en" || lang === "zh")) {
      return { ...next, settings: { ...next.settings, language: lang } };
    }
    return next;
  });
  const [view, setView] = useState<"home" | "settings">(previewScene === "settings" ? "settings" : "home");
  const [hidePassword, setHidePassword] = useState(false);
  const [connectId, setConnectId] = useState("");
  const [connectPassword, setConnectPassword] = useState("");
  const [passwordStep, setPasswordStep] = useState(false);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState("");
  const [qualityOpen, setQualityOpen] = useState(false);
  const [displayOpen, setDisplayOpen] = useState(false);
  const [connectStep, setConnectStep] = useState(0);
  const [toast, setToast] = useState("");
  const [settingsTab, setSettingsTab] = useState(
    previewScene === "settings" || previewScene === "permissions" ? (previewScene === "permissions" ? "permissions" : "general") : "general",
  );
  const [perms, setPerms] = useState<PermissionsStatus | null>(null);

  useEffect(() => {
    if (!isTauri()) {
      setPerms(mockPermissions());
      return;
    }
    void fetchPermissions().then(setPerms);
    const timer = window.setInterval(() => {
      void fetchPermissions().then(setPerms);
    }, 2500);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    const theme = snap.settings.theme;
    const root = document.documentElement;
    if (previewScene) {
      const themeParam = new URLSearchParams(window.location.search).get("theme");
      root.dataset.theme = themeParam === "dark" ? "dark" : "light";
      return;
    }
    if (theme === "light" || theme === "dark") {
      root.dataset.theme = theme;
      return;
    }
    delete root.dataset.theme;
  }, [snap.settings.theme]);

  useEffect(() => {
    if (previewScene === "connecting") {
      setConnectStep(2);
      return;
    }
    if (snap.phase !== "connecting") {
      setConnectStep(0);
      return;
    }
    const timers = [400, 900, 1500, 2100].map((ms, index) =>
      window.setTimeout(() => setConnectStep(index + 1), ms),
    );
    return () => timers.forEach(clearTimeout);
  }, [snap.phase]);

  useEffect(() => {
    if (!isTauri()) return;
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const current = await invoke<Snapshot>("snapshot");
        setSnap(current);
        const { listen } = await import("@tauri-apps/api/event");
        unlisten = await listen<Snapshot>("snapshot", (event) => setSnap(event.payload));
        await listen<string>("copy", async (event) => {
          await navigator.clipboard.writeText(event.payload);
        });
      } catch (err) {
        console.error(err);
      }
    })();
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    void (async () => {
      const { getCurrentWindow, LogicalSize } = await import("@tauri-apps/api/window");
      const win = getCurrentWindow();
      if (snap.phase === "connected") {
        await win.setSize(new LogicalSize(980, 720));
        await win.setMinSize(new LogicalSize(720, 520));
        return;
      }
      await win.setMinSize(new LogicalSize(400, 620));
      await win.setSize(new LogicalSize(440, 720));
    })();
  }, [snap.phase]);

  useEffect(() => {
    if (isTauri() || previewScene || snap.phase !== "connected") return;
    const timer = window.setInterval(() => {
      setSnap((prev) => {
        if (!prev.session) return prev;
        const wave = 1 + Math.random() * 0.12 - 0.06;
        const rtt = 26 + Math.round(Math.random() * 20);
        return {
          ...prev,
          rtt_ms: rtt,
          session: {
            ...prev.session,
            rtt_ms: rtt,
            down_kbps: Math.round(8200 * wave),
            up_kbps: 120 + Math.round(Math.random() * 90),
          },
        };
      });
    }, 1400);
    return () => window.clearInterval(timer);
  }, [snap.phase]);

  const locale = resolveLocale(snap.settings.language);
  const tr = (key: MessageKey) => t(locale, key);

  useEffect(() => {
    if (snap.last_error) setError(translateError(locale, snap.last_error));
  }, [snap.last_error, locale]);

  const formatIdInput = (value: string) => {
    const digits = value.replace(/\D/g, "").slice(0, 9);
    return digits.replace(/(\d{3})(\d{0,3})(\d{0,3})/, (_, a, b, c) =>
      [a, b, c].filter(Boolean).join(" "),
    );
  };

  const copy = async (label: string, value: string) => {
    await navigator.clipboard.writeText(value.replace(/\s/g, ""));
    setCopied(label);
    window.setTimeout(() => setCopied(""), 1200);
  };

  const showSoon = () => {
    setToast(tr("comingSoon"));
    window.setTimeout(() => setToast(""), 1400);
  };

  const run = async (fn: () => Promise<unknown>) => {
    try {
      setError("");
      await fn();
    } catch (err) {
      const message =
        typeof err === "object" && err && "message" in err
          ? String((err as { message: string }).message)
          : String(err);
      setError(translateError(locale, message));
    }
  };

  const startConnect = () => {
    const id = connectId.replace(/\D/g, "");
    if (id.length !== 9) {
      setError(tr("invalidId"));
      return;
    }
    setError("");
    setPasswordStep(true);
  };

  const confirmConnect = () =>
    run(async () => {
      if (isTauri()) {
        await invoke("connect", {
          targetId: connectId,
          password: connectPassword,
        });
      } else {
        setSnap({
          ...snap,
          phase: "connecting",
          session: {
            session_id: "demo",
            peer_id: connectId.replace(/\D/g, ""),
            peer_name: connectId,
            peer_os: "windows",
            rtt_ms: 0,
            down_kbps: 0,
            up_kbps: 0,
            path: "unknown",
            quality: "balanced",
          },
        });
        window.setTimeout(() => {
          setSnap((prev) => ({
            ...prev,
            phase: "connected",
            session: prev.session
              ? { ...prev.session, rtt_ms: 36, down_kbps: 8200, up_kbps: 180, path: "p2p", peer_name: "Office PC" }
              : prev.session,
          }));
        }, 2400);
      }
      setPasswordStep(false);
      setConnectPassword("");
    });

  const updateSettings = (patch: Partial<AppSettings>) => {
    const settings = { ...snap.settings, ...patch };
    setSnap({ ...snap, settings, unattended: settings.unattended });
    if (isTauri()) {
      void invoke("save_settings", { settings });
    }
  };

  const qualityLabel = useMemo(() => {
    const map: Record<string, MessageKey> = {
      smooth: "qualitySmooth",
      balanced: "qualityBalanced",
      high: "qualityHigh",
      original: "qualityOriginal",
    };
    return t(locale, map[snap.settings.quality] ?? "qualityBalanced");
  }, [snap.settings.quality, locale]);

  if (snap.phase === "connected" && snap.session) {
    return (
      <div className="app session-app">
        <Titlebar
          onSettings={() => setView("settings")}
          compact
          subtitle={`${snap.session.peer_name} · ${snap.session.path === "p2p" ? "P2P" : "Relay"}`}
        />
        <div
          className="session-toolbar"
          onMouseLeave={() => {
            setQualityOpen(false);
            setDisplayOpen(false);
          }}
        >
          <span className="brand-mini">
            <Logo size={14} /> RemoteX
          </span>
          <span>{snap.session.peer_name}</span>
          <span className="pill">{snap.session.rtt_ms || "—"} ms</span>
          <span className="pill">{formatMbps(snap.session.down_kbps)} ↓</span>
          <span className="toolbar-grow" />
          <button className="ghost" onClick={() => setDisplayOpen((v) => !v)}>
            {tr("display")}
          </button>
          <button className="ghost" onClick={() => setQualityOpen((v) => !v)}>
            {qualityLabel}
          </button>
          <button className="ghost" onClick={showSoon}>
            {tr("keys")}
          </button>
          <button className="ghost" onClick={showSoon}>
            {tr("files")}
          </button>
          <button className="danger" onClick={() => run(() => isTauri() ? invoke("hangup") : Promise.resolve(setSnap({ ...snap, phase: "idle", session: null })))}>
            {tr("end")}
          </button>
          {displayOpen && (
            <div className="popover">
              <p className="label">{tr("display")}</p>
              <label className="choice"><input type="radio" defaultChecked readOnly /> {tr("display1")}</label>
              <label className="choice"><input type="radio" disabled /> {tr("display2")}</label>
              <button className="ghost">{tr("showAll")}</button>
            </div>
          )}
          {qualityOpen && (
            <div className="popover right">
              <p className="label">{tr("displayQuality")}</p>
              {(["smooth", "balanced", "high", "original"] as const).map((item) => (
                <label className="choice" key={item}>
                  <input
                    type="radio"
                    checked={snap.settings.quality === item}
                    onChange={() => updateSettings({ quality: item })}
                  />
                  {t(locale, ({
                    smooth: "qualitySmooth",
                    balanced: "qualityBalanced",
                    high: "qualityHigh",
                    original: "qualityOriginal",
                  } as const)[item])}
                </label>
              ))}
              <p className="label">{tr("resolution")}</p>
              <label className="choice"><input type="radio" defaultChecked readOnly /> {tr("auto")}</label>
            </div>
          )}
        </div>
        <div className="session-stats">
          <div className={`stat ${latencyTone(snap.session.rtt_ms)}`}>
            <span>{tr("latency")}</span>
            <strong>{snap.session.rtt_ms ? `${snap.session.rtt_ms} ms` : "—"}</strong>
          </div>
          <div className="stat good">
            <span>{tr("download")}</span>
            <strong>{formatMbps(snap.session.down_kbps)}</strong>
          </div>
          <div className="stat">
            <span>{tr("upload")}</span>
            <strong>{formatMbps(snap.session.up_kbps)}</strong>
          </div>
          <div className="stat">
            <span>{tr("path")}</span>
            <strong>{snap.session.path === "p2p" ? tr("directP2p") : tr("relay")}</strong>
          </div>
        </div>
        <div className="desktop-stage">
          <div className="desktop-canvas">
            <p>{tr("remoteDesktop")}</p>
            <span>{snap.session.peer_os === "macos" ? "macOS" : "Windows"} {tr("screen")}</span>
          </div>
        </div>
        {toast && <div className="toast">{toast}</div>}
      </div>
    );
  }

  return (
    <div className="app">
      <Titlebar onSettings={() => setView(view === "settings" ? "home" : "settings")} settingsLabel={tr("settings")} />

      {view === "settings" ? (
        <Settings
          snap={snap}
          locale={locale}
          initialTab={settingsTab}
          perms={perms}
          onRefreshPerms={() => void fetchPermissions().then(setPerms)}
          onBack={() => setView("home")}
          onSettings={updateSettings}
          onPermanentPassword={(password) => {
            if (isTauri()) void invoke("set_permanent_password", { password });
          }}
        />
      ) : (
        <main className="home">
          {perms && perms.platform === "macos" && !perms.all_granted && (
            <section className="perm-banner">
              <p>{tr("permNeeded")}</p>
              <button
                className="primary"
                onClick={() => {
                  setSettingsTab("permissions");
                  setView("settings");
                }}
              >
                {tr("permGoSettings")}
              </button>
            </section>
          )}
          <header className="hero">
            <Logo size={28} />
            <div>
              <h1>{tr("remoteDesktop")}</h1>
              <p>{tr("tagline")}</p>
            </div>
          </header>

          <section className="card device-card">
            <p className="eyebrow">{tr("thisDevice")}</p>
            <div className="id-row">
              <h2>{snap.formatted_id}</h2>
              <button className="icon-btn" onClick={() => copy("ID", snap.formatted_id)} title={tr("copyId")}>
                {copied === "ID" ? "✓" : "⧉"}
              </button>
            </div>
            <div className={`status ${snap.ready ? "ready" : "offline"}`}>
              <span className="dot" />
              {snap.ready ? tr("ready") : tr("connectingNetwork")}
              {snap.ready && snap.rtt_ms > 0 ? ` · ${snap.rtt_ms}ms` : ""}
            </div>
            <p className="label">{tr("tempPassword")}</p>
            <div className="password-row">
              <strong>{hidePassword ? "• • • • • •" : snap.formatted_password}</strong>
              <div className="row-actions">
                <button className="icon-btn" onClick={() => setHidePassword((v) => !v)}>
                  {hidePassword ? "○" : "●"}
                </button>
                <button
                  className="icon-btn"
                  onClick={() =>
                    run(async () => {
                      if (isTauri()) {
                        const next = await invoke<Snapshot>("refresh_password");
                        setSnap(next);
                      } else {
                        setSnap({ ...snap, formatted_password: "K 3 M 8 Q 4", temp_password: "K3M8Q4" });
                      }
                    })
                  }
                  title={tr("refreshPassword")}
                >
                  ↻
                </button>
              </div>
            </div>
          </section>

          <section className="connect">
            <p className="label">{tr("connectTo")}</p>
            <div className="connect-row">
              <input
                value={connectId}
                onChange={(e) => setConnectId(formatIdInput(e.target.value))}
                placeholder={tr("enterId")}
                inputMode="numeric"
                onKeyDown={(e) => e.key === "Enter" && startConnect()}
              />
              <button className="primary" onClick={startConnect}>
                {tr("connect")}
              </button>
            </div>
            {error && <p className="error">{error}</p>}
          </section>

          {snap.nearby && snap.nearby.length > 0 && (
            <section className="recents">
              <p className="label">{tr("nearby")}</p>
              {snap.nearby.map((item) => (
                <button
                  key={item.id}
                  className="recent-item"
                  onClick={() => {
                    setConnectId(item.id.replace(/(\d{3})(\d{3})(\d{3})/, "$1 $2 $3"));
                    setPasswordStep(true);
                  }}
                >
                  <span className="recent-icon">{item.os === "macos" ? "⌘" : "▣"}</span>
                  <span className="recent-name">{item.name}</span>
                  <span className="muted">{item.id.replace(/(\d{3})(\d{3})(\d{3})/, "$1 $2 $3")}</span>
                  <span className="dot ready" />
                </button>
              ))}
            </section>
          )}

          {snap.recents.length > 0 && (
            <section className="recents">
              <p className="label">{tr("recent")}</p>
              {snap.recents.map((item) => (
                <button
                  key={item.id}
                  className="recent-item"
                  onClick={() => {
                    setConnectId(item.id.replace(/(\d{3})(\d{3})(\d{3})/, "$1 $2 $3"));
                    setPasswordStep(true);
                  }}
                >
                  <span className="recent-icon">{item.os === "macos" ? "⌘" : "▣"}</span>
                  <span className="recent-name">
                    {item.favorite ? "★ " : ""}
                    {item.name}
                  </span>
                  <span className="muted">{item.id.replace(/(\d{3})(\d{3})(\d{3})/, "$1 $2 $3")}</span>
                  <span className="chevron">→</span>
                </button>
              ))}
            </section>
          )}

          <footer>RemoteX v0.2.2</footer>
        </main>
      )}

      {toast && view !== "settings" && snap.phase !== "connected" && <div className="toast">{toast}</div>}

      {passwordStep && (
        <div className="overlay">
          <div className="modal">
            <p className="eyebrow">{tr("connect")}</p>
            <h3>{connectId}</h3>
            <p className="muted">{tr("enterTempPassword")}</p>
            <input
              autoFocus
              value={connectPassword}
              onChange={(e) => setConnectPassword(e.target.value.toUpperCase())}
              placeholder={tr("password")}
              onKeyDown={(e) => e.key === "Enter" && void confirmConnect()}
            />
            {error && <p className="error">{error}</p>}
            <div className="modal-actions">
              <button onClick={() => setPasswordStep(false)}>{tr("cancel")}</button>
              <button className="primary" onClick={() => void confirmConnect()}>
                {tr("continue")}
              </button>
            </div>
          </div>
        </div>
      )}

      {snap.phase === "connecting" && (
        <div className="overlay connecting">
          <Logo size={36} />
          <p className="eyebrow">RemoteX</p>
          <h2>{tr("connecting")}</h2>
          <p>{snap.session?.peer_name ?? tr("remoteDevice")}</p>
          <div className="link-graph">
            <span>{tr("you")}</span>
            <i />
            <b />
            <i />
            <span>{snap.session?.peer_name ?? tr("peer")}</span>
          </div>
          <ul className="steps">
            {(["stepFind", "stepHandshake", "stepP2p", "stepVideo"] as const).map((key, index) => (
              <li key={key} className={connectStep > index ? "done" : connectStep === index ? "active" : ""}>
                <span>{connectStep > index ? "✓" : "●"}</span>
                {tr(key)}
              </li>
            ))}
          </ul>
        </div>
      )}

      {snap.phase === "incoming" && snap.incoming && (
        <div className="overlay">
          <div className="modal">
            <p className="eyebrow">{tr("incomingTitle")}</p>
            <h3>{snap.incoming.from_name}</h3>
            <p>{tr("incomingBody")}</p>
            <div className="modal-actions">
              <button onClick={() => run(() => isTauri() ? invoke("decline") : Promise.resolve(setSnap({ ...snap, phase: "idle", incoming: null })))}>
                {tr("decline")}
              </button>
              <button className="primary" onClick={() => run(async () => {
                if (isTauri()) {
                  await invoke("accept");
                  return;
                }
                setSnap((prev) => ({
                  ...prev,
                  phase: "connected",
                  incoming: null,
                  session: {
                    session_id: "preview",
                    peer_id: prev.incoming?.from_id ?? "391285663",
                    peer_name: prev.incoming?.from_name ?? "Office PC",
                    peer_os: prev.incoming?.from_os ?? "windows",
                    rtt_ms: 36,
                    down_kbps: 8420,
                    up_kbps: 186,
                    path: "p2p",
                    quality: prev.settings.quality,
                  },
                }));
              })}>
                {tr("accept")}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function Titlebar({
  onSettings,
  compact,
  subtitle,
  settingsLabel,
}: {
  onSettings: () => void;
  compact?: boolean;
  subtitle?: string;
  settingsLabel?: string;
}) {
  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="titlebar-label">
        <Logo size={16} />
        <span>RemoteX</span>
        {subtitle && <span className="muted">{subtitle}</span>}
      </div>
      {!compact && (
        <button className="icon-btn" onClick={onSettings} title={settingsLabel ?? "Settings"}>
          ⚙
        </button>
      )}
    </div>
  );
}

function Logo({ size }: { size: number }) {
  const id = `rx-${size}`;
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" className="logo" aria-hidden>
      <defs>
        <linearGradient id={id} x1="6" y1="2" x2="28" y2="30" gradientUnits="userSpaceOnUse">
          <stop stopColor="#FF3B55" />
          <stop offset="1" stopColor="#9B1230" />
        </linearGradient>
      </defs>
      <rect x="1" y="1" width="30" height="30" rx="8" fill={`url(#${id})`} />
      <g stroke="#fff" strokeWidth="2.7" strokeLinecap="round">
        <path d="M9.2 9.2 L13.05 13.05" />
        <path d="M22.8 9.2 L18.95 13.05" />
        <path d="M9.2 22.8 L13.05 18.95" />
        <path d="M22.8 22.8 L18.95 18.95" />
      </g>
      <circle cx="16" cy="16" r="2.15" fill="#fff" />
    </svg>
  );
}

function Settings({
  snap,
  locale,
  initialTab,
  perms,
  onRefreshPerms,
  onBack,
  onSettings,
  onPermanentPassword,
}: {
  snap: Snapshot;
  locale: Locale;
  initialTab: string;
  perms: PermissionsStatus | null;
  onRefreshPerms: () => void;
  onBack: () => void;
  onSettings: (patch: Partial<AppSettings>) => void;
  onPermanentPassword: (password: string) => void;
}) {
  const [tab, setTab] = useState(initialTab);
  const [permanent, setPermanent] = useState("");
  const [copiedLan, setCopiedLan] = useState(false);
  const tr = (key: MessageKey) => t(locale, key);
  const tabs: { id: string; key: MessageKey }[] = [
    { id: "general", key: "tabGeneral" },
    { id: "connection", key: "tabConnection" },
    { id: "security", key: "tabSecurity" },
    { id: "display", key: "tabDisplay" },
    { id: "permissions", key: "tabPermissions" },
    { id: "about", key: "tabAbout" },
  ];

  return (
    <main className="settings">
      <header className="settings-head">
        <button className="back" onClick={onBack}>← {tr("back")}</button>
        <h1>{tr("settings")}</h1>
      </header>
      <div className="settings-shell">
        <nav className="settings-nav">
          {tabs.map((item) => (
            <button key={item.id} className={tab === item.id ? "active" : ""} onClick={() => setTab(item.id)}>
              {tr(item.key)}
            </button>
          ))}
        </nav>
        <div className="settings-pane">
          {tab === "general" && (
            <section className="card">
              <p className="pane-title">{tr("tabGeneral")}</p>
              <Toggle label={tr("startAtLogin")} checked={snap.settings.start_at_login} onChange={(v) => onSettings({ start_at_login: v })} />
              <Toggle label={tr("minimizeToTray")} checked={snap.settings.minimize_to_tray} onChange={(v) => onSettings({ minimize_to_tray: v })} />
              <Toggle label={tr("autoUpdate")} checked={snap.settings.auto_update} onChange={(v) => onSettings({ auto_update: v })} />
              <label className="set-row">
                <span>{tr("language")}</span>
                <select value={snap.settings.language} onChange={(e) => onSettings({ language: e.target.value })}>
                  <option value="system">{tr("languageSystem")}</option>
                  <option value="en">{tr("languageEn")}</option>
                  <option value="zh">{tr("languageZh")}</option>
                </select>
              </label>
              <label className="set-row">
                <span>{tr("theme")}</span>
                <select value={snap.settings.theme} onChange={(e) => onSettings({ theme: e.target.value })}>
                  <option value="system">{tr("themeSystem")}</option>
                  <option value="light">{tr("themeLight")}</option>
                  <option value="dark">{tr("themeDark")}</option>
                </select>
              </label>
            </section>
          )}

          {tab === "connection" && (
            <section className="card">
              <p className="pane-title">{tr("tabConnection")}</p>
              <Toggle label={tr("preferP2p")} checked={snap.settings.p2p_preferred} onChange={(v) => onSettings({ p2p_preferred: v })} />
              <Toggle label={tr("hardwareEncode")} checked={snap.settings.hardware_encode} onChange={(v) => onSettings({ hardware_encode: v })} />
              <label className="set-stack">
                <span>{tr("signalingServer")}</span>
                <input
                  value={snap.settings.signaling_url}
                  onChange={(e) => onSettings({ signaling_url: e.target.value })}
                />
              </label>
              <p className="hint">{tr("signalingHint")}</p>
              {snap.lan_url && (
                <>
                  <p className="hint">{tr("lanHint")}</p>
                  <button
                    type="button"
                    className="lan-copy"
                    onClick={async () => {
                      await navigator.clipboard.writeText(snap.lan_url);
                      setCopiedLan(true);
                      window.setTimeout(() => setCopiedLan(false), 1200);
                    }}
                  >
                    {copiedLan ? "✓" : snap.lan_url.replace("ws://", "").replace("/ws", "")}
                  </button>
                </>
              )}
              <p className="hint">{tr("connection")}: {snap.ready ? tr("online") : tr("offline")}</p>
            </section>
          )}

          {tab === "security" && (
            <section className="card">
              <p className="pane-title">{tr("tabSecurity")}</p>
              <Toggle label={tr("unattended")} checked={snap.settings.unattended} onChange={(v) => onSettings({ unattended: v })} />
              <label className="set-stack">
                <span>{tr("permanentPassword")}</span>
                <input
                  type="password"
                  value={permanent}
                  placeholder={snap.has_permanent_password ? "••••••••" : tr("setPassword")}
                  onChange={(e) => setPermanent(e.target.value)}
                  onBlur={() => permanent && onPermanentPassword(permanent)}
                />
              </label>
              <Toggle label={tr("askBeforeConnecting")} checked={snap.settings.require_confirm} onChange={(v) => onSettings({ require_confirm: v })} />
              <Toggle label={tr("allowClipboard")} checked={snap.settings.allow_clipboard} onChange={(v) => onSettings({ allow_clipboard: v })} />
              <Toggle label={tr("allowFileTransfer")} checked={snap.settings.allow_file_transfer} onChange={(v) => onSettings({ allow_file_transfer: v })} />
              <Toggle label={tr("lockAfterSession")} checked={snap.settings.lock_after_session} onChange={(v) => onSettings({ lock_after_session: v })} />
            </section>
          )}

          {tab === "display" && (
            <section className="card">
              <p className="pane-title">{tr("tabDisplay")}</p>
              <label className="set-row">
                <span>{tr("quality")}</span>
                <select value={snap.settings.quality} onChange={(e) => onSettings({ quality: e.target.value })}>
                  <option value="smooth">{tr("qualitySmooth")}</option>
                  <option value="balanced">{tr("qualityBalanced")}</option>
                  <option value="high">{tr("qualityHigh")}</option>
                  <option value="original">{tr("qualityOriginal")}</option>
                </select>
              </label>
              <label className="set-row">
                <span>{tr("fps")}</span>
                <select value={snap.settings.fps} onChange={(e) => onSettings({ fps: Number(e.target.value) })}>
                  <option value={30}>30</option>
                  <option value={60}>60</option>
                  <option value={120}>120</option>
                </select>
              </label>
            </section>
          )}

          {tab === "permissions" && (
            <section className="card permissions">
              <p className="pane-title">{tr("systemPermissions")}</p>
              <p className="hint">{tr("permHint")}</p>
              {perms?.all_granted ? (
                <p className="perm-ready">{tr("permReady")}</p>
              ) : (
                <div className="perm-guide">
                  <p className="label">{tr("permGuideTitle")}</p>
                  <ol>
                    <li>{tr("permStep1")}</li>
                    <li>{tr("permStep2")}</li>
                    <li>{tr("permStep3")}</li>
                  </ol>
                  <p className="hint">{tr("permRestart")}</p>
                </div>
              )}
              <PermRow
                locale={locale}
                title={tr("screenRecording")}
                why={tr("permWhyScreen")}
                state={perms?.screen_recording ?? "unknown"}
                actions={
                  <>
                    <button
                      type="button"
                      className="ghost perm-action"
                      onClick={() => isTauri() && void invoke("request_screen_recording")}
                    >
                      {tr("permRequestScreen")}
                    </button>
                    <button
                      type="button"
                      className="ghost perm-action"
                      onClick={() => isTauri() && void invoke("open_permission_panel", { kind: "screen_recording" })}
                    >
                      {tr("permOpen")}
                    </button>
                  </>
                }
              />
              <PermRow
                locale={locale}
                title={tr("accessibility")}
                why={tr("permWhyAccessibility")}
                state={perms?.accessibility ?? "unknown"}
                actions={
                  <button
                    type="button"
                    className="ghost perm-action"
                    onClick={() => isTauri() && void invoke("open_permission_panel", { kind: "accessibility" })}
                  >
                    {tr("permOpen")}
                  </button>
                }
              />
              <PermRow
                locale={locale}
                title={tr("inputMonitoring")}
                why={tr("permWhyInput")}
                state={perms?.input_monitoring ?? "unknown"}
                actions={
                  <button
                    type="button"
                    className="ghost perm-action"
                    onClick={() => isTauri() && void invoke("open_permission_panel", { kind: "input_monitoring" })}
                  >
                    {tr("permOpen")}
                  </button>
                }
              />
              <button type="button" className="primary perm-btn" onClick={onRefreshPerms}>
                {tr("permRefresh")}
              </button>
            </section>
          )}

          {tab === "about" && (
            <section className="card about">
              <Logo size={48} />
              <h2>RemoteX</h2>
              <p>{tr("aboutTagline")}</p>
              <p className="muted">{tr("aboutNote")}</p>
              <p className="muted">v0.2.2 · macOS / Windows</p>
              <a className="github-link" href="https://github.com/linux503/RemoteX" target="_blank" rel="noreferrer">
                GitHub
              </a>
            </section>
          )}
        </div>
      </div>
    </main>
  );
}

function PermRow({
  locale,
  title,
  why,
  state,
  actions,
}: {
  locale: Locale;
  title: string;
  why: string;
  state: string;
  actions: ReactNode;
}) {
  return (
    <div className="perm-row">
      <div className="perm-main">
        <span className={`dot ${permDotClass(state)}`} />
        <div>
          <strong>{title}</strong>
          <p className="muted">{why}</p>
        </div>
      </div>
      <div className="perm-side">
        <span className={`perm-badge ${state}`}>{permLabel(locale, state as "granted" | "denied" | "unknown")}</span>
        <div className="perm-actions">{actions}</div>
      </div>
    </div>
  );
}

function Toggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <div className="set-row">
      <span>{label}</span>
      <button
        type="button"
        role="switch"
        className={`switch ${checked ? "on" : ""}`}
        aria-checked={checked}
        aria-label={label}
        onClick={() => onChange(!checked)}
      />
    </div>
  );
}
