import { useEffect, useRef, useState, type ReactNode } from "react";
import { previewSnapshot, type AppSettings, type PermissionsStatus, type Snapshot } from "./types";
import { resolveLocale, t, translateError, type Locale, type MessageKey } from "./i18n";

const isTauri = () => "__TAURI_INTERNALS__" in window;

function latencyTone(ms: number) {
  if (!ms) return "";
  if (ms < 45) return "good";
  if (ms < 90) return "ok";
  return "bad";
}

const mockPermissions = (): PermissionsStatus => {
  const scene = typeof window !== "undefined" ? new URLSearchParams(window.location.search).get("scene") : null;
  const denied = scene === "permissions";
  return {
    screen_recording: denied ? "denied" : "granted",
    accessibility: denied ? "denied" : "granted",
    input_monitoring: denied ? "unknown" : "granted",
    platform: "macos",
    all_granted: !denied,
  };
};

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

type PermKind = "screen_recording" | "accessibility" | "input_monitoring";

const PERM_ORDER: PermKind[] = ["screen_recording", "accessibility", "input_monitoring"];

function permItems(perms: PermissionsStatus) {
  return [
    { kind: "screen_recording" as const, state: perms.screen_recording, titleKey: "screenRecording" as const, whyKey: "permWhyScreen" as const },
    { kind: "accessibility" as const, state: perms.accessibility, titleKey: "accessibility" as const, whyKey: "permWhyAccessibility" as const },
    { kind: "input_monitoring" as const, state: perms.input_monitoring, titleKey: "inputMonitoring" as const, whyKey: "permWhyInput" as const },
  ];
}

function nextPermKind(perms: PermissionsStatus): PermKind | null {
  for (const kind of PERM_ORDER) {
    const item = permItems(perms).find((p) => p.kind === kind);
    if (item && item.state !== "granted") return kind;
  }
  return null;
}

function grantedPermCount(perms: PermissionsStatus) {
  return permItems(perms).filter((p) => p.state === "granted").length;
}

async function openPermKind(kind: PermKind) {
  if (!isTauri()) return;
  if (kind === "screen_recording") {
    await invoke("request_screen_recording");
  }
  await invoke("open_permission_panel", { kind });
}

async function guidePermission(kind: PermKind, onUpdate: (perms: PermissionsStatus | null) => void) {
  await openPermKind(kind);
  for (let i = 0; i < 18; i += 1) {
    await new Promise((resolve) => window.setTimeout(resolve, 1200));
    const latest = await fetchPermissions();
    onUpdate(latest);
    if (!latest) continue;
    if (latest[kind] === "granted" || nextPermKind(latest) !== kind) return;
  }
}

function modifierBits(event: { shiftKey: boolean; ctrlKey: boolean; altKey: boolean; metaKey: boolean }) {
  let bits = 0;
  if (event.shiftKey) bits |= 1;
  if (event.ctrlKey) bits |= 2;
  if (event.altKey) bits |= 4;
  if (event.metaKey) bits |= 8;
  return bits;
}

