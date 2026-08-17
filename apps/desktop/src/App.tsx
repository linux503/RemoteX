import { useEffect, useMemo, useState } from "react";
import { mockSnapshot, type AppSettings, type Snapshot } from "./types";

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

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export default function App() {
  const [snap, setSnap] = useState<Snapshot>(mockSnapshot);
  const [view, setView] = useState<"home" | "settings">("home");
  const [hidePassword, setHidePassword] = useState(false);
  const [connectId, setConnectId] = useState("");
  const [connectPassword, setConnectPassword] = useState("");
  const [passwordStep, setPasswordStep] = useState(false);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState("");
  const [qualityOpen, setQualityOpen] = useState(false);
  const [displayOpen, setDisplayOpen] = useState(false);
  const [connectStep, setConnectStep] = useState(0);

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
    if (isTauri() || snap.phase !== "connected") return;
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

  const run = async (fn: () => Promise<unknown>) => {
    try {
      setError("");
      await fn();
    } catch (err) {
      const message =
        typeof err === "object" && err && "message" in err
          ? String((err as { message: string }).message)
          : String(err);
      setError(message);
    }
  };

  const startConnect = () => {
    const id = connectId.replace(/\D/g, "");
    if (id.length !== 9) {
      setError("Enter a 9-digit device ID");
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
    const map: Record<string, string> = {
      smooth: "Smooth",
      balanced: "Balanced",
      high: "High Quality",
      original: "Original",
    };
    return map[snap.settings.quality] ?? "Balanced";
  }, [snap.settings.quality]);

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
            Display
          </button>
          <button className="ghost" onClick={() => setQualityOpen((v) => !v)}>
            {qualityLabel}
          </button>
          <button className="ghost">Keys</button>
          <button className="ghost">Files</button>
          <button className="danger" onClick={() => run(() => isTauri() ? invoke("hangup") : Promise.resolve(setSnap({ ...snap, phase: "idle", session: null })))}>
            End
          </button>
          {displayOpen && (
            <div className="popover">
              <p className="label">Display</p>
              <label className="choice"><input type="radio" defaultChecked readOnly /> Display 1</label>
              <label className="choice"><input type="radio" disabled /> Display 2</label>
              <button className="ghost">Show All</button>
            </div>
          )}
          {qualityOpen && (
            <div className="popover right">
              <p className="label">Display Quality</p>
              {(["smooth", "balanced", "high", "original"] as const).map((item) => (
                <label className="choice" key={item}>
                  <input
                    type="radio"
                    checked={snap.settings.quality === item}
                    onChange={() => updateSettings({ quality: item })}
                  />
                  {item === "high" ? "High Quality" : item[0].toUpperCase() + item.slice(1)}
                </label>
              ))}
              <p className="label">Resolution</p>
              <label className="choice"><input type="radio" defaultChecked readOnly /> Auto</label>
            </div>
          )}
        </div>
        <div className="session-stats">
          <div className={`stat ${latencyTone(snap.session.rtt_ms)}`}>
            <span>Latency</span>
            <strong>{snap.session.rtt_ms ? `${snap.session.rtt_ms} ms` : "—"}</strong>
          </div>
          <div className="stat good">
            <span>Download</span>
            <strong>{formatMbps(snap.session.down_kbps)}</strong>
          </div>
          <div className="stat">
            <span>Upload</span>
            <strong>{formatMbps(snap.session.up_kbps)}</strong>
          </div>
          <div className="stat">
            <span>Path</span>
            <strong>{snap.session.path === "p2p" ? "Direct P2P" : "Relay"}</strong>
          </div>
        </div>
        <div className="desktop-stage">
          <div className="desktop-canvas">
            <p>Remote Desktop</p>
            <span>{snap.session.peer_os === "macos" ? "macOS" : "Windows"} screen</span>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="app">
      <Titlebar onSettings={() => setView(view === "settings" ? "home" : "settings")} />

      {view === "settings" ? (
        <Settings
          snap={snap}
          onBack={() => setView("home")}
          onSettings={updateSettings}
          onPermanentPassword={(password) => {
            if (isTauri()) void invoke("set_permanent_password", { password });
          }}
        />
      ) : (
        <main className="home">
          <header className="hero">
            <Logo size={28} />
            <div>
              <h1>Remote Desktop</h1>
              <p>Connect securely from anywhere.</p>
            </div>
          </header>

          <section className="card device-card">
            <p className="eyebrow">This device</p>
            <div className="id-row">
              <h2>{snap.formatted_id}</h2>
              <button className="icon-btn" onClick={() => copy("ID", snap.formatted_id)} title="Copy ID">
                {copied === "ID" ? "✓" : "⧉"}
              </button>
            </div>
            <div className={`status ${snap.ready ? "ready" : "offline"}`}>
              <span className="dot" />
              {snap.ready ? "Ready" : "Connecting to network…"}
              {snap.ready && snap.rtt_ms > 0 ? ` · ${snap.rtt_ms}ms` : ""}
            </div>
            <p className="label">Temporary Password</p>
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
                  title="Refresh password"
                >
                  ↻
                </button>
              </div>
            </div>
          </section>

          <section className="connect">
            <p className="label">Connect to Remote Device</p>
            <div className="connect-row">
              <input
                value={connectId}
                onChange={(e) => setConnectId(formatIdInput(e.target.value))}
                placeholder="Enter Device ID"
                inputMode="numeric"
                onKeyDown={(e) => e.key === "Enter" && startConnect()}
              />
              <button className="primary" onClick={startConnect}>
                Connect
              </button>
            </div>
            {error && <p className="error">{error}</p>}
          </section>

          {snap.recents.length > 0 && (
            <section className="recents">
              <p className="label">Recent</p>
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

          <footer>RemoteX v0.2</footer>
        </main>
      )}

      {passwordStep && (
        <div className="overlay">
          <div className="modal">
            <p className="eyebrow">Connect</p>
            <h3>{connectId}</h3>
            <p className="muted">Enter the temporary password from the other device.</p>
            <input
              autoFocus
              value={connectPassword}
              onChange={(e) => setConnectPassword(e.target.value.toUpperCase())}
              placeholder="Password"
              onKeyDown={(e) => e.key === "Enter" && void confirmConnect()}
            />
            {error && <p className="error">{error}</p>}
            <div className="modal-actions">
              <button onClick={() => setPasswordStep(false)}>Cancel</button>
              <button className="primary" onClick={() => void confirmConnect()}>
                Continue
              </button>
            </div>
          </div>
        </div>
      )}

      {snap.phase === "connecting" && (
        <div className="overlay connecting">
          <Logo size={36} />
          <p className="eyebrow">RemoteX</p>
          <h2>Connecting…</h2>
          <p>{snap.session?.peer_name ?? "Remote device"}</p>
          <div className="link-graph">
            <span>You</span>
            <i />
            <b />
            <i />
            <span>{snap.session?.peer_name ?? "Peer"}</span>
          </div>
          <ul className="steps">
            {["Finding device", "Secure handshake", "P2P connection", "Starting video"].map((label, index) => (
              <li key={label} className={connectStep > index ? "done" : connectStep === index ? "active" : ""}>
                <span>{connectStep > index ? "✓" : "●"}</span>
                {label}
              </li>
            ))}
          </ul>
        </div>
      )}

      {snap.phase === "incoming" && snap.incoming && (
        <div className="overlay">
          <div className="modal">
            <p className="eyebrow">Remote Connection Request</p>
            <h3>{snap.incoming.from_name}</h3>
            <p>wants to control this device.</p>
            <div className="modal-actions">
              <button onClick={() => run(() => isTauri() ? invoke("decline") : Promise.resolve(setSnap({ ...snap, phase: "idle", incoming: null })))}>
                Decline
              </button>
              <button className="primary" onClick={() => run(() => isTauri() ? invoke("accept") : Promise.resolve())}>
                Accept
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
}: {
  onSettings: () => void;
  compact?: boolean;
  subtitle?: string;
}) {
  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="titlebar-label">
        <Logo size={16} />
        <span>RemoteX</span>
        {subtitle && <span className="muted">{subtitle}</span>}
      </div>
      {!compact && (
        <button className="icon-btn" onClick={onSettings} title="Settings">
          ⚙
        </button>
      )}
    </div>
  );
}

function Logo({ size }: { size: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" className="logo" aria-hidden>
      <rect x="1" y="1" width="30" height="30" rx="8" fill="currentColor" opacity="0.08" />
      <path
        d="M9 9 L23 23 M23 9 L9 23"
        stroke="currentColor"
        strokeWidth="2.6"
        strokeLinecap="round"
      />
    </svg>
  );
}

function Settings({
  snap,
  onBack,
  onSettings,
  onPermanentPassword,
}: {
  snap: Snapshot;
  onBack: () => void;
  onSettings: (patch: Partial<AppSettings>) => void;
  onPermanentPassword: (password: string) => void;
}) {
  const [tab, setTab] = useState("General");
  const [permanent, setPermanent] = useState("");
  const tabs = ["General", "Connection", "Security", "Display", "Permissions", "About"];

  return (
    <main className="settings">
      <button className="back" onClick={onBack}>← Back</button>
      <h1>Settings</h1>
      <nav className="tabs">
        {tabs.map((item) => (
          <button key={item} className={tab === item ? "active" : ""} onClick={() => setTab(item)}>
            {item}
          </button>
        ))}
      </nav>

      {tab === "General" && (
        <section className="card">
          <Toggle label="Start RemoteX at startup" checked={snap.settings.start_at_login} onChange={(v) => onSettings({ start_at_login: v })} />
          <Toggle label="Minimize to tray" checked={snap.settings.minimize_to_tray} onChange={(v) => onSettings({ minimize_to_tray: v })} />
          <Toggle label="Automatic updates" checked={snap.settings.auto_update} onChange={(v) => onSettings({ auto_update: v })} />
          <label className="field">
            Theme
            <select value={snap.settings.theme} onChange={(e) => onSettings({ theme: e.target.value })}>
              <option value="system">System</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </label>
        </section>
      )}

      {tab === "Connection" && (
        <section className="card">
          <Toggle label="Prefer P2P" checked={snap.settings.p2p_preferred} onChange={(v) => onSettings({ p2p_preferred: v })} />
          <Toggle label="Hardware encoding" checked={snap.settings.hardware_encode} onChange={(v) => onSettings({ hardware_encode: v })} />
          <label className="field">
            Signaling server
            <input
              value={snap.settings.signaling_url}
              onChange={(e) => onSettings({ signaling_url: e.target.value })}
            />
          </label>
          <p className="muted">Connection: {snap.ready ? "Online" : "Offline"}</p>
        </section>
      )}

      {tab === "Security" && (
        <section className="card">
          <Toggle label="Unattended Access" checked={snap.settings.unattended} onChange={(v) => onSettings({ unattended: v })} />
          <label className="field">
            Permanent Password
            <input
              type="password"
              value={permanent}
              placeholder={snap.has_permanent_password ? "••••••••" : "Set a password"}
              onChange={(e) => setPermanent(e.target.value)}
              onBlur={() => permanent && onPermanentPassword(permanent)}
            />
          </label>
          <Toggle label="Ask before connecting" checked={snap.settings.require_confirm} onChange={(v) => onSettings({ require_confirm: v })} />
          <Toggle label="Allow clipboard" checked={snap.settings.allow_clipboard} onChange={(v) => onSettings({ allow_clipboard: v })} />
          <Toggle label="Allow file transfer" checked={snap.settings.allow_file_transfer} onChange={(v) => onSettings({ allow_file_transfer: v })} />
          <Toggle label="Lock computer after session" checked={snap.settings.lock_after_session} onChange={(v) => onSettings({ lock_after_session: v })} />
        </section>
      )}

      {tab === "Display" && (
        <section className="card">
          <label className="field">
            Quality
            <select value={snap.settings.quality} onChange={(e) => onSettings({ quality: e.target.value })}>
              <option value="smooth">Smooth</option>
              <option value="balanced">Balanced</option>
              <option value="high">High Quality</option>
              <option value="original">Original</option>
            </select>
          </label>
          <label className="field">
            FPS
            <select value={snap.settings.fps} onChange={(e) => onSettings({ fps: Number(e.target.value) })}>
              <option value={30}>30</option>
              <option value={60}>60</option>
              <option value={120}>120</option>
            </select>
          </label>
        </section>
      )}

      {tab === "Permissions" && (
        <section className="card permissions">
          <p className="label">System Permissions</p>
          <div className="perm"><span className="dot ready" /> Screen Recording</div>
          <div className="perm"><span className="dot ready" /> Accessibility</div>
          <div className="perm"><span className="dot offline" /> Input Monitoring</div>
          <button
            className="primary"
            onClick={() => {
              if (isTauri()) void invoke("open_permission_settings");
            }}
          >
            Open Settings
          </button>
        </section>
      )}

      {tab === "About" && (
        <section className="card about">
          <Logo size={40} />
          <h2>RemoteX</h2>
          <p>Fast Remote Desktop</p>
          <p className="muted">No account. No setup. Just connect.</p>
          <p className="muted">v0.2.0</p>
        </section>
      )}
    </main>
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
    <label className="toggle">
      <span>{label}</span>
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
    </label>
  );
}
