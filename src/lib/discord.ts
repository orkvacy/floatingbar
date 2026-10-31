import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { demo, isTauri } from "./native";

export type DiscordUser = {
  id: string;
  name: string;
  avatar: string | null;
  volume: number; // 0–1
  isMuted: boolean;
  isSpeaking: boolean;
};

export type DiscordState = {
  status: string;
  error: string | null;
  needsConfig: boolean;
  connected: boolean;
  inVoice: boolean;
  channelId: string | null;
  channelName: string;
  guildId: string | null;
  selfMuted: boolean;
  selfDeafened: boolean;
  users: DiscordUser[];
};

const initialState: DiscordState = {
  status: "",
  error: null,
  needsConfig: false,
  connected: false,
  inVoice: false,
  channelId: null,
  channelName: "",
  guildId: null,
  selfMuted: false,
  selfDeafened: false,
  users: [],
};

export const DEMO_DISCORD_USERS: DiscordUser[] = [
  { id: "demo-1", name: "Alex", avatar: null, volume: 0.8, isMuted: false, isSpeaking: true },
  { id: "demo-2", name: "Mika", avatar: null, volume: 0.6, isMuted: false, isSpeaking: false },
  { id: "demo-3", name: "Sam", avatar: null, volume: 1, isMuted: true, isSpeaking: false },
  { id: "demo-4", name: "Taylor", avatar: null, volume: 0.5, isMuted: false, isSpeaking: false },
  { id: "demo-5", name: "Jordan", avatar: null, volume: 0.9, isMuted: false, isSpeaking: true },
  { id: "demo-6", name: "Robin", avatar: null, volume: 0.7, isMuted: false, isSpeaking: false },
];
const discordDemo = demo === "discord" || demo === "discord-compact";

export function useDiscord() {
  const [state, setState] = useState<DiscordState>(discordDemo ? { ...initialState, connected: true, inVoice: true, channelName: "Study group", users: DEMO_DISCORD_USERS } : initialState);
  const [error, setError] = useState<string | null>(null);
  const esRef = useRef<EventSource | null>(null);
  const volumeTimers = useRef(new Map<string, ReturnType<typeof setTimeout>>());

  useEffect(() => () => {
    volumeTimers.current.forEach(clearTimeout);
    volumeTimers.current.clear();
  }, []);

  useEffect(() => {
    volumeTimers.current.forEach(clearTimeout);
    volumeTimers.current.clear();
  }, [state.channelId]);

  const reportError = useCallback((err: unknown) => setError(String(err)), []);

  useEffect(() => {
    // Screenshot/demo pages must not open a real Discord authorization flow.
    if (demo) return;
    let active = true;
    if (isTauri) {
      let receivedEvent = false;
      const unlisten = listen<DiscordState>("discord-state", (e) => {
        if (!active) return;
        receivedEvent = true;
        setState(e.payload);
        setError(e.payload.error ?? null);
      });
      void unlisten.then(() => invoke<DiscordState>("get_discord_state"))
        .then((s) => {
          if (active && !receivedEvent && s) { setState(s); setError(s.error ?? null); }
        }).catch(reportError);

      return () => {
        active = false;
        void unlisten.then((f) => f()).catch(console.error);
      };
    }

    let retryTimer: ReturnType<typeof setTimeout> | undefined;

    function connectSSE() {
      if (!active) return;
      try {
        const es = new EventSource("/api/discord/events");
        esRef.current = es;

        es.onmessage = (e) => {
          if (!active) return;
          try {
            const data: DiscordState = JSON.parse(e.data);
            setState(data);
            setError(data.error ?? null);
          } catch (err) {
            console.error("Failed to parse discord SSE data", err);
          }
        };

        es.onerror = () => {
          es.close();
          if (active) setState(initialState);
          // Retry connection after 3s
          if (active) {
            retryTimer = setTimeout(connectSSE, 3000);
          }
        };
      } catch (err) {
        if (active) retryTimer = setTimeout(connectSSE, 3000);
      }
    }

    connectSSE();

    return () => {
      active = false;
      clearTimeout(retryTimer);
      if (esRef.current) {
        esRef.current.close();
      }
    };
  }, [reportError]);

  const setMute = useCallback((mute: boolean) => {
    if (demo) { setState((s) => ({ ...s, selfMuted: mute })); return; }
    if (isTauri) {
      setError(null);
      invoke("discord_set_mute", { mute }).catch(reportError);
    } else {
      fetch("/api/discord/mute", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ mute }),
      }).catch(console.error);
    }
  }, [reportError]);

  const setDeaf = useCallback((deaf: boolean) => {
    if (demo) { setState((s) => ({ ...s, selfDeafened: deaf, selfMuted: deaf || s.selfMuted })); return; }
    if (isTauri) {
      setError(null);
      invoke("discord_set_deaf", { deaf }).catch(reportError);
    } else {
      fetch("/api/discord/deaf", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ deaf }),
      }).catch(console.error);
    }
  }, [reportError]);

  const setUserVolume = useCallback((userId: string, volume: number) => {
    const next = Math.max(0, Math.min(1, volume));
    // Show the drag immediately; send only the latest value after it settles.
    setState((s) => ({ ...s, users: s.users.map((u) =>
      u.id === userId ? { ...u, volume: next } : u) }));
    if (demo) return;
    clearTimeout(volumeTimers.current.get(userId));
    volumeTimers.current.set(userId, setTimeout(() => {
      volumeTimers.current.delete(userId);
      if (isTauri) {
        setError(null);
        invoke("discord_set_user_volume", { userId, volume: next }).catch(reportError);
      } else {
        fetch("/api/discord/user-volume", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ userId, volume: next }),
        }).catch(console.error);
      }
    }, 100));
  }, [reportError]);

  const toggleUserMute = useCallback((userId: string, isCurrentlyMuted: boolean) => {
    if (demo) { setState((s) => ({ ...s, users: s.users.map((u) => u.id === userId ? { ...u, isMuted: !isCurrentlyMuted } : u) })); return; }
    if (isTauri) {
      setError(null);
      invoke("discord_toggle_user_mute", { userId, mute: !isCurrentlyMuted }).catch(reportError);
    } else {
      fetch("/api/discord/user-mute", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ userId, mute: !isCurrentlyMuted }),
      }).catch(console.error);
    }
  }, [reportError]);

  const disconnect = useCallback(() => {
    if (demo) { setState(initialState); return; }
    if (isTauri) {
      setError(null);
      invoke("discord_disconnect").catch(reportError);
    } else {
      fetch("/api/discord/disconnect", {
        method: "POST",
      }).catch(console.error);
    }
  }, [reportError]);

  return {
    ...state,
    error,
    setMute,
    setDeaf,
    setUserVolume,
    toggleUserMute,
    disconnect,
    reconnect(resetAuth = false) {
      if (!isTauri) { setError("Sambungkan ulang melalui aplikasi desktop FloatingBar."); return; }
      setError(null);
      invoke("discord_reconnect", { resetAuth }).catch(reportError);
    },
    async configure(clientId: string, clientSecret: string) {
      if (!isTauri) { setError("Simpan kredensial melalui aplikasi desktop FloatingBar."); return false; }
      setError(null);
      try { await invoke("discord_configure", { clientId, clientSecret }); return true; }
      catch (err) { reportError(err); return false; }
    },
  };
}
