import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState, type PointerEvent, type RefObject } from "react";

export type MediaInfo = {
  title: string;
  artist: string;
  album: string;
  appId: string;
  playing: boolean;
  positionMs: number;
  durationMs: number;
  thumbnail: string | null;
};

export type NotificationInfo = {
  id: number;
  app: string;
  title: string;
  body: string;
  icon: string | null;
};

export type BatteryInfo = {
  hasBattery: boolean;
  percent: number | null;
  plugged: boolean;
  charging: boolean;
};

// Opened as a plain web page (bun run dev), there is no Rust side: fall back
// to mock data so the UI can be designed in a browser.
export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/**
 * Browser-only demo states with neutral sample data, for screenshots and UI
 * work: open the dev server with ?demo=<state>, one of compact, music,
 * notifications, notification, permission or claude.
 */
export const demo = isTauri ? null : new URLSearchParams(window.location.search).get("demo");

// Sample album art, drawn inline so the demo needs no image files.
const MOCK_ARTWORK =
  "data:image/svg+xml;utf8," +
  encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 120">
      <defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stop-color="#5e5ce6"/><stop offset="1" stop-color="#ff375f"/>
      </linearGradient></defs>
      <rect width="120" height="120" fill="url(#g)"/>
      <circle cx="60" cy="60" r="30" fill="none" stroke="#fff" stroke-opacity=".75" stroke-width="5"/>
      <circle cx="60" cy="60" r="7" fill="#fff" fill-opacity=".85"/>
    </svg>`,
  );

const MOCK_MEDIA: MediaInfo = {
  title: "Midnight Drive",
  artist: "Sample Artist",
  album: "Demo",
  appId: "Spotify.exe",
  playing: true,
  positionMs: 72_000,
  durationMs: 214_000,
  thumbnail: MOCK_ARTWORK,
};

const SAMPLE_NOTIFICATIONS: NotificationInfo[] = [
  { id: 1, app: "WhatsApp", title: "Rina", body: "Jangan lupa meeting jam 3 ya!", icon: null },
  { id: 2, app: "GitHub", title: "New pull request", body: "feat: add dark mode toggle (#42)", icon: null },
  { id: 3, app: "Claude", title: "Trip planning", body: "Claude has finished responding", icon: null },
];

const SAMPLE_PERMISSION: PermissionRequest = {
  id: 1,
  tool: "Bash",
  summary: 'cd "D:/projects/my-app" && git status && bun run build',
  project: "my-app",
};

/** Subscribes to a value the Rust side pushes as events, seeded by a getter. */
function useNative<T>(event: string, getter: string, mock: T): [T, (v: T) => void] {
  const [value, setValue] = useState<T>(isTauri ? (null as T) : mock);
  useEffect(() => {
    if (!isTauri) return;
    invoke<T>(getter).then(setValue);
    const unlisten = listen<T>(event, (e) => setValue(e.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, [event, getter]);
  return [value, setValue];
}

export function useMedia() {
  const [media, setMedia] = useNative<MediaInfo | null>("media", "get_media", MOCK_MEDIA);

  // Mock playback so the progress bar and controls do something in a browser.
  useEffect(() => {
    if (isTauri || !media?.playing) return;
    const id = setInterval(() => {
      setMedia({ ...media, positionMs: (media.positionMs + 1000) % media.durationMs });
    }, 1000);
    return () => clearInterval(id);
  }, [media, setMedia]);

  const control = (cmd: "media_play_pause" | "media_next" | "media_previous") => {
    if (isTauri) {
      invoke(cmd).catch(console.error);
    } else if (media && cmd === "media_play_pause") {
      setMedia({ ...media, playing: !media.playing });
    } else if (media) {
      setMedia({ ...media, positionMs: 0 });
    }
  };

  return {
    media,
    playPause: () => control("media_play_pause"),
    next: () => control("media_next"),
    previous: () => control("media_previous"),
  };
}

export function useBattery() {
  return useNative<BatteryInfo | null>("battery", "get_battery", {
    hasBattery: true,
    percent: 76,
    plugged: false,
    charging: false,
  })[0];
}

export type VolumeInfo = {
  level: number;
  muted: boolean;
  scope: "app" | "system";
};

/**
 * Volume of the app playing the media (or the speakers as a fallback).
 * Refreshed while `active`, since other apps can change it too.
 */
export function useVolume(active: boolean, appId: string | undefined) {
  const [volume, setVolume] = useState<VolumeInfo | null>(
    isTauri ? null : { level: 0.6, muted: false, scope: "app" },
  );
  // Ignore stale polls that land right after the user moved the slider.
  const lastChange = useRef(0);
  // Latest level, so quick successive scroll steps add up within one render.
  const levelRef = useRef(volume?.level ?? 0);
  levelRef.current = volume?.level ?? levelRef.current;

  useEffect(() => {
    if (!isTauri || !active) return;
    const refresh = () => {
      if (Date.now() - lastChange.current < 1000) return;
      invoke<VolumeInfo>("get_volume").then(setVolume).catch(() => setVolume(null));
    };
    refresh();
    const id = setInterval(refresh, 1500);
    return () => clearInterval(id);
  }, [active, appId]);

  const update = useCallback((cmd: string, args: Record<string, unknown>, optimistic: Partial<VolumeInfo>) => {
    lastChange.current = Date.now();
    setVolume((v) => (v ? { ...v, ...optimistic } : v));
    if (isTauri) invoke<VolumeInfo>(cmd, args).then(setVolume).catch(console.error);
  }, []);

  const setLevel = useCallback(
    (level: number) => {
      const l = Math.round(Math.min(1, Math.max(0, level)) * 100) / 100;
      levelRef.current = l;
      update("set_volume", { level: l }, { level: l, muted: false });
    },
    [update],
  );
  const nudge = useCallback((delta: number) => setLevel(levelRef.current + delta), [setLevel]);
  const setMuted = useCallback((muted: boolean) => update("set_muted", { muted }, { muted }), [update]);

  return { volume, setLevel, nudge, setMuted };
}

export type PermissionRequest = {
  id: number;
  tool: string;
  summary: string;
  project: string;
};

export type PermissionDecision = "allow" | "deny" | "ask";

/** Claude Code permission prompts forwarded by the PermissionRequest hook, oldest first. */
export function usePermissions() {
  const [queue, setQueue] = useState<PermissionRequest[]>(demo === "permission" ? [SAMPLE_PERMISSION] : []);
  const remove = useCallback((id: number) => setQueue((q) => q.filter((p) => p.id !== id)), []);

  useEffect(() => {
    if (!isTauri) {
      // In a browser, call mockPermission() from the console to preview a prompt.
      Object.assign(window, {
        mockPermission: () => setQueue((q) => [...q, { ...SAMPLE_PERMISSION, id: Date.now() }]),
      });
      return;
    }
    const added = listen<PermissionRequest>("permission", (e) => setQueue((q) => [...q, e.payload]));
    // Timed out on the Rust side: Claude has fallen back to its own prompt.
    const closed = listen<number>("permission-closed", (e) => remove(e.payload));
    return () => {
      added.then((f) => f());
      closed.then((f) => f());
    };
  }, [remove]);

  const answer = useCallback(
    (id: number, decision: PermissionDecision) => {
      remove(id);
      if (isTauri) invoke("answer_permission", { id, decision }).catch(console.error);
    },
    [remove],
  );

  return { permission: queue[0] ?? null, waiting: queue.length, answer };
}

export type ClaudeMode = "chat" | "code" | "";

const EXTRA_KEY = "floatingbar.claudeExtra";
/** How far the Claude page can be dragged taller; fits the 640px window. */
export const MAX_EXTRA = 340;
const TALL_EXTRA = 200;
export type ClaudeMessage = { role: "user" | "claude"; text: string };
/** The open conversation as mirrored from the app. */
export type ClaudeHistory = { messages: ClaudeMessage[]; working: boolean; error: string | null };

/**
 * Talk to the Claude desktop app from the pill: pick Chat/Code mode and a
 * conversation from its sidebar, see its recent messages, send a prompt
 * (with an optional pasted image), and follow the reply.
 */
export function useAsk() {
  const [draft, setDraft] = useState("");
  // Extra height (px) the user dragged the Claude page to; remembered across
  // restarts, kept while the pill closes and reopens.
  const [extra, setExtraState] = useState(() => {
    if (demo === "claude") return 90;
    try {
      return Number(localStorage.getItem(EXTRA_KEY)) || 0;
    } catch {
      return 0;
    }
  });
  const setExtra = useCallback((px: number) => {
    const clamped = Math.round(Math.min(MAX_EXTRA, Math.max(0, px)));
    setExtraState(clamped);
    try {
      localStorage.setItem(EXTRA_KEY, String(clamped));
    } catch {
      // Storage unavailable: the size just isn't remembered.
    }
  }, []);
  const [mode, setModeState] = useState<ClaudeMode>(isTauri ? "" : "chat");
  const [chats, setChats] = useState<string[]>(
    isTauri ? [] : ["Trip planning", "Bug in login form", "Weekly report draft"],
  );
  /** null = start a new chat. */
  const [chat, setChat] = useState<string | null>(demo === "claude" ? "Trip planning" : null);
  /** The conversation open in the app. */
  const [current, setCurrent] = useState<string | null>(demo === "claude" ? "Trip planning" : null);
  // A mode just picked here wins over what the app reports while it switches.
  const pendingMode = useRef<{ mode: ClaudeMode; until: number } | null>(null);
  const [history, setHistory] = useState<ClaudeHistory | null>(
    demo === "claude"
      ? {
          messages: [
            { role: "user", text: "Bikinin itinerary 3 hari di Jogja dong, budget mahasiswa" },
            {
              role: "claude",
              text: "Hari 1: Malioboro & Keraton, makan gudeg di Wijilan.\nHari 2: Candi Prambanan pagi, sore ke Tebing Breksi.\nHari 3: Pantai Parangtritis, pulang lewat Bukit Bintang.",
            },
          ],
          working: false,
          error: null,
        }
      : null,
  );
  /** A pasted image (data URL) to attach on send; it's taken from the clipboard. */
  const [image, setImage] = useState<string | null>(null);
  const [lastPrompt, setLastPrompt] = useState("");
  const [listError, setListError] = useState<string | null>(null);

  /** Re-read the app's mode and sidebar; `load` also shows the open chat's messages. */
  const refresh = useCallback((load = false) => {
    if (!isTauri) return;
    invoke<{ mode: ClaudeMode; chats: string[]; current: string | null }>("claude_chats")
      .then((r) => {
        setListError(null);
        const pending = pendingMode.current;
        const switching = pending !== null && Date.now() < pending.until && pending.mode !== r.mode;
        if (!switching) setModeState(r.mode);
        setChats(r.chats);
        setCurrent(r.current);
        // Follow what's open in the app; otherwise start a new one.
        setChat(r.current);
        // Only mirror an open chat; opening null would press New in the app.
        if (load && r.current) invoke("claude_open", { chat: r.current });
      })
      .catch((e) => setListError(String(e)));
  }, []);

  useEffect(() => {
    if (!isTauri) return;
    const unlisten = listen<ClaudeHistory>("claude-history", (e) => setHistory(e.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // When a reply finishes, a new chat has a title in the sidebar: pick it up.
  const working = history?.working ?? false;
  const wasWorking = useRef(false);
  useEffect(() => {
    if (wasWorking.current && !working) refresh();
    wasWorking.current = working;
  }, [working, refresh]);

  /** Open a conversation (null = a new one) in the app and show its messages. */
  const selectChat = useCallback((c: string | null) => {
    setChat(c);
    setHistory(c ? { messages: [], working: true, error: null } : null);
    if (isTauri) invoke("claude_open", { chat: c });
  }, []);

  const setMode = useCallback(
    (m: ClaudeMode) => {
      setModeState(m);
      setChat(null);
      setHistory(null);
      if (!isTauri) return;
      pendingMode.current = { mode: m, until: Date.now() + 3000 };
      invoke("claude_set_mode", { mode: m });
      // The sidebar swaps to that mode's conversations a moment later.
      window.setTimeout(() => refresh(), 900);
      window.setTimeout(() => refresh(true), 2200);
    },
    [refresh],
  );

  const send = useCallback(() => {
    const prompt = draft.trim();
    if (!prompt) return;
    setDraft("");
    setLastPrompt(prompt);
    const pasteImage = image !== null;
    setImage(null);
    // Show the question right away; the app's copy replaces it once it lands.
    const before = history?.messages ?? [];
    setHistory({ messages: [...before, { role: "user", text: prompt }], working: true, error: null });
    if (isTauri) {
      invoke("claude_send", { chat, prompt, pasteImage });
      return;
    }
    // Browser preview: fake a streamed answer.
    const words = `Ini balasan contoh untuk "${prompt}" yang dibaca dari aplikasi Claude.`.split(" ");
    words.forEach((_, i) =>
      setTimeout(() => {
        const answer: ClaudeMessage = { role: "claude", text: words.slice(0, i + 1).join(" ") };
        setHistory({
          messages: [...before, { role: "user", text: prompt }, answer],
          working: i < words.length - 1,
          error: null,
        });
      }, 90 * (i + 1)),
    );
  }, [draft, chat, image, history]);

  return {
    draft,
    setDraft,
    extra,
    setExtra,
    /** Jump between the compact size and a roomy one. */
    toggleTall: () => setExtra(extra > 0 ? 0 : TALL_EXTRA),
    mode,
    setMode,
    chats,
    chat,
    selectChat,
    current,
    refresh,
    listError,
    history,
    busy: working,
    image,
    setImage,
    send,
    /** Fallback when the app can't be driven: a new chat with the prompt typed in. */
    openInApp: () => {
      if (isTauri) invoke("open_in_claude", { prompt: draft.trim() || lastPrompt }).catch(console.error);
    },
  };
}

export type Shortcut = {
  name: string;
  target: string;
  icon?: string;
};

/** Launcher shortcuts from shortcuts.json, re-read whenever `active` turns on. */
export function useShortcuts(active: boolean) {
  const [shortcuts, setShortcuts] = useState<Shortcut[]>(
    isTauri
      ? []
      : [
          { name: "WhatsApp", target: "https://web.whatsapp.com" },
          { name: "GitHub", target: "https://github.com" },
          { name: "YouTube", target: "https://www.youtube.com" },
          { name: "Google Drive", target: "https://drive.google.com" },
        ],
  );
  useEffect(() => {
    if (isTauri && active) invoke<Shortcut[]>("get_shortcuts").then(setShortcuts);
  }, [active]);

  return {
    shortcuts,
    open: (index: number) => {
      if (isTauri) invoke("open_shortcut", { index }).catch(console.error);
      else window.open(shortcuts[index]?.target, "_blank");
    },
    edit: () => {
      if (isTauri) invoke("edit_shortcuts").catch(console.error);
    },
  };
}

export function shortcutIcon(s: Shortcut): string | null {
  if (s.icon) return s.icon;
  try {
    const url = new URL(s.target);
    if (url.protocol === "http:" || url.protocol === "https:") {
      return `https://www.google.com/s2/favicons?domain=${url.hostname}&sz=64`;
    }
  } catch {
    // Not a URL (a path or app protocol): use a letter instead.
  }
  return null;
}

