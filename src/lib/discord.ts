import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "./native";

export type DiscordUser = {
  id: string;
  name: string;
  avatar: string | null;
  volume: number; // 0–1
  isMuted: boolean;
  isSpeaking: boolean;
};

export type DiscordState = {
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
  connected: false,
  inVoice: false,
  channelId: null,
  channelName: "",
  guildId: null,
  selfMuted: false,
  selfDeafened: false,
  users: [],
};

export function useDiscord() {
  const [state, setState] = useState<DiscordState>(initialState);
  const esRef = useRef<EventSource | null>(null);

  useEffect(() => {
    if (isTauri) {
      invoke<DiscordState>("get_discord_state")
        .then((s) => {
          if (s) setState(s);
        })
        .catch(console.error);

      const unlisten = listen<DiscordState>("discord-state", (e) => {
        setState(e.payload);
      });

      return () => {
        unlisten.then((f) => f());
      };
    }

    let active = true;

    function connectSSE() {
      try {
        const es = new EventSource("/api/discord/events");
        esRef.current = es;

        es.onmessage = (e) => {
          if (!active) return;
          try {
            const data: DiscordState = JSON.parse(e.data);
            setState(data);
          } catch (err) {
            console.error("Failed to parse discord SSE data", err);
          }
        };

        es.onerror = () => {
          es.close();
          // Retry connection after 3s
          if (active) {
            setTimeout(connectSSE, 3000);
          }
        };
      } catch (err) {
        if (active) setTimeout(connectSSE, 3000);
      }
    }

    connectSSE();

    return () => {
      active = false;
      if (esRef.current) {
        esRef.current.close();
      }
    };
  }, []);

  const setMute = useCallback((mute: boolean) => {
    if (isTauri) {
      invoke("discord_set_mute", { mute }).catch(console.error);
    } else {
      fetch("/api/discord/mute", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ mute }),
      }).catch(console.error);
    }
  }, []);

  const setDeaf = useCallback((deaf: boolean) => {
    if (isTauri) {
      invoke("discord_set_deaf", { deaf }).catch(console.error);
    } else {
      fetch("/api/discord/deaf", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ deaf }),
      }).catch(console.error);
    }
  }, []);

  const setUserVolume = useCallback((userId: string, volume: number) => {
    if (isTauri) {
      invoke("discord_set_user_volume", { userId, volume }).catch(console.error);
    } else {
      fetch("/api/discord/user-volume", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ userId, volume }),
      }).catch(console.error);
    }
  }, []);

  const toggleUserMute = useCallback((userId: string, isCurrentlyMuted: boolean) => {
    if (isTauri) {
      invoke("discord_toggle_user_mute", { userId, mute: !isCurrentlyMuted }).catch(console.error);
    } else {
      fetch("/api/discord/user-mute", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ userId, mute: !isCurrentlyMuted }),
      }).catch(console.error);
    }
  }, []);

  const disconnect = useCallback(() => {
    if (isTauri) {
      invoke("discord_disconnect").catch(console.error);
    } else {
      fetch("/api/discord/disconnect", {
        method: "POST",
      }).catch(console.error);
    }
  }, []);

  return {
    ...state,
    setMute,
    setDeaf,
    setUserVolume,
    toggleUserMute,
    disconnect,
  };
}