function RemoteDesktop({ locale, isHost }: { locale: Locale; isHost: boolean }) {
  const [waiting, setWaiting] = useState(true);
  const stageRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const lastData = useRef("");
  const pendingMove = useRef<{ x: number; y: number } | null>(null);
  const sendingMove = useRef(false);

  const drawFrame = async (data: string) => {
    if (!data || data === lastData.current) return;
    lastData.current = data;
    const canvas = canvasRef.current;
    if (!canvas) return;
    try {
      const raw = atob(data);
      const bytes = new Uint8Array(raw.length);
      for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
      const blob = new Blob([bytes], { type: "image/jpeg" });
      let bmp: ImageBitmap;
      try {
        bmp = await createImageBitmap(blob, {
          colorSpaceConversion: "none",
          premultiplyAlpha: "none",
        });
      } catch {
        bmp = await createImageBitmap(blob);
      }
      if (canvas.width !== bmp.width || canvas.height !== bmp.height) {
        canvas.width = bmp.width;
        canvas.height = bmp.height;
      }
      const stage = stageRef.current;
      if (stage) {
        const scale = Math.min(stage.clientWidth / bmp.width, stage.clientHeight / bmp.height);
        canvas.style.width = `${Math.max(1, Math.round(bmp.width * scale))}px`;
        canvas.style.height = `${Math.max(1, Math.round(bmp.height * scale))}px`;
      }
      const ctx = canvas.getContext("2d", { alpha: false });
      if (!ctx) {
        bmp.close();
        return;
      }
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(bmp, 0, 0);
      bmp.close();
      setWaiting(false);
    } catch {
      /* keep last frame */
    }
  };

  useEffect(() => {
    if (!isTauri()) return;
    const timer = window.setInterval(() => {
      void invoke<{ data: string; width: number; height: number } | null>("latest_frame")
        .then((frame) => {
          if (frame?.data) void drawFrame(frame.data);
        })
        .catch(() => {});
    }, 32);
    return () => window.clearInterval(timer);
  }, []);

  const norm = (clientX: number, clientY: number) => {
    const el = canvasRef.current;
    if (!el) return null;
    const rect = el.getBoundingClientRect();
    if (!rect.width || !rect.height) return null;
    const x = (clientX - rect.left) / rect.width;
    const y = (clientY - rect.top) / rect.height;
    if (x < 0 || y < 0 || x > 1 || y > 1) return null;
    return { x, y };
  };

  const sendInput = (event: Record<string, unknown>) => {
    if (!isTauri() || isHost) return;
    void invoke("session_input", { event });
  };

  const flushMove = () => {
    if (sendingMove.current || !pendingMove.current) return;
    const p = pendingMove.current;
    pendingMove.current = null;
    sendingMove.current = true;
    void invoke("session_input", { event: { type: "mouse_move", x: p.x, y: p.y } }).finally(() => {
      sendingMove.current = false;
      if (pendingMove.current) requestAnimationFrame(flushMove);
    });
  };

  const showPreview = previewScene === "session";

  return (
    <div className="desktop-stage">
      <div
        ref={stageRef}
        className="desktop-canvas remote"
        tabIndex={0}
        onContextMenu={(e) => e.preventDefault()}
        onMouseEnter={() => canvasRef.current?.focus()}
        onMouseMove={(e) => {
          const p = norm(e.clientX, e.clientY);
          if (!p) return;
          pendingMove.current = p;
          flushMove();
        }}
        onMouseDown={(e) => {
          e.preventDefault();
          canvasRef.current?.focus();
          const p = norm(e.clientX, e.clientY);
          if (!p) return;
          sendInput({ type: "mouse_down", button: e.button, x: p.x, y: p.y });
        }}
        onMouseUp={(e) => {
          const p = norm(e.clientX, e.clientY);
          if (!p) return;
          sendInput({ type: "mouse_up", button: e.button, x: p.x, y: p.y });
        }}
        onWheel={(e) => {
          e.preventDefault();
          sendInput({ type: "wheel", dx: e.deltaX, dy: e.deltaY });
        }}
        onKeyDown={(e) => {
          e.preventDefault();
          if (e.repeat) return;
          sendInput({ type: "key_down", key: e.code, modifiers: modifierBits(e) });
        }}
        onKeyUp={(e) => {
          e.preventDefault();
          sendInput({ type: "key_up", key: e.code, modifiers: modifierBits(e) });
        }}
      >
        <canvas ref={canvasRef} className={`remote-canvas${waiting ? "" : " in"}`} />
        {showPreview && waiting && <PreviewRemoteScreen locale={locale} />}
        {waiting && !showPreview && (
          <div className="desktop-wait">
            <span className="wait-spin" aria-hidden />
            <p>{t(locale, "remoteDesktop")}</p>
            <span>
              {isHost
                ? t(locale, "sharingScreen")
                : t(locale, "waitingScreen")}
            </span>
          </div>
        )}
      </div>
    </div>
  );
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
  const [hideSessionToolbar, setHideSessionToolbar] = useState(false);
  const [connectId, setConnectId] = useState("");
  const [connectPassword, setConnectPassword] = useState("");
  const [passwordStep, setPasswordStep] = useState(false);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState("");
  const [chromeVisible, setChromeVisible] = useState(true);
  const chromeTimer = useRef<number | null>(null);
  const [connectStep, setConnectStep] = useState(0);
  const [settingsTab, setSettingsTab] = useState(
    previewScene === "settings" || previewScene === "permissions" ? (previewScene === "permissions" ? "permissions" : "general") : "general",
  );
  const [perms, setPerms] = useState<PermissionsStatus | null>(null);
  const [toast, setToast] = useState("");

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
      root.dataset.preview = previewScene;
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
      setConnectStep(3);
      return;
    }
    if (snap.phase !== "connecting") {
      setConnectStep(0);
      return;
    }
    setConnectStep(1);
    const timers = [650, 650, 650].map((ms, index) =>
      window.setTimeout(() => setConnectStep(index + 2), ms * (index + 1)),
    );
    timers.push(window.setTimeout(() => setConnectStep(4), 2600));
    return () => timers.forEach(clearTimeout);
  }, [snap.phase, previewScene]);

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
  const activeLineLabel =
    snap.settings.signaling_line === "auto"
      ? snap.active_line === "2"
        ? tr("line2")
        : tr("line1")
      : snap.settings.signaling_line === "2"
        ? tr("line2")
        : tr("line1");

  useEffect(() => {
    if (!isTauri()) return;
    let unlistenToast: (() => void) | undefined;
    let unlistenFile: (() => void) | undefined;
    (async () => {
      const { listen } = await import("@tauri-apps/api/event");
      unlistenToast = await listen<string>("toast", (event) => {
        setToast(event.payload);
        window.setTimeout(() => setToast(""), 3200);
      });
      unlistenFile = await listen<{ name: string; path: string }>("file-received", (event) => {
        setToast(t(locale, "fileReceived").replace("{name}", event.payload.name));
        window.setTimeout(() => setToast(""), 4200);
      });
    })();
    return () => {
      unlistenToast?.();
      unlistenFile?.();
    };
  }, [locale]);

  useEffect(() => {
    if (snap.last_error) setError(translateError(locale, snap.last_error));
  }, [snap.last_error, locale]);

  useEffect(() => {
    // Leaving connected state should restore the default top toolbar UI.
    if (snap.phase !== "connected") setHideSessionToolbar(false);
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

  const copyBoth = async () => {
    const text =
      locale === "zh"
        ? `设备码：${snap.formatted_id}\n密码：${hidePassword ? snap.temp_password : snap.formatted_password}`
        : `Device ID: ${snap.formatted_id}\nPassword: ${hidePassword ? snap.temp_password : snap.formatted_password}`;
    await navigator.clipboard.writeText(text);
    setCopied("both");
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
            quality: "high",
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

  const bumpChrome = () => {
    setChromeVisible(true);
    if (previewScene) return;
    if (chromeTimer.current) window.clearTimeout(chromeTimer.current);
    chromeTimer.current = window.setTimeout(() => setChromeVisible(false), 2800);
  };

  useEffect(() => {
    if (snap.phase !== "connected") return;
    bumpChrome();
    return () => {
      if (chromeTimer.current) window.clearTimeout(chromeTimer.current);
    };
  }, [snap.phase]);

  const sessionQuality = snap.session?.quality === "original" ? "high" : (snap.session?.quality || snap.settings.quality);

  const applyQuality = (quality: string) => {
    setSnap((prev) => ({
      ...prev,
      settings: { ...prev.settings, quality },
      session: prev.session ? { ...prev.session, quality } : prev.session,
    }));
    if (isTauri()) {
      void invoke("set_session_quality", { quality });
    }
  };

  if (snap.phase === "connected" && snap.session) {
    return (
      <div className="app session-app">
        {previewScene !== "session" && (
          <Titlebar onSettings={() => setView("settings")} compact />
        )}
        <div
          className="session-stage"
          onMouseMove={bumpChrome}
          onPointerDown={bumpChrome}
        >
          <RemoteDesktop locale={locale} isHost={snap.is_host} />
          {!hideSessionToolbar ? (
            <div className={`session-chrome${chromeVisible ? " show" : ""}`}>
              <div className="session-toolbar">
                <div className="session-peer">
                  <span className="live-dot" aria-hidden />
                  <strong>{snap.session.peer_name}</strong>
                  <span className={`pill ${snap.session.path === "p2p" ? "good" : ""}`}>
                    {snap.session.path === "p2p" ? tr("directP2p") : tr("relay")}
                  </span>
                  <span className={`pill ${latencyTone(snap.session.rtt_ms)}`}>
                    {snap.session.rtt_ms || "—"} ms
                  </span>
                </div>
                <div className="quality-switch" role="radiogroup" aria-label={tr("displayQuality")}>
                  {([
                    ["smooth", "qualitySmooth"],
                    ["balanced", "qualityBalanced"],
                    ["high", "qualityHigh"],
                  ] as const).map(([value, key]) => (
                    <button
                      key={value}
                      type="button"
                      role="radio"
                      aria-checked={sessionQuality === value}
                      className={sessionQuality === value ? "on" : ""}
                      onClick={() => applyQuality(value)}
                    >
                      {t(locale, key)}
                    </button>
                  ))}
                </div>
                {snap.settings.allow_file_transfer && (
                  <button
                    type="button"
                    className="ghost session-send-file"
                    onClick={() =>
                      run(async () => {
                        setToast(tr("fileSending"));
                        await invoke("session_pick_send_file");
                        window.setTimeout(() => setToast(""), 1800);
                      })
                    }
                  >
                    {tr("sendFile")}
                  </button>
                )}
                <button
                  type="button"
                  className="ghost session-hide-toolbar"
                  onClick={() => {
                    setChromeVisible(false);
                    setHideSessionToolbar(true);
                  }}
                >
                  {tr("hideToolbar")}
                </button>
                <button
                  className="danger"
                  onClick={() =>
                    run(() =>
                      isTauri()
                        ? invoke("hangup")
                        : Promise.resolve(setSnap({ ...snap, phase: "idle", session: null })),
                    )
                  }
                >
                  {tr("end")}
                </button>
              </div>
            </div>
          ) : (
            <button
              type="button"
              className="ghost session-toolbar-show"
              onClick={() => {
                setHideSessionToolbar(false);
                setChromeVisible(true);
              }}
            >
              {tr("showToolbar")}
            </button>
          )}
        </div>
        {toast && (
          <div className="toast" role="status">
            {toast}
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="app">
      <Titlebar
        onSettings={() => setView(view === "settings" ? "home" : "settings")}
        settingsLabel={tr("settings")}
        needsPermissions={!!(perms && perms.platform === "macos" && !perms.all_granted)}
      />

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
            if (!isTauri()) return;
            void run(async () => {
              await invoke("set_permanent_password", { password });
            });
          }}
        />
      ) : (
        <main className="home">
          {perms && perms.platform === "macos" && !perms.all_granted && (
            <PermHomeBanner
              locale={locale}
              perms={perms}
              onFixNext={() => {
                const next = nextPermKind(perms);
                if (next) void openPermKind(next).then(() => fetchPermissions().then(setPerms));
              }}
              onViewAll={() => {
                setSettingsTab("permissions");
                setView("settings");
              }}
            />
          )}
          <header className="hero hero-compact">
            <div>
              <h1>{tr("remoteDesktop")}</h1>
              <p>{tr("tagline")}</p>
            </div>
          </header>

          <section className="home-grid">
            <section className="card device-card">
              <div className="device-head">
                <div>
                  <p className="eyebrow">{tr("thisDevice")}</p>
                  <div className="device-title-row">
                    <h3 className="device-name">{snap.name}</h3>
                    <span className={`platform-tag ${platformKind(snap.os)}`}>{osLabel(locale, snap.os)}</span>
                  </div>
                </div>
                <PlatformBadge os={snap.os} label={osLabel(locale, snap.os)} />
              </div>
              <div className="device-main-grid">
                <div className="device-primary">
                  <div className="id-row">
                    <h2>{snap.formatted_id}</h2>
                    <button
                      type="button"
                      className={`copy-chip ${copied === "ID" ? "copied" : ""}`}
                      onClick={() => copy("ID", snap.formatted_id)}
                    >
                      {copied === "ID" ? tr("copied") : tr("copyId")}
                    </button>
                  </div>
                  <div className="device-metrics">
                    <span className={`metric-pill ${snap.ready ? "ready" : "offline"}`}>
                      <span className="dot" />
                      {snap.ready ? tr("ready") : tr("connectingNetwork")}
                    </span>
                    {snap.ready && <span className="metric-pill">{activeLineLabel}</span>}
                    {snap.ready && snap.rtt_ms > 0 && (
                      <span className={`metric-pill latency ${latencyLabel(snap.rtt_ms)}`}>{snap.rtt_ms} ms</span>
                    )}
                  </div>
                  <p className="credential-kicker">{tr("shareCredentials")}</p>
                </div>
                <div className="credential-panel compact">
                  <div className="credential-summary">
                    <div className="credential-box">
                      <p className="label">{tr("tempPassword")}</p>
                      <div className="password-row">
                        <strong>{hidePassword ? "• • • • • •" : snap.formatted_password}</strong>
                        <div className="row-actions">
                          <button
                            type="button"
                            className={`copy-chip tiny ${copied === "password" ? "copied" : ""}`}
                            onClick={() => copy("password", snap.temp_password)}
                          >
                            {copied === "password" ? "✓" : tr("copyPassword")}
                          </button>
                          <button
                            type="button"
                            className="icon-btn"
                            onClick={() => setHidePassword((v) => !v)}
                            aria-label="Toggle password"
                          >
                            {hidePassword ? "○" : "●"}
                          </button>
                          <button
                            type="button"
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
                    </div>
                    <button
                      type="button"
                      className={`copy-all compact ${copied === "both" ? "copied" : ""}`}
                      onClick={copyBoth}
                    >
                      {copied === "both" ? tr("copied") : tr("copyBoth")}
                    </button>
                  </div>
                </div>
              </div>
            </section>

            <aside className="home-side">
              <section className="card connect-card">
                <p className="eyebrow">{tr("quickConnect")}</p>
                <p className="label connect-label">{tr("connectTo")}</p>
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

              <section className="card network-card">
                <p className="eyebrow">{tr("connection")}</p>
                <div className="network-stats">
                  <div className="network-stat">
                    <span>{tr("networkStatus")}</span>
                    <strong>{snap.ready ? tr("online") : tr("offline")}</strong>
                  </div>
                  <div className="network-stat">
                    <span>{tr("activeLine")}</span>
                    <strong>{activeLineLabel}</strong>
                  </div>
                  <div className="network-stat">
                    <span>{tr("signalLatency")}</span>
                    <strong>{snap.rtt_ms > 0 ? `${snap.rtt_ms} ms` : "—"}</strong>
                  </div>
                </div>
              </section>
            </aside>
          </section>

          {(snap.nearby?.length > 0 || snap.recents.length > 0) && (
            <section className="home-lists">
              {snap.nearby && snap.nearby.length > 0 && (
                <section className="recents card">
                  <p className="label">{tr("nearby")}</p>
                  {snap.nearby.map((item) => (
                    <DeviceListItem
                      key={item.id}
                      locale={locale}
                      name={item.name}
                      os={item.os}
                      deviceId={item.id}
                      trailing={<span className="dot ready" />}
                      onClick={() => {
                        setConnectId(item.id.replace(/(\d{3})(\d{3})(\d{3})/, "$1 $2 $3"));
                        setPasswordStep(true);
                      }}
                    />
                  ))}
                </section>
              )}

              {snap.recents.length > 0 && (
                <section className="recents card">
                  <p className="label">{tr("recent")}</p>
                  {snap.recents.map((item) => (
                    <DeviceListItem
                      key={item.id}
                      locale={locale}
                      name={item.name}
                      os={item.os}
                      deviceId={item.id}
                      favorite={item.favorite}
                      trailing={<span className="chevron">→</span>}
                      onClick={() => {
                        setConnectId(item.id.replace(/(\d{3})(\d{3})(\d{3})/, "$1 $2 $3"));
                        setPasswordStep(true);
                      }}
                    />
                  ))}
                </section>
              )}
            </section>
          )}

          <footer>RemoteX v2.0.1</footer>
        </main>
      )}

      {passwordStep && (
        <ConnectPasswordModal
          locale={locale}
          deviceId={connectId}
          password={connectPassword}
          error={error}
          onPasswordChange={setConnectPassword}
          onCancel={() => setPasswordStep(false)}
          onConfirm={() => void confirmConnect()}
        />
      )}

      {snap.phase === "connecting" && (
        <ConnectFlowOverlay
          locale={locale}
          peerName={snap.session?.peer_name ?? tr("remoteDevice")}
          connectStep={connectStep}
          onCancel={() =>
            run(() =>
              isTauri()
                ? invoke("hangup")
                : Promise.resolve(setSnap({ ...snap, phase: "idle", session: null })),
            )
          }
        />
      )}

      {snap.phase === "incoming" && snap.incoming && (
        <IncomingConnectModal
          locale={locale}
          fromName={snap.incoming.from_name}
          fromOs={snap.incoming.from_os}
          onDecline={() =>
            run(() =>
              isTauri()
                ? invoke("decline")
                : Promise.resolve(setSnap({ ...snap, phase: "idle", incoming: null })),
            )
          }
          onAccept={() =>
            run(async () => {
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
            })
          }
        />
      )}
      {toast && (
        <div className="toast" role="status">
          {toast}
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
  needsPermissions,
}: {
  onSettings: () => void;
  compact?: boolean;
  subtitle?: string;
  settingsLabel?: string;
  needsPermissions?: boolean;
}) {
  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="titlebar-label">
        <span>RemoteX</span>
        {subtitle && <span className="muted">{subtitle}</span>}
      </div>
      {!compact && (
        <button
          type="button"
          className={`settings-btn${needsPermissions ? " needs-perm" : ""}`}
          onClick={onSettings}
          title={settingsLabel ?? "Settings"}
        >
          <span className="settings-btn-icon" aria-hidden>⚙</span>
          <span>{settingsLabel ?? "Settings"}</span>
          {needsPermissions && <span className="settings-alert" aria-hidden />}
        </button>
      )}
    </div>
  );
}

function PreviewRemoteScreen({ locale }: { locale: Locale }) {
  const zh = locale === "zh";
  return (
    <div className="preview-desktop" aria-hidden>
      <div className="preview-menubar">
        <b>{zh ? "访达" : "Finder"}</b>
        <span>{zh ? "文件" : "File"}</span>
        <span>{zh ? "编辑" : "Edit"}</span>
        <span>{zh ? "显示" : "View"}</span>
        <span>{zh ? "前往" : "Go"}</span>
        <em>{zh ? "周二 22:42" : "Tue 10:42 PM"}</em>
      </div>
      <div className="preview-icons">
        <div>
          <i />
          <span>{zh ? "文稿" : "Documents"}</span>
        </div>
        <div>
          <i />
          <span>{zh ? "下载" : "Downloads"}</span>
        </div>
        <div>
          <i />
          <span>{zh ? "项目" : "Projects"}</span>
        </div>
      </div>
      <article className="preview-win files">
        <header>
          <i /><i /><i />
          <strong>{zh ? "文稿" : "Documents"}</strong>
        </header>
        <ul>
          <li>{zh ? "季度报告.xlsx" : "Q3 Report.xlsx"}</li>
          <li>{zh ? "客户名单.csv" : "Clients.csv"}</li>
          <li>RemoteX.dmg</li>
          <li>{zh ? "设计稿.fig" : "Landing.fig"}</li>
        </ul>
      </article>
      <article className="preview-win notes">
        <header>
          <i /><i /><i />
          <strong>Notes</strong>
        </header>
        <p>{zh ? "远程已连接，画面同步中。" : "Remote session is live."}</p>
        <p>{zh ? "直连 P2P · 36 ms" : "Direct P2P · 36 ms"}</p>
      </article>
      <div className="preview-dock">
        <span /><span /><span /><span /><span />
      </div>
    </div>
  );
}

function Logo({ size }: { size: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" className="logo" aria-hidden>
      <rect x="1" y="1" width="30" height="30" rx="8" fill="#DC2626" />
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

function platformKind(os: string): "macos" | "windows" | "unknown" {
  if (os === "macos") return "macos";
  if (os === "windows") return "windows";
  return "unknown";
}

function OsGlyph({ os, className }: { os: string; className?: string }) {
  const kind = platformKind(os);
  if (kind === "macos") {
    return (
      <svg viewBox="0 0 24 24" className={className} aria-hidden>
        <path d="M16.67 12.46c-.03-3 2.45-4.43 2.56-4.5-1.4-2.03-3.58-2.3-4.34-2.33-1.85-.19-3.61 1.09-4.55 1.09-.94 0-2.39-1.06-3.93-1.03-2.02.03-3.89 1.18-4.93 2.98-2.1 3.64-.54 9.02 1.51 11.98 1 1.45 2.2 3.08 3.77 3.02 1.51-.06 2.08-.98 3.9-.98 1.82 0 2.33.98 3.94.95 1.63-.03 2.66-1.48 3.66-2.94 1.15-1.68 1.43-3.3 1.46-3.39-.04-.02-2.75-1.06-2.78-4.15zM14.34 4.08c.84-1.02 1.41-2.43 1.25-3.84-1.21.05-2.67.81-3.54 1.83-.78.9-1.46 2.34-1.28 3.72 1.35.1 2.73-.68 3.57-1.71z" />
      </svg>
    );
  }
  if (kind === "windows") {
    return (
      <svg viewBox="0 0 24 24" className={className} aria-hidden>
        <path d="M3 12.5V3.5l8 1.2v7.8H3zm9 0V4.3l9 1.3v7.9H12zM3 20.5v-7.8h8v9L3 20.5zm9-.9V13h9v8.3l-9-1.7z" />
      </svg>
    );
  }
  return (
    <svg viewBox="0 0 24 24" className={className} aria-hidden>
      <rect x="4" y="5" width="16" height="11" rx="1.5" />
      <path d="M8 19h8" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
    </svg>
  );
}

function PlatformBadge({ os, label }: { os: string; label: string }) {
  const kind = platformKind(os);
  return (
    <div className={`platform-badge ${kind}`} aria-label={label}>
      <OsGlyph os={os} className="os-glyph" />
    </div>
  );
}

function DeviceListItem({
  locale,
  name,
  os,
  deviceId,
  favorite,
  trailing,
  onClick,
}: {
  locale: Locale;
  name: string;
  os: string;
  deviceId: string;
  favorite?: boolean;
  trailing: ReactNode;
  onClick: () => void;
}) {
  const tr = (key: MessageKey) => t(locale, key);
  const kind = platformKind(os);
  const osLabel = kind === "macos" ? tr("osMac") : kind === "windows" ? tr("osWindows") : os;

  return (
    <button type="button" className="recent-item" onClick={onClick}>
      <PlatformBadge os={os} label={osLabel} />
      <div className="recent-main">
        <span className="recent-name">
          {favorite ? "★ " : ""}
          {name}
        </span>
        <span className="recent-meta">
          <span className={`platform-tag ${kind}`}>{osLabel}</span>
          <span className="recent-id">{deviceId.replace(/(\d{3})(\d{3})(\d{3})/, "$1 $2 $3")}</span>
        </span>
      </div>
      {trailing}
    </button>
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
  const tabs: { id: string; key: MessageKey; icon: string }[] = [
    { id: "general", key: "tabGeneral", icon: "◎" },
    { id: "connection", key: "tabConnection", icon: "⧉" },
    { id: "security", key: "tabSecurity", icon: "🔒" },
    { id: "display", key: "tabDisplay", icon: "◐" },
    { id: "permissions", key: "tabPermissions", icon: "✓" },
    { id: "about", key: "tabAbout", icon: "ℹ" },
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
              <span className="nav-icon" aria-hidden>{item.icon}</span>
              <span>{tr(item.key)}</span>
              {item.id === "permissions" && perms && perms.platform === "macos" && !perms.all_granted && (
                <span className="nav-badge">{3 - grantedPermCount(perms)}</span>
              )}
            </button>
          ))}
        </nav>
        <div className="settings-pane">
          {tab === "general" && (
            <section className="card settings-card">
              <header className="pane-hero">
                <p className="pane-title">{tr("tabGeneral")}</p>
                <p className="pane-sub">{tr("setGeneralSub")}</p>
              </header>
              <SettingsSection title={tr("tabGeneral")}>
                <SettingToggle
                  label={tr("startAtLogin")}
                  hint={tr("startAtLoginHint")}
                  checked={snap.settings.start_at_login}
                  onChange={(v) => onSettings({ start_at_login: v })}
                />
                <SettingToggle
                  label={tr("minimizeToTray")}
                  hint={tr("minimizeToTrayHint")}
                  checked={snap.settings.minimize_to_tray}
                  onChange={(v) => onSettings({ minimize_to_tray: v })}
                />
                <SettingToggle
                  label={tr("autoUpdate")}
                  hint={tr("autoUpdateHint")}
                  checked={snap.settings.auto_update}
                  onChange={(v) => onSettings({ auto_update: v })}
                />
              </SettingsSection>
              <SettingsSection title={tr("language")}>
                <SettingSelect
                  label={tr("language")}
                  value={snap.settings.language}
                  options={[
                    { value: "system", label: tr("languageSystem") },
                    { value: "en", label: tr("languageEn") },
                    { value: "zh", label: tr("languageZh") },
                  ]}
                  onChange={(v) => onSettings({ language: v })}
                />
                <SettingSelect
                  label={tr("theme")}
                  value={snap.settings.theme}
                  options={[
                    { value: "system", label: tr("themeSystem") },
                    { value: "light", label: tr("themeLight") },
                    { value: "dark", label: tr("themeDark") },
                  ]}
                  onChange={(v) => onSettings({ theme: v })}
                />
              </SettingsSection>
            </section>
          )}

          {tab === "connection" && (
            <section className="card settings-card">
              <header className="pane-hero">
                <p className="pane-title">{tr("tabConnection")}</p>
                <p className="pane-sub">{tr("setConnectionSub")}</p>
              </header>
              <ConnectionStatusPanel locale={locale} snap={snap} />
              <SettingsSection title={tr("signalingServer")}>
                <LinePickGrid
                  locale={locale}
                  active={snap.settings.signaling_line || "auto"}
                  activeLine={snap.active_line}
                  line1Ms={snap.line1_rtt_ms}
                  line2Ms={snap.line2_rtt_ms}
                  onPick={(line) => onSettings({ signaling_line: line, settings_rev: 1 })}
                />
              </SettingsSection>
              <SettingsSection title={tr("tabConnection")}>
                <SettingToggle
                  label={tr("preferP2p")}
                  hint={tr("preferP2pHint")}
                  checked={snap.settings.p2p_preferred}
                  onChange={(v) => onSettings({ p2p_preferred: v })}
                />
                <SettingToggle
                  label={tr("hardwareEncode")}
                  hint={tr("hardwareEncodeHint")}
                  checked={snap.settings.hardware_encode}
                  onChange={(v) => onSettings({ hardware_encode: v })}
                />
              </SettingsSection>
              {snap.lan_url && (
                <SettingsSection title={tr("lanHint")}>
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
                </SettingsSection>
              )}
            </section>
          )}

          {tab === "security" && (
            <section className="card settings-card">
              <header className="pane-hero">
                <p className="pane-title">{tr("tabSecurity")}</p>
                <p className="pane-sub">{tr("setSecuritySub")}</p>
              </header>
              <SettingsSection title={tr("unattended")}>
                <SettingToggle
                  label={tr("unattended")}
                  hint={tr("unattendedHint")}
                  checked={snap.settings.unattended}
                  onChange={(v) => onSettings({ unattended: v })}
                />
                <label className="set-item set-item-stack">
                  <div className="set-item-copy">
                    <strong>{tr("permanentPassword")}</strong>
                    <p className="set-item-hint">{tr("permanentPasswordHint")}</p>
                  </div>
                  <input
                    type="password"
                    value={permanent}
                    placeholder={snap.has_permanent_password ? "••••••••" : tr("setPassword")}
                    onChange={(e) => setPermanent(e.target.value)}
                    onBlur={() => permanent && onPermanentPassword(permanent)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        permanent && onPermanentPassword(permanent);
                        (e.target as HTMLInputElement).blur();
                      }
                    }}
                  />
                </label>
              </SettingsSection>
              <SettingsSection title={tr("tabSecurity")}>
                <SettingToggle
                  label={tr("askBeforeConnecting")}
                  hint={tr("askBeforeConnectingHint")}
                  checked={snap.settings.require_confirm}
                  onChange={(v) => onSettings({ require_confirm: v })}
                />
                <SettingToggle
                  label={tr("allowClipboard")}
                  hint={tr("allowClipboardHint")}
                  checked={snap.settings.allow_clipboard}
                  onChange={(v) => onSettings({ allow_clipboard: v })}
                />
                <SettingToggle
                  label={tr("allowFileTransfer")}
                  hint={tr("allowFileTransferHint")}
                  checked={snap.settings.allow_file_transfer}
                  onChange={(v) => onSettings({ allow_file_transfer: v })}
                />
                <SettingToggle
                  label={tr("lockAfterSession")}
                  hint={tr("lockAfterSessionHint")}
                  checked={snap.settings.lock_after_session}
                  onChange={(v) => onSettings({ lock_after_session: v })}
                />
              </SettingsSection>
            </section>
          )}

          {tab === "display" && (
            <section className="card settings-card">
              <header className="pane-hero">
                <p className="pane-title">{tr("tabDisplay")}</p>
                <p className="pane-sub">{tr("setDisplaySub")}</p>
              </header>
              <SettingsSection title={tr("defaultQuality")} subtitle={tr("defaultQualityHint")}>
                <QualityGrid
                  locale={locale}
                  value={snap.settings.quality}
                  onChange={(v) => onSettings({ quality: v })}
                />
              </SettingsSection>
              <SettingsSection title={tr("fps")}>
                <SettingSelect
                  label={tr("fps")}
                  hint={tr("fpsHint")}
                  value={snap.settings.fps}
                  options={[
                    { value: 30, label: "30" },
                    { value: 60, label: "60" },
                    { value: 120, label: "120" },
                  ]}
                  onChange={(v) => onSettings({ fps: Number(v) })}
                />
              </SettingsSection>
            </section>
          )}

          {tab === "permissions" && (
            <PermissionsPanel locale={locale} perms={perms} onRefresh={onRefreshPerms} />
          )}

          {tab === "about" && (
            <section className="card about settings-card">
              <Logo size={52} />
              <h2>RemoteX</h2>
              <p>{tr("aboutTagline")}</p>
              <p className="muted">{tr("aboutNote")}</p>
              <p className="about-version">v2.0.1 · macOS / Windows</p>
              <div className="about-features">
                <p className="eyebrow">{tr("aboutFeaturesTitle")}</p>
                <ul>
                  <li>{tr("featureNoAccount")}</li>
                  <li>{tr("featureAutoLine")}</li>
                  <li>{tr("featureUltraClear")}</li>
                  <li>{tr("featureCrossPlatform")}</li>
                </ul>
              </div>
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

function PermHomeBanner({
  locale,
  perms,
  onFixNext,
  onViewAll,
}: {
  locale: Locale;
  perms: PermissionsStatus;
  onFixNext: () => void;
  onViewAll: () => void;
}) {
  const tr = (key: MessageKey) => t(locale, key);
  const next = nextPermKind(perms);
  const done = grantedPermCount(perms);
  const nextItem = next ? permItems(perms).find((p) => p.kind === next) : null;

  return (
    <section className="perm-banner perm-banner-smart">
      <div className="perm-banner-copy">
        <p className="perm-banner-title">{tr("permBannerTitle")}</p>
        <p className="muted">{tr("permNeeded")}</p>
        <div className="perm-chip-row">
          {permItems(perms).map((item) => (
            <span key={item.kind} className={`perm-chip ${item.state === "granted" ? "ok" : item.kind === next ? "next" : ""}`}>
              {tr(item.titleKey)}
            </span>
          ))}
        </div>
        <div className="perm-progress">
          <span>{tr("permProgress").replace("{done}", String(done)).replace("{total}", "3")}</span>
          <div className="perm-progress-bar"><span style={{ width: `${(done / 3) * 100}%` }} /></div>
        </div>
      </div>
      <div className="perm-banner-actions">
        {nextItem && (
          <button type="button" className="primary" onClick={onFixNext}>
            {tr("permFixNext")}: {tr(nextItem.titleKey)}
          </button>
        )}
        <button type="button" className="ghost perm-view-all" onClick={onViewAll}>
          {tr("permViewAll")}
        </button>
      </div>
    </section>
  );
}

function PermissionsPanel({
  locale,
  perms,
  onRefresh,
}: {
  locale: Locale;
  perms: PermissionsStatus | null;
  onRefresh: () => void;
}) {
  const tr = (key: MessageKey) => t(locale, key);
  const next = perms ? nextPermKind(perms) : null;
  const done = perms ? grantedPermCount(perms) : 0;
  const [busyKind, setBusyKind] = useState<PermKind | null>(null);

  const runGuide = async (kind: PermKind) => {
    setBusyKind(kind);
    try {
      await guidePermission(kind, () => onRefresh());
    } finally {
      setBusyKind(null);
      onRefresh();
    }
  };

  return (
    <section className="card permissions">
      <p className="pane-title">{tr("systemPermissions")}</p>
      <p className="hint">{tr("permHint")}</p>
      {perms?.all_granted ? (
        <p className="perm-ready">{tr("permReady")}</p>
      ) : (
        <div className="perm-wizard">
          <div className="perm-progress">
            <span>{tr("permProgress").replace("{done}", String(done)).replace("{total}", "3")}</span>
            <div className="perm-progress-bar"><span style={{ width: `${(done / 3) * 100}%` }} /></div>
          </div>
          {next && (
            <>
              <p className="perm-wizard-step">{tr("permCurrentStep")}</p>
              <p className="perm-wizard-lead">
                {tr(permItems(perms!).find((p) => p.kind === next)!.titleKey)} —{" "}
                {tr(permItems(perms!).find((p) => p.kind === next)!.whyKey)}
              </p>
              <button
                type="button"
                className="primary perm-wizard-cta"
                onClick={() => void runGuide(next)}
              >
                {busyKind === next ? tr("permChecking") : tr("permGuideSmart")}
              </button>
            </>
          )}
          <p className="hint">{tr("permRestart")}</p>
        </div>
      )}
      {permItems(perms ?? mockPermissions()).map((item, index) => (
        <PermRow
          key={item.kind}
          locale={locale}
          step={index + 1}
          highlight={item.kind === next}
          title={tr(item.titleKey)}
          why={tr(item.whyKey)}
          state={item.state}
          actions={
            item.kind === "screen_recording" ? (
              <>
                <button
                  type="button"
                  className="ghost perm-action"
                  onClick={() => void runGuide(item.kind)}
                >
                  {busyKind === item.kind ? tr("permChecking") : tr("permGuideSmart")}
                </button>
                <button type="button" className="ghost perm-action" onClick={() => isTauri() && void invoke("request_screen_recording")}>
                  {tr("permRequestScreen")}
                </button>
                <button type="button" className="ghost perm-action" onClick={() => void openPermKind(item.kind).then(onRefresh)}>
                  {tr("permOpen")}
                </button>
              </>
            ) : (
              <>
                <button
                  type="button"
                  className="ghost perm-action"
                  onClick={() => void runGuide(item.kind)}
                >
                  {busyKind === item.kind ? tr("permChecking") : tr("permGuideSmart")}
                </button>
                <button type="button" className="ghost perm-action" onClick={() => void openPermKind(item.kind).then(onRefresh)}>
                  {tr("permOpen")}
                </button>
              </>
            )
          }
        />
      ))}
      <button type="button" className="primary perm-btn" onClick={onRefresh}>
        {tr("permRefresh")}
      </button>
    </section>
  );
}

function PermRow({
  locale,
  title,
  why,
  state,
  actions,
  highlight,
  step,
}: {
  locale: Locale;
  title: string;
  why: string;
  state: string;
  actions: ReactNode;
  highlight?: boolean;
  step?: number;
}) {
  return (
    <div className={`perm-row${highlight ? " perm-row-next" : ""}${state === "granted" ? " perm-row-done" : ""}`}>
      <div className="perm-main">
        {step ? <span className="perm-step">{step}</span> : null}
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

function osLabel(locale: Locale, os: string) {
  const kind = platformKind(os);
  if (kind === "macos") return t(locale, "osMac");
  if (kind === "windows") return t(locale, "osWindows");
  return os;
}

function latencyLabel(ms: number) {
  if (!ms) return "";
  if (ms < 45) return "good";
  if (ms < 90) return "ok";
  return "bad";
}

function SettingsSection({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle?: string;
  children: ReactNode;
}) {
  return (
    <section className="set-section">
      <header className="set-section-head">
        <h3>{title}</h3>
        {subtitle && <p>{subtitle}</p>}
      </header>
      <div className="set-section-body">{children}</div>
    </section>
  );
}

function SettingToggle({
  label,
  hint,
  checked,
  disabled,
  soon,
  soonLabel,
  onChange,
}: {
  label: string;
  hint?: string;
  checked: boolean;
  disabled?: boolean;
  soon?: boolean;
  soonLabel?: string;
  onChange: (value: boolean) => void;
}) {
  return (
    <div className={`set-item${disabled ? " set-item-disabled" : ""}`}>
      <div className="set-item-copy">
        <div className="set-item-title">
          <strong>{label}</strong>
          {soon && soonLabel && <span className="set-soon">{soonLabel}</span>}
        </div>
        {hint && <p className="set-item-hint">{hint}</p>}
      </div>
      <button
        type="button"
        role="switch"
        className={`switch ${checked ? "on" : ""}`}
        aria-checked={checked}
        aria-label={label}
        disabled={disabled}
        onClick={() => !disabled && onChange(!checked)}
      />
    </div>
  );
}

function SettingSelect({
  label,
  hint,
  value,
  options,
  onChange,
}: {
  label: string;
  hint?: string;
  value: string | number;
  options: { value: string | number; label: string }[];
  onChange: (value: string) => void;
}) {
  return (
    <label className="set-item set-item-select">
      <div className="set-item-copy">
        <strong>{label}</strong>
        {hint && <p className="set-item-hint">{hint}</p>}
      </div>
      <select value={value} onChange={(e) => onChange(e.target.value)}>
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
    </label>
  );
}

function LinePickGrid({
  locale,
  active,
  activeLine,
  line1Ms,
  line2Ms,
  onPick,
}: {
  locale: Locale;
  active: string;
  activeLine: string;
  line1Ms: number;
  line2Ms: number;
  onPick: (line: string) => void;
}) {
  const tr = (key: MessageKey) => t(locale, key);
  const items = [
    { value: "auto", title: tr("lineAuto"), hint: tr("signalingHint"), ms: 0 },
    { value: "1", title: tr("line1"), hint: tr("line1Desc"), ms: line1Ms },
    { value: "2", title: tr("line2"), hint: tr("line2Desc"), ms: line2Ms },
  ] as const;

  return (
    <div className="line-grid">
      {items.map((item) => {
        const on = (active || "auto") === item.value;
        const probe =
          item.value === "auto"
            ? on && activeLine
              ? tr("lineProbe").replace(
                  "{ms}",
                  String((activeLine === "2" ? line2Ms : line1Ms) || 0),
                )
              : tr("auto")
            : item.ms
              ? tr("lineProbe").replace("{ms}", String(item.ms))
              : tr("lineProbeOff");
        return (
          <button
            key={item.value}
            type="button"
            className={`line-card${on ? " on" : ""}`}
            onClick={() => onPick(item.value)}
          >
            <span className="line-card-top">
              <strong>{item.title}</strong>
              <span className={`line-probe${item.ms && item.ms < 90 ? " good" : item.ms ? " ok" : ""}`}>{probe}</span>
            </span>
            <span className="line-card-hint">{item.hint}</span>
          </button>
        );
      })}
    </div>
  );
}

function QualityGrid({
  locale,
  value,
  onChange,
}: {
  locale: Locale;
  value: string;
  onChange: (value: string) => void;
}) {
  const tr = (key: MessageKey) => t(locale, key);
  const items = [
    { value: "smooth", title: tr("qualitySmooth"), hint: tr("qualitySpeedHint") },
    { value: "balanced", title: tr("qualityBalanced"), hint: tr("qualityBalancedHint") },
    { value: "high", title: tr("qualityHigh"), hint: tr("qualityClarityHint") },
  ] as const;
  const current = value === "original" ? "high" : value;

  return (
    <div className="quality-grid">
      {items.map((item) => (
        <button
          key={item.value}
          type="button"
          className={`quality-card${current === item.value ? " on" : ""}`}
          onClick={() => onChange(item.value)}
        >
          <strong>{item.title}</strong>
          <span>{item.hint}</span>
        </button>
      ))}
    </div>
  );
}

function ConnectionStatusPanel({
  locale,
  snap,
}: {
  locale: Locale;
  snap: Snapshot;
}) {
  const tr = (key: MessageKey) => t(locale, key);
  const lineLabel =
    snap.settings.signaling_line === "auto"
      ? snap.active_line === "2"
        ? tr("line2")
        : tr("line1")
      : snap.settings.signaling_line === "2"
        ? tr("line2")
        : tr("line1");

  return (
    <div className="status-panel">
      <div className="status-panel-item">
        <span>{tr("networkStatus")}</span>
        <strong className={snap.ready ? "good" : "warn"}>{snap.ready ? tr("online") : tr("offline")}</strong>
      </div>
      <div className="status-panel-item">
        <span>{tr("activeLine")}</span>
        <strong>{lineLabel}</strong>
      </div>
      <div className="status-panel-item">
        <span>{tr("signalLatency")}</span>
        <strong className={latencyLabel(snap.rtt_ms)}>{snap.rtt_ms ? `${snap.rtt_ms} ms` : "—"}</strong>
      </div>
    </div>
  );
}

const CONNECT_STEPS = ["stepFind", "stepHandshake", "stepP2p", "stepVideo"] as const;
const CONNECT_STEP_STATUS = ["stepStatusFind", "stepStatusHandshake", "stepStatusP2p", "stepStatusVideo"] as const;

function ConnectPasswordModal({
  locale,
  deviceId,
  password,
  error,
  onPasswordChange,
  onCancel,
  onConfirm,
}: {
  locale: Locale;
  deviceId: string;
  password: string;
  error: string;
  onPasswordChange: (value: string) => void;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const tr = (key: MessageKey) => t(locale, key);

  return (
    <div className="overlay connect-overlay">
      <div className="connect-modal">
        <div className="connect-modal-glow" aria-hidden />
        <div className="connect-modal-head">
          <div className="connect-modal-badge">
            <span className="connect-lock" aria-hidden>🔒</span>
            <span>{tr("connectSecureLink")}</span>
          </div>
          <p className="connect-modal-kicker">{tr("connectVerify")}</p>
        </div>
        <div className="connect-device-card">
          <span className="connect-device-icon" aria-hidden><OsGlyph os="unknown" className="os-glyph" /></span>
          <div>
            <p className="connect-device-label">{tr("connectTo")}</p>
            <h3>{deviceId}</h3>
          </div>
        </div>
        <p className="connect-modal-hint">{tr("enterTempPassword")}</p>
        <label className="connect-input-wrap">
          <span>{tr("password")}</span>
          <input
            autoFocus
            className="connect-input"
            value={password}
            onChange={(e) => onPasswordChange(e.target.value.toUpperCase())}
            placeholder="••••••"
            onKeyDown={(e) => e.key === "Enter" && onConfirm()}
          />
        </label>
        {error && <p className="error">{error}</p>}
        <div className="connect-modal-actions">
          <button type="button" onClick={onCancel}>{tr("cancel")}</button>
          <button type="button" className="primary" onClick={onConfirm}>{tr("continue")}</button>
        </div>
      </div>
    </div>
  );
}

function ConnectFlowOverlay({
  locale,
  peerName,
  connectStep,
  onCancel,
}: {
  locale: Locale;
  peerName: string;
  connectStep: number;
  onCancel: () => void;
}) {
  const tr = (key: MessageKey) => t(locale, key);
  const activeIndex = Math.max(0, connectStep - 1);
  const progress = Math.min(100, Math.round((connectStep / CONNECT_STEPS.length) * 100));
  const statusKey = CONNECT_STEP_STATUS[Math.min(activeIndex, CONNECT_STEP_STATUS.length - 1)];

  return (
    <div className="overlay connect-flow">
      <div className="connect-flow-grid" aria-hidden />
      <div className="connect-flow-panel">
        <header className="connect-flow-head">
          <div className="connect-flow-brand">
            <Logo size={28} />
            <div>
              <p className="connect-flow-kicker">RemoteX</p>
              <h2>{tr("connecting")}</h2>
            </div>
          </div>
          <div className="connect-flow-meter" aria-hidden>
            <svg viewBox="0 0 44 44">
              <circle className="connect-flow-ring-bg" cx="22" cy="22" r="18" />
              <circle
                className="connect-flow-ring"
                cx="22"
                cy="22"
                r="18"
                style={{ strokeDashoffset: `${113 - (113 * progress) / 100}` }}
              />
            </svg>
            <strong>{progress}%</strong>
          </div>
        </header>

        <div className="connect-flow-orbit">
          <div className="connect-node local">
            <span className="connect-node-ring" aria-hidden />
            <span className="connect-node-icon"><OsGlyph os="macos" className="os-glyph" /></span>
            <span className="connect-node-label">{tr("you")}</span>
          </div>
          <div className="connect-link-track">
            <div className="connect-link-beam" style={{ width: `${progress}%` }} />
            <span className="connect-link-dot" style={{ left: `${Math.max(8, Math.min(92, progress))}%` }} />
            <span className="connect-link-scan" aria-hidden />
          </div>
          <div className="connect-node remote">
            <span className="connect-node-ring" aria-hidden />
            <span className="connect-node-icon"><OsGlyph os="unknown" className="os-glyph" /></span>
            <span className="connect-node-label">{peerName}</span>
          </div>
        </div>

        <p className="connect-flow-status">{tr(statusKey)}</p>

        <ol className="connect-flow-steps">
          {CONNECT_STEPS.map((key, index) => {
            const done = connectStep > index + 1;
            const active = connectStep === index + 1;
            return (
              <li key={key} className={`${done ? "done" : ""}${active ? " active" : ""}`}>
                <span className="connect-step-dot">{done ? "✓" : index + 1}</span>
                <div className="connect-step-copy">
                  <strong>{tr(key)}</strong>
                  <span>{tr(CONNECT_STEP_STATUS[index])}</span>
                </div>
                <span className="connect-step-tag">
                  {done ? tr("connectLinkReady") : active ? tr("connecting") : "—"}
                </span>
              </li>
            );
          })}
        </ol>

        <button type="button" className="connect-flow-cancel" onClick={onCancel}>
          {tr("connectCancel")}
        </button>
      </div>
    </div>
  );
}

function IncomingConnectModal({
  locale,
  fromName,
  fromOs,
  onDecline,
  onAccept,
}: {
  locale: Locale;
  fromName: string;
  fromOs: string;
  onDecline: () => void;
  onAccept: () => void;
}) {
  const tr = (key: MessageKey) => t(locale, key);

  return (
    <div className="overlay connect-overlay">
      <div className="connect-modal incoming-modal">
        <div className="connect-modal-glow incoming-glow" aria-hidden />
        <div className="connect-radar" aria-hidden>
          <span /><span /><span />
        </div>
        <p className="connect-modal-kicker">{tr("incomingSecure")}</p>
        <h2>{tr("incomingTitle")}</h2>
        <div className="connect-device-card incoming-device">
          <span className="connect-device-icon"><OsGlyph os={fromOs} className="os-glyph" /></span>
          <div>
            <p className="connect-device-label">{tr("incomingFrom")}</p>
            <h3>{fromName}</h3>
          </div>
        </div>
        <p className="connect-modal-hint">{fromName} {tr("incomingBody")}</p>
        <div className="connect-modal-actions incoming-actions">
          <button type="button" onClick={onDecline}>{tr("decline")}</button>
          <button type="button" className="primary" onClick={onAccept}>{tr("accept")}</button>
        </div>
      </div>
    </div>
  );
}