export type ReceivedNotification = NotificationInfo & { receivedAt: number };

const HISTORY_LIMIT = 20;

/**
 * The toast currently shown on the pill, the recent ones for the
 * notifications page, and why toasts can't be read (if so).
 */
export function useNotifications() {
  const [current, setCurrent] = useState<NotificationInfo | null>(
    demo === "notification" ? SAMPLE_NOTIFICATIONS[0] : null,
  );
  const [history, setHistory] = useState<ReceivedNotification[]>(() =>
    demo === "notifications"
      ? SAMPLE_NOTIFICATIONS.map((n, i) => ({ ...n, receivedAt: Date.now() - i * 7 * 60_000 }))
      : [],
  );
  const [error] = useNative<string | null>("notify-error", "get_notify_error", null);

  const push = useCallback((n: NotificationInfo) => {
    setCurrent(n);
    setHistory((h) => [{ ...n, receivedAt: Date.now() }, ...h].slice(0, HISTORY_LIMIT));
  }, []);

  useEffect(() => {
    if (!isTauri) {
      // In a browser, call mockNotify() from the console to preview a toast.
      let next = 0;
      Object.assign(window, {
        mockNotify: () => {
          const sample = SAMPLE_NOTIFICATIONS[next++ % SAMPLE_NOTIFICATIONS.length];
          push({ ...sample, id: Date.now() });
        },
      });
      return;
    }
    const unlisten = listen<NotificationInfo>("notification", (e) => push(e.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, [push]);

  const dismiss = useCallback(() => setCurrent(null), []);
  const remove = useCallback((id: number) => setHistory((h) => h.filter((n) => n.id !== id)), []);
  const clear = useCallback(() => setHistory([]), []);
  return { notification: current, dismiss, push, history, remove, clear, error };
}

/**
 * Drag the pill to move the window. Dragging only starts after the pointer
 * travels a few px, so clicks and double-clicks still work.
 */
export function useDragWindow() {
  const start = useRef<{ x: number; y: number; grabX: number; grabY: number; headerX: number; headerY: number } | null>(null);
  const [dragging, setDragging] = useState(false);
  // Controls, text fields and selectable answers keep their own mouse handling.
  const onButton = (e: { target: EventTarget }) =>
    (e.target as HTMLElement).closest("button, input, textarea, select, a, label, summary, .organizer, .ask__grip") !== null;
  const onHandle = (e: { target: EventTarget }) =>
    (e.target as HTMLElement).closest("[data-drag-handle]") !== null;

  return {
    dragging,
    onPointerDown: (e: PointerEvent) => {
      if (e.button === 0 && onHandle(e) && !onButton(e)) {
        const handle = (e.target as HTMLElement).closest<HTMLElement>("[data-drag-handle]")!;
        const rect = handle.getBoundingClientRect();
        start.current = { x: e.screenX, y: e.screenY, grabX: e.clientX, grabY: e.clientY,
          headerX: rect.x + rect.width / 2, headerY: rect.y + rect.height / 2 };
        e.currentTarget.setPointerCapture(e.pointerId);
      }
    },
    onPointerMove: (e: PointerEvent) => {
      const s = start.current;
      if (!s || Math.hypot(e.screenX - s.x, e.screenY - s.y) < 4) return;
      start.current = null;
      if (isTauri) {
        setDragging(true);
        void invoke("drag_window_header", { grabX: s.grabX, grabY: s.grabY, headerX: s.headerX, headerY: s.headerY })
          .catch(console.error).finally(() => setDragging(false));
      }
    },
    onPointerUp: () => {
      start.current = null;
    },
    /** Double-click puts the pill back at the top-center. */
    onDoubleClick: (e: { target: EventTarget }) => {
      if (isTauri && onHandle(e) && !onButton(e)) invoke("reset_position");
    },
  };
}

/** The native window keeps the compact pill anchored when expansion points up. */
export function useWindowAnchor() {
  const [up, setUp] = useState(!isTauri && new URLSearchParams(window.location.search).get("anchor") === "bottom");
  useEffect(() => {
    if (!isTauri) return;
    let active = true;
    let receivedEvent = false;
    const unlisten = listen<boolean>("window-anchor", (event) => {
      if (active) { receivedEvent = true; setUp(event.payload); }
    });
    void unlisten.then(() => invoke<boolean>("get_window_anchor"))
      .then((value) => { if (active && !receivedEvent) setUp(value); }).catch(console.error);
    return () => { active = false; void unlisten.then((f) => f()).catch(console.error); };
  }, []);
  return up;
}

/** Hover as seen by the Rust cursor poller (the window is click-through). */
export function useNativeHover(onChange: (hover: boolean) => void) {
  useEffect(() => {
    if (!isTauri) return;
    const unlisten = listen<boolean>("hover", (e) => onChange(e.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, [onChange]);
}

/** Tells Rust where the pill is, so only that area catches the mouse. */
export function useHitRegion(ref: RefObject<HTMLElement | null>, expandUp = false) {
  useEffect(() => {
    const el = ref.current;
    if (!isTauri || !el) return;
    // Layout size jumps straight to the final size (motion animates with
    // transforms), which is what we want for hit-testing.
    const report = () => {
      const pad = 6;
      invoke("set_hit_region", {
        rect: {
          x: el.offsetLeft - pad,
          y: el.offsetTop - pad,
          width: el.offsetWidth + pad * 2,
          height: el.offsetHeight + pad * 2,
        },
      });
    };
    const ro = new ResizeObserver(report);
    ro.observe(el);
    window.addEventListener("resize", report);
    report();
    return () => { ro.disconnect(); window.removeEventListener("resize", report); };
  }, [ref, expandUp]);
}

export function useClock() {
  const [now, setNow] = useState(() => {
    const value = new Date();
    if (demo) value.setHours(14, 30, 0, 0);
    return value;
  });
  useEffect(() => {
    if (demo) return;
    const id = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(id);
  }, []);
  return now;
}

export function appName(appId: string) {
  const id = appId.toLowerCase();
  if (id.includes("spotify")) return "Spotify";
  if (id.includes("chrome")) return "Chrome";
  if (id.includes("msedge")) return "Edge";
  if (id.includes("firefox") || id === "308046b0af4a39cb") return "Firefox";
  if (id.includes("zunemusic") || id.includes("media.player")) return "Media Player";
  return appId.replace(/\.exe$/i, "").split(/[.!\\]/).pop() || appId;
}
