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
  const [connectId, setConnectId] = useState("");
  const [connectPassword, setConnectPassword] = useState("");
  const [passwordStep, setPasswordStep] = useState(false);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState("");
  const [customPassword, setCustomPassword] = useState("");
  const [chromeVisible, setChromeVisible] = useState(true);
  const chromeTimer = useRef<number | null>(null);
  const [connectStep, setConnectStep] = useState(0);
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

  useEffect(() => {
    if (snap.last_error) setError(translateError(locale, snap.last_error));
  }, [snap.last_error, locale]);

  useEffect(() => {
    setCustomPassword(snap.temp_password);
  }, [snap.temp_password]);

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

  const saveCustomPassword = () =>
    run(async () => {
      const nextValue = customPassword.trim();
      if (!nextValue) return;
      if (isTauri()) {
        const next = await invoke<Snapshot>("set_temp_password", { password: nextValue });
        setSnap(next);
      } else {
        const normalized = nextValue.replace(/[^a-zA-Z0-9]/g, "").toUpperCase();
        setSnap({
          ...snap,
          temp_password: normalized,
          formatted_password: normalized.split("").join(" "),
        });
      }
    });

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
        </div>
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

          <section className="card device-card">
            <p className="eyebrow">{tr("thisDevice")}</p>
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
            <div className={`status ${snap.ready ? "ready" : "offline"}`}>
              <span className="dot" />
              {snap.ready ? tr("ready") : tr("connectingNetwork")}
              {snap.ready && snap.rtt_ms > 0 ? ` · ${snap.rtt_ms}ms` : ""}
            </div>
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
                <button type="button" className="icon-btn" onClick={() => setHidePassword((v) => !v)} aria-label="Toggle password">
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
            <label className="custom-password">
              <span>{tr("customPassword")}</span>
              <div className="custom-password-row">
                <input
                  value={customPassword}
                  onChange={(e) => setCustomPassword(e.target.value.replace(/[^a-zA-Z0-9]/g, "").toUpperCase())}
                  placeholder={tr("passwordHint")}
                  maxLength={16}
                  spellCheck={false}
                  onKeyDown={(e) => e.key === "Enter" && saveCustomPassword()}
                  onBlur={() => {
                    if (customPassword && customPassword !== snap.temp_password) saveCustomPassword();
                  }}
                />
                <button type="button" className="ghost save-password" onClick={saveCustomPassword}>
                  {tr("savePassword")}
                </button>
              </div>
            </label>
            <button
              type="button"
              className={`copy-all ${copied === "both" ? "copied" : ""}`}
              onClick={copyBoth}
            >
              {copied === "both" ? tr("copied") : tr("copyBoth")}
            </button>
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
            <section className="recents">
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

          <footer>RemoteX v0.2.6</footer>
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

function PlatformBadge({ os, label }: { os: string; label: string }) {
  const kind = platformKind(os);
  return (
    <div className={`platform-badge ${kind}`} aria-label={label}>
      {kind === "macos" ? (
        <svg viewBox="0 0 24 24" aria-hidden>
          <path d="M16.365 1.43c0 1.14-.413 2.193-1.232 3.014-.855.855-1.902 1.275-3.028 1.197-.14-1.098.402-2.248 1.213-3.07.855-.88 2.022-1.41 3.047-1.141zM20.88 17.203c-.747 1.626-1.109 2.358-2.072 3.803-1.342 1.983-3.232 4.458-5.586 4.474-2.088.015-2.626-1.357-5.456-1.357-2.83 0-3.431 1.327-5.47 1.372-2.187.045-3.851-2.276-5.193-4.254C-.55 18.853-.972 13.212 2.36 10.21c1.657-1.534 3.817-2.44 6.005-2.466 2.358-.03 3.643 1.327 5.47 1.327 1.827 0 2.947-1.327 5.564-1.302 2.006.03 3.676 1.089 5.01 2.465-4.403 2.427-3.692 8.744.471 10.969z" />
        </svg>
      ) : kind === "windows" ? (
        <svg viewBox="0 0 24 24" aria-hidden>
          <path d="M3 12.5V3.5l8 1.2v7.8H3zm9 0V4.3l9 1.3v7.9H12zM3 20.5v-7.8h8v9L3 20.5zm9-.9V13h9v8.3l-9-1.7z" />
        </svg>
      ) : (
        <svg viewBox="0 0 24 24" aria-hidden>
          <rect x="4" y="5" width="16" height="11" rx="1.5" />
          <path d="M8 19h8" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
        </svg>
      )}
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
              <span>{tr(item.key)}</span>
              {item.id === "permissions" && perms && perms.platform === "macos" && !perms.all_granted && (
                <span className="nav-badge">{3 - grantedPermCount(perms)}</span>
              )}
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
              <div className="set-stack">
                <span>{tr("signalingServer")}</span>
                <div className="line-switch" role="radiogroup" aria-label={tr("signalingServer")}>
                  {([
                    ["1", "line1"],
                    ["2", "line2"],
                  ] as const).map(([value, key]) => (
                    <button
                      key={value}
                      type="button"
                      role="radio"
                      aria-checked={(snap.settings.signaling_line === "2" ? "2" : "1") === value}
                      className={(snap.settings.signaling_line === "2" ? "2" : "1") === value ? "on" : ""}
                      onClick={() => onSettings({ signaling_line: value })}
                    >
                      {tr(key)}
                    </button>
                  ))}
                </div>
              </div>
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
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      permanent && onPermanentPassword(permanent);
                      (e.target as HTMLInputElement).blur();
                    }
                  }}
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
                <select value={snap.settings.quality === "original" ? "high" : snap.settings.quality} onChange={(e) => onSettings({ quality: e.target.value })}>
                  <option value="smooth">{tr("qualitySmooth")}</option>
                  <option value="balanced">{tr("qualityBalanced")}</option>
                  <option value="high">{tr("qualityHigh")}</option>
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
            <PermissionsPanel locale={locale} perms={perms} onRefresh={onRefreshPerms} />
          )}

          {tab === "about" && (
            <section className="card about">
              <Logo size={48} />
              <h2>RemoteX</h2>
              <p>{tr("aboutTagline")}</p>
              <p className="muted">{tr("aboutNote")}</p>
              <p className="muted">v0.2.6 · macOS / Windows</p>
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
                onClick={() => void openPermKind(next).then(onRefresh)}
              >
                {tr("permOpenNow")}
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
                <button type="button" className="ghost perm-action" onClick={() => isTauri() && void invoke("request_screen_recording")}>
                  {tr("permRequestScreen")}
                </button>
                <button type="button" className="ghost perm-action" onClick={() => void openPermKind(item.kind).then(onRefresh)}>
                  {tr("permOpen")}
                </button>
              </>
            ) : (
              <button type="button" className="ghost perm-action" onClick={() => void openPermKind(item.kind).then(onRefresh)}>
                {tr("permOpen")}
              </button>
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
          <span className="connect-device-icon" aria-hidden>▣</span>
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
            <span className="connect-node-icon">⌘</span>
            <span className="connect-node-label">{tr("you")}</span>
          </div>
          <div className="connect-link-track">
            <div className="connect-link-beam" style={{ width: `${progress}%` }} />
            <span className="connect-link-dot" style={{ left: `${Math.max(8, Math.min(92, progress))}%` }} />
            <span className="connect-link-scan" aria-hidden />
          </div>
          <div className="connect-node remote">
            <span className="connect-node-ring" aria-hidden />
            <span className="connect-node-icon">▣</span>
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
          <span className="connect-device-icon">{fromOs === "macos" ? "⌘" : "▣"}</span>
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
