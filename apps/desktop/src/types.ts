export type OsKind = "macos" | "windows" | "linux" | "unknown";

export type SessionPhase = "idle" | "connecting" | "incoming" | "connected";

export interface NearbyDevice {
  id: string;
  name: string;
  os: string;
  ws: string;
}

export interface PermissionsStatus {
  screen_recording: "granted" | "denied" | "unknown";
  accessibility: "granted" | "denied" | "unknown";
  input_monitoring: "granted" | "denied" | "unknown";
  platform: string;
  all_granted: boolean;
}

export interface RecentDevice {
  id: string;
  name: string;
  os: string;
  favorite: boolean;
  last_seen: string;
}

export interface AppSettings {
  signaling_url: string;
  signaling_line: string;
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
  lan_url: string;
  hosting: boolean;
  phase: SessionPhase;
  session: SessionView | null;
  incoming: IncomingView | null;
  recents: RecentDevice[];
  nearby: NearbyDevice[];
  settings: AppSettings;
  unattended: boolean;
  has_permanent_password: boolean;
  last_error: string | null;
  is_host: boolean;
}

export const defaultSettings = (): AppSettings => ({
  signaling_url: "ws://127.0.0.1:7829/ws",
  signaling_line: "1",
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
  quality: "high",
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
  lan_url: "ws://192.168.1.18:7829/ws",
  hosting: true,
  phase: "idle",
  session: null,
  incoming: null,
  recents: [
    {
      id: "194827563",
      name: "Office PC",
      os: "windows",
      favorite: true,
      last_seen: new Date().toISOString(),
    },
    {
      id: "391285663",
      name: "Studio Mac",
      os: "macos",
      favorite: false,
      last_seen: new Date().toISOString(),
    },
  ],
  nearby: [],
  settings: defaultSettings(),
  unattended: false,
  has_permanent_password: false,
  last_error: null,
  is_host: false,
});

export function previewSnapshot(scene: string | null): Snapshot {
  const base = { ...mockSnapshot(), recents: [] };
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
        quality: "high",
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
        quality: "high",
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
  if (scene === "permissions") {
    return { ...base, phase: "idle" };
  }
  return base;
}
