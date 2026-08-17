export type OsKind = "macos" | "windows" | "linux" | "unknown";

export type SessionPhase = "idle" | "connecting" | "incoming" | "connected";

export interface RecentDevice {
  id: string;
  name: string;
  os: string;
  favorite: boolean;
  last_seen: string;
}

export interface AppSettings {
  signaling_url: string;
  language: string;
  theme: string;
  start_at_login: boolean;
  minimize_to_tray: boolean;
  auto_update: boolean;
  unattended: boolean;
  require_confirm: boolean;
  allow_clipboard: boolean;
  allow_file_transfer: boolean;
  lock_after_session: boolean;
  p2p_preferred: boolean;
  hardware_encode: boolean;
  quality: string;
  fps: number;
}

export interface SessionView {
  session_id: string;
  peer_id: string;
  peer_name: string;
  peer_os: string;
  rtt_ms: number;
  down_kbps: number;
  up_kbps: number;
  path: string;
  quality: string;
}

export interface IncomingView {
  session_id: string;
  from_id: string;
  from_name: string;
  from_os: string;
}

export interface Snapshot {
  device_id: string;
  formatted_id: string;
  name: string;
  os: OsKind;
  temp_password: string;
  formatted_password: string;
  ready: boolean;
  rtt_ms: number;
  signaling_url: string;
  phase: SessionPhase;
  session: SessionView | null;
  incoming: IncomingView | null;
  recents: RecentDevice[];
  settings: AppSettings;
  unattended: boolean;
  has_permanent_password: boolean;
}

export const defaultSettings = (): AppSettings => ({
  signaling_url: "ws://127.0.0.1:7829/ws",
  language: "system",
  theme: "system",
  start_at_login: false,
  minimize_to_tray: true,
  auto_update: true,
  unattended: false,
  require_confirm: true,
  allow_clipboard: true,
  allow_file_transfer: true,
  lock_after_session: false,
  p2p_preferred: true,
  hardware_encode: true,
  quality: "balanced",
  fps: 60,
});

export const mockSnapshot = (): Snapshot => ({
  device_id: "825391726",
  formatted_id: "825 391 726",
  name: "MacBook Pro",
  os: "macos",
  temp_password: "X7F9K2",
  formatted_password: "X 7 F 9 K 2",
  ready: true,
  rtt_ms: 18,
  signaling_url: "ws://127.0.0.1:7829/ws",
  phase: "idle",
  session: null,
  incoming: null,
  recents: [
    {
      id: "825391726",
      name: "Office PC",
      os: "windows",
      favorite: true,
      last_seen: new Date().toISOString(),
    },
    {
      id: "391285663",
      name: "MacBook Pro",
      os: "macos",
      favorite: false,
      last_seen: new Date().toISOString(),
    },
  ],
  settings: defaultSettings(),
  unattended: false,
  has_permanent_password: false,
});

export function previewSnapshot(scene: string | null): Snapshot {
  const base = mockSnapshot();
  if (scene === "connecting") {
    return {
      ...base,
      phase: "connecting",
      session: {
        session_id: "preview",
        peer_id: "391285663",
        peer_name: "Office PC",
        peer_os: "windows",
        rtt_ms: 0,
        down_kbps: 0,
        up_kbps: 0,
        path: "unknown",
        quality: "balanced",
      },
    };
  }
  if (scene === "session") {
    return {
      ...base,
      rtt_ms: 36,
      phase: "connected",
      session: {
        session_id: "preview",
        peer_id: "391285663",
        peer_name: "Office PC",
        peer_os: "windows",
        rtt_ms: 36,
        down_kbps: 8420,
        up_kbps: 186,
        path: "p2p",
        quality: "balanced",
      },
    };
  }
  if (scene === "incoming") {
    return {
      ...base,
      phase: "incoming",
      incoming: {
        session_id: "preview",
        from_id: "391285663",
        from_name: "Office PC",
        from_os: "windows",
      },
    };
  }
  return base;
}
