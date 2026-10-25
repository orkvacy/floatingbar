import { AnimatePresence, motion } from "motion/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { BatteryIcon, NextIcon, NoteIcon, PauseIcon, PlayIcon, PrevIcon, SpeakerIcon } from "./icons";
import {
  appName,
  demo,
  useBattery,
  useClock,
  useDragWindow,
  useHitRegion,
  useMedia,
  useNativeHover,
  shortcutIcon,
  MAX_EXTRA,
  useAsk,
  useNotifications,
  usePermissions,
  useShortcuts,
  useVolume,
  type BatteryInfo,
  type MediaInfo,
  type NotificationInfo,
  type PermissionDecision,
  type PermissionRequest,
  type ReceivedNotification,
  type Shortcut,
  type VolumeInfo,
} from "./lib/native";

const spring = { type: "spring", stiffness: 420, damping: 32, mass: 0.9 } as const;

// Pages of the open island, left to right; swiping moves between them.
const PAGES = [
  { id: "notifications", label: "Notifikasi" },
  { id: "media", label: "Musik" },
  { id: "claude", label: "Claude" },
] as const;
const HOME_PAGE = 1;
/** Horizontal scroll needed for one page turn (touchpad px). */
const SWIPE_DISTANCE = 40;
const slide = {
  enter: (dir: number) => ({ x: dir * 60, opacity: 0 }),
  center: { x: 0, opacity: 1 },
  exit: (dir: number) => ({ x: -dir * 60, opacity: 0 }),
};
const PEEK_MS = 2800;
const NOTIFICATION_MS = 6000;
const fade = {
  initial: { opacity: 0, scale: 0.92, filter: "blur(6px)" },
  animate: { opacity: 1, scale: 1, filter: "blur(0px)" },
  exit: { opacity: 0, scale: 0.92, filter: "blur(6px)" },
  transition: { duration: 0.22 },
};

export default function App() {
  const { media, playPause, next, previous } = useMedia();
  const battery = useBattery();
  const now = useClock();
  const {
    notification,
    dismiss,
    history,
    remove: removeNotification,
    clear: clearNotifications,
    error: notifyError,
  } = useNotifications();
  const { permission, waiting, answer } = usePermissions();
  const drag = useDragWindow();
  const { volume, setLevel, nudge, setMuted } = useVolume(media !== null, media?.appId);
  const pillRef = useRef<HTMLDivElement>(null);

  // Collapse a little after the cursor leaves, so brushing the edge
  // doesn't make the island flicker.
  const [hover, setHoverNow] = useState(false);
  const leaveTimer = useRef<number | undefined>(undefined);
  const setHover = useCallback((h: boolean) => {
    window.clearTimeout(leaveTimer.current);
    if (h) setHoverNow(true);
    else leaveTimer.current = window.setTimeout(() => setHoverNow(false), 180);
  }, []);
  useNativeHover(setHover);
  useHitRegion(pillRef);

  // Briefly open by itself when the charger is plugged in. (Not on track
  // changes: the compact artwork already shows the new song.)
  const [peek, setPeek] = useState(false);
  const plugged = battery?.plugged ?? false;
  const wasPlugged = useRef(plugged);
  useEffect(() => {
    const pluggedIn = plugged && !wasPlugged.current;
    wasPlugged.current = plugged;
    if (!pluggedIn) return;
    setPeek(true);
    const id = window.setTimeout(() => setPeek(false), PEEK_MS);
    return () => window.clearTimeout(id);
  }, [plugged]);

  // A toast stays while the cursor is on it, then fades after a few seconds.
  useEffect(() => {
    if (!notification || hover) return;
    const id = window.setTimeout(dismiss, NOTIFICATION_MS);
    return () => window.clearTimeout(id);
  }, [notification, hover, dismiss]);

  // Claude's own toast for a request already shown here would just repeat it.
  useEffect(() => {
    if (permission && notification?.app === "Claude") dismiss();
  }, [permission, notification, dismiss]);

  // Stay open while typing a prompt, even after the cursor wanders off.
  // (The select counts as typing: its dropdown takes focus.)
  const ask = useAsk();
  const [typing, setTyping] = useState(false);
  // Also stay open while a reply streams in, so it can be read.
  const pinned = typing || ask.busy;

  const mode = permission
    ? "permission"
    : notification
      ? "notification"
      : hover || peek || pinned
        ? "open"
        : "compact";
  const expanded = mode !== "compact";
  const { shortcuts, open: openShortcut, edit: editShortcuts } = useShortcuts(mode === "open");

  // Which page is showing, and which way the last turn went (for the slide).
  const [pager, setPager] = useState({ index: HOME_PAGE, dir: 0 });
  const turn = useCallback((delta: number) => {
    setPager((p) => {
      const index = Math.min(PAGES.length - 1, Math.max(0, p.index + delta));
      return index === p.index ? p : { index, dir: delta };
    });
  }, []);
  const page = PAGES[pager.index].id;

  // Demo states (browser only) open the pill on a given page for screenshots.
  useEffect(() => {
    if (!demo) return;
    document.body.classList.add("demo");
    if (demo !== "compact") setHoverNow(true);
    const demoPage: Record<string, number> = { notifications: 0, music: 1, claude: 2 };
    if (demo in demoPage) setPager({ index: demoPage[demo], dir: 0 });
  }, []);

  // A touchpad swipe arrives as many small horizontal scroll events: add them
  // up, turn once, then ignore the rest of that gesture.
  const swipe = useRef({ distance: 0, last: 0, lockedUntil: 0 });
  const onWheel = (e: React.WheelEvent) => {
    const horizontal = e.shiftKey || Math.abs(e.deltaX) > Math.abs(e.deltaY);
    if (horizontal && mode === "open") {
      const s = swipe.current;
      const t = performance.now();
      if (t - s.last > 250) s.distance = 0;
      s.last = t;
      if (t < s.lockedUntil) return;
      s.distance += e.shiftKey ? e.deltaY : e.deltaX;
      if (Math.abs(s.distance) >= SWIPE_DISTANCE) {
        turn(s.distance > 0 ? 1 : -1);
        s.distance = 0;
        s.lockedUntil = t + 450;
      }
      return;
    }
    // Scrolling up and down nudges the volume, like a hardware knob.
    if (!horizontal && volume && (mode !== "open" || page === "media")) {
      nudge(e.deltaY < 0 ? 0.05 : -0.05);
    }
  };

  return (
    <div className="stage">
      <motion.div
        ref={pillRef}
        layout
        transition={spring}
        className={`pill ${expanded ? "pill--open" : ""}`}
        style={{ borderRadius: expanded ? 30 : 20 }}
        onMouseEnter={() => setHover(true)}
        onMouseLeave={() => setHover(false)}
        onWheel={onWheel}
        {...drag}
      >
        <AnimatePresence mode="popLayout" initial={false}>
          {mode === "permission" && permission ? (
            <motion.div key={`p${permission.id}`} className="open" {...fade}>
              <Permission request={permission} waiting={waiting} onAnswer={answer} />
            </motion.div>
          ) : mode === "notification" && notification ? (
            <motion.div key={`n${notification.id}`} className="open" {...fade}>
              <Notification notification={notification} onDismiss={dismiss} />
            </motion.div>
          ) : mode === "open" ? (
            <motion.div key="open" className="open" {...fade}>
              <Header now={now} battery={battery}>
                <Shortcuts shortcuts={shortcuts} onOpen={openShortcut} onEdit={editShortcuts} />
              </Header>
              {notifyError && <p className="hint">{notifyError}</p>}
              <AnimatePresence mode="popLayout" initial={false} custom={pager.dir}>
                <motion.div
                  key={page}
                  custom={pager.dir}
                  variants={slide}
                  initial="enter"
                  animate="center"
                  exit="exit"
                  transition={{ duration: 0.22, ease: "easeOut" }}
                >
                  {page === "notifications" ? (
                    <NotificationsPage
                      history={history}
                      now={now}
                      onRemove={removeNotification}
                      onClear={clearNotifications}
                    />
                  ) : page === "claude" ? (
                    <ClaudePage ask={ask} onTyping={setTyping} />
                  ) : media ? (
                    <Player media={media} onPlayPause={playPause} onNext={next} onPrevious={previous}>
                      {volume && (
                        <Volume
                          volume={volume}
                          app={appName(media.appId)}
                          onLevel={setLevel}
                          onMuted={setMuted}
                        />
                      )}
                    </Player>
                  ) : (
                    <p className="empty">Tidak ada media yang diputar</p>
                  )}
                </motion.div>
              </AnimatePresence>
              <div className="dots">
                {PAGES.map((p, i) => (
                  <button
                    key={p.id}
                    className={i === pager.index ? "dots__on" : ""}
                    onClick={() => turn(i - pager.index)}
                    title={p.label}
                    aria-label={p.label}
                  />
                ))}
              </div>
            </motion.div>
          ) : (
            <motion.div
              key="compact"
              className="compact"
              initial={{ opacity: 0, filter: "blur(4px)" }}
              animate={{ opacity: 1, filter: "blur(0px)" }}
              exit={{ opacity: 0, filter: "blur(4px)" }}
              transition={{ duration: 0.18 }}
            >
              <Compact media={media} now={now} battery={battery} />
            </motion.div>
          )}
        </AnimatePresence>
      </motion.div>
    </div>
  );
}

function Compact({ media, now, battery }: { media: MediaInfo | null; now: Date; battery: BatteryInfo | null }) {
  if (media) {
    return (
      <>
        <Artwork src={media.thumbnail} size={22} radius={6} />
        <Equalizer playing={media.playing} />
      </>
    );
  }
  return (
    <>
      <span className="time">{formatClock(now)}</span>
      <Battery battery={battery} />
    </>
  );
}

function NotificationsPage({
  history,
  now,
  onRemove,
  onClear,
}: {
  history: ReceivedNotification[];
  now: Date;
  onRemove: (id: number) => void;
  onClear: () => void;
}) {
  if (history.length === 0) return <p className="empty">Belum ada notifikasi</p>;
  return (
    <div className="history">
      <div className="history__head">
        <span>{history.length} notifikasi</span>
        <button onClick={onClear}>Hapus semua</button>
      </div>
      <div className="history__list">
        {history.map((n) => (
          <button key={`${n.id}-${n.receivedAt}`} className="history__item" onClick={() => onRemove(n.id)} title="Hapus">
            <NotificationIcon notification={n} size={28} />
            <span className="meta">
              <span className="history__top">
                <span className="notification__app">{n.app || "Notifikasi"}</span>
                <span className="notification__app">{timeAgo(n.receivedAt, now)}</span>
              </span>
              <span className="history__text">{[n.title, n.body].filter(Boolean).join(" — ")}</span>
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}

function ClaudePage({ ask, onTyping }: { ask: ReturnType<typeof useAsk>; onTyping: (typing: boolean) => void }) {
  // Read the app's sidebar and the open chat each time the page opens.
  const { refresh } = ask;
  useEffect(() => refresh(true), [refresh]);
  const replyBox = useRef<HTMLDivElement>(null);
  useEffect(() => {
    replyBox.current?.scrollTo({ top: replyBox.current.scrollHeight });
  }, [ask.history]);
  // With messages showing, most of the extra room goes to reading them.
  const showing = ask.history !== null && (ask.history.messages.length > 0 || ask.history.working);
  const replyExtra = showing ? Math.round(ask.extra * 0.6) : 0;

  // A pasted image is kept as a preview; on send it's pasted into the app.
  const onPaste = (e: React.ClipboardEvent) => {
    const file = [...e.clipboardData.items].find((i) => i.type.startsWith("image/"))?.getAsFile();
    if (!file) return;
    e.preventDefault();
    const reader = new FileReader();
    reader.onload = () => ask.setImage(String(reader.result));
    reader.readAsDataURL(file);
  };

  return (
    <div className="ask">
      <div className="ask__head">
        <div className="ask__modes" role="radiogroup" aria-label="Mode Claude">
          {(["chat", "code"] as const).map((m) => (
            <button
              key={m}
              role="radio"
              aria-checked={ask.mode === m}
              className={ask.mode === m ? "ask__mode--on" : ""}
              onClick={() => ask.setMode(m)}
            >
              {m === "chat" ? "Chat" : "Code"}
            </button>
          ))}
        </div>
        <select
          className="ask__chat"
          value={ask.chat ?? ""}
          onChange={(e) => ask.selectChat(e.target.value || null)}
          onFocus={() => onTyping(true)}
          onBlur={() => onTyping(false)}
          aria-label="Pilih percakapan"
        >
          <option value="">＋ {ask.mode === "code" ? "Sesi baru" : "Chat baru"}</option>
          {ask.chats.map((c) => (
            <option key={c} value={c}>
              {c === ask.current ? `● ${c} (terbuka)` : c}
            </option>
          ))}
        </select>
        <button
          className="ask__expand"
          onClick={() => ask.refresh(true)}
          title="Muat ulang daftar"
          aria-label="Muat ulang daftar"
        >
          ⟳
        </button>
        <button
          className="ask__expand"
          onClick={ask.toggleTall}
          title={ask.extra > 0 ? "Perkecil" : "Perluas"}
          aria-label={ask.extra > 0 ? "Perkecil halaman Claude" : "Perluas halaman Claude"}
        >
          {ask.extra > 0 ? "⤡" : "⤢"}
        </button>
      </div>
      {ask.listError ? (
        <p className="ask__status ask__status--error">
          {ask.listError}.{" "}
          <button className="ask__link" onClick={ask.openInApp}>
            Buka aplikasi Claude ↗
          </button>
        </p>
      ) : ask.history?.error ? (
        <p className="ask__status ask__status--error">{ask.history.error}</p>
      ) : showing ? (
        <div className="ask__reply" style={{ maxHeight: 96 + replyExtra }} ref={replyBox}>
          {ask.history!.messages.map((m, i) => (
            <p key={i} className={`ask__text ask__text--${m.role}`}>
              {m.text}
            </p>
          ))}
          {ask.history!.working && <p className="ask__status ask__typing">Claude sedang bekerja…</p>}
        </div>
      ) : (
        <p className="ask__status">Prompt dikirim lewat aplikasi Claude. Enter kirim, Shift+Enter baris baru.</p>
      )}
      {ask.image && (
        <div className="ask__attach">
          <img src={ask.image} alt="Gambar yang ditempel" />
          <span>Gambar ikut dikirim</span>
          <button onClick={() => ask.setImage(null)} aria-label="Hapus gambar">
            ✕
          </button>
        </div>
      )}
      <div className="ask__row">
        <textarea
          className="ask__input"
          style={{ height: 72 + (ask.extra - replyExtra) }}
          rows={3}
          placeholder="Tanya Claude…"
          value={ask.draft}
          onChange={(e) => ask.setDraft(e.target.value)}
          onPaste={onPaste}
          onFocus={() => onTyping(true)}
          onBlur={() => onTyping(false)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              ask.send();
            }
          }}
        />
        <button
          className="ask__send"
          onClick={ask.send}
          disabled={!ask.draft.trim() || ask.busy}
          title="Kirim ke Claude"
        >
          ↑
        </button>
      </div>
      <ResizeGrip extra={ask.extra} onResize={ask.setExtra} onActive={onTyping} />
    </div>
  );
}

/** Drag handle under the Claude page: pull down for more room, up for less. */
function ResizeGrip({
  extra,
  onResize,
  onActive,
}: {
  extra: number;
  onResize: (px: number) => void;
  onActive: (active: boolean) => void;
}) {
  const start = useRef<{ y: number; extra: number } | null>(null);
  return (
    <div
      className="ask__grip"
      role="separator"
      aria-orientation="horizontal"
      aria-label="Ubah tinggi halaman Claude"
      aria-valuemin={0}
      aria-valuemax={MAX_EXTRA}
      aria-valuenow={extra}
      title="Tarik untuk mengubah tinggi"
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId);
        start.current = { y: e.clientY, extra };
        // Keep the pill open even if the pointer slips past its edge.
        onActive(true);
      }}
      onPointerMove={(e) => {
        if (start.current) onResize(start.current.extra + e.clientY - start.current.y);
      }}
      onPointerUp={() => {
        start.current = null;
        onActive(false);
      }}
      onDoubleClick={() => onResize(0)}
    >
      <span />
    </div>
  );
}

function timeAgo(at: number, now: Date) {
  const min = Math.floor((now.getTime() - at) / 60_000);
  if (min < 1) return "baru saja";
  if (min < 60) return `${min} mnt`;
  return `${Math.floor(min / 60)} jam`;
}

function Permission({
  request,
  waiting,
  onAnswer,
}: {
  request: PermissionRequest;
  waiting: number;
  onAnswer: (id: number, decision: PermissionDecision) => void;
}) {
  return (
    <div className="permission">
      <div className="permission__head">
        <span className="permission__badge">Claude minta izin</span>
        <span className="notification__app">
          {request.tool}
          {request.project && ` · ${request.project}`}
          {waiting > 1 && ` · ${waiting - 1} lagi`}
        </span>
      </div>
      <code className="permission__summary">{request.summary}</code>
      <div className="permission__actions">
        <button className="permission__ask" onClick={() => onAnswer(request.id, "ask")}>
          Buka di Claude
        </button>
        <button className="permission__deny" onClick={() => onAnswer(request.id, "deny")}>
          Tolak
        </button>
        <button className="permission__allow" onClick={() => onAnswer(request.id, "allow")}>
          Izinkan
        </button>
      </div>
    </div>
  );
}

function Notification({ notification, onDismiss }: { notification: NotificationInfo; onDismiss: () => void }) {
  return (
    <div className="notification" onClick={onDismiss}>
      <NotificationIcon notification={notification} size={40} />
      <div className="meta">
        <span className="notification__app">{notification.app || "Notifikasi"}</span>
        {notification.title && <span className="title">{notification.title}</span>}
        {notification.body && <span className="notification__body">{notification.body}</span>}
      </div>
    </div>
  );
}

function NotificationIcon({ notification, size }: { notification: NotificationInfo; size: number }) {
  const style = { width: size, height: size, borderRadius: size / 4, fontSize: size * 0.45 };
  if (notification.icon) {
    return <img className="notification__icon" src={notification.icon} style={style} alt="" draggable={false} />;
  }
  return (
    <span className="notification__icon notification__icon--empty" style={style}>
      {(notification.app || "?").charAt(0).toUpperCase()}
    </span>
  );
}

function Header({
  now,
  battery,
  children,
}: {
  now: Date;
  battery: BatteryInfo | null;
  children?: React.ReactNode;
}) {
  return (
    <div className="header">
      <div className="header__clock">
        <span className="time time--big">{formatClock(now)}</span>
        <span className="date">
          {now.toLocaleDateString("id-ID", { weekday: "short", day: "numeric", month: "short" })}
        </span>
      </div>
      {children}
      <Battery battery={battery} showPercent />
    </div>
  );
}

function Player({
  media,
  onPlayPause,
  onNext,
  onPrevious,
  children,
}: {
  media: MediaInfo;
  onPlayPause: () => void;
  onNext: () => void;
  onPrevious: () => void;
  children?: React.ReactNode;
}) {
  const progress = media.durationMs > 0 ? (media.positionMs / media.durationMs) * 100 : 0;
  return (
    <div className="player">
      <div className="track">
        <Artwork src={media.thumbnail} size={56} radius={14} />
        <div className="meta">
          <span className="title">{media.title}</span>
          <span className="artist">
            {[media.artist, appName(media.appId)].filter(Boolean).join(" · ")}
          </span>
        </div>
        <Equalizer playing={media.playing} />
      </div>

      {media.durationMs > 0 && (
        <div className="progress">
          <span>{formatDuration(media.positionMs)}</span>
          <div className="bar">
            <div className="bar__fill" style={{ width: `${progress}%` }} />
          </div>
          <span>-{formatDuration(media.durationMs - media.positionMs)}</span>
        </div>
      )}

      <div className="controls">
        <button onClick={onPrevious} aria-label="Sebelumnya">
          <PrevIcon />
        </button>
        <button className="controls__main" onClick={onPlayPause} aria-label={media.playing ? "Jeda" : "Putar"}>
          {media.playing ? <PauseIcon width={24} height={24} /> : <PlayIcon width={24} height={24} />}
        </button>
        <button onClick={onNext} aria-label="Berikutnya">
          <NextIcon />
        </button>
        {children}
      </div>
    </div>
  );
}

function Shortcuts({
  shortcuts,
  onOpen,
  onEdit,
}: {
  shortcuts: Shortcut[];
  onOpen: (index: number) => void;
  onEdit: () => void;
}) {
  return (
    <div className="shortcuts">
      {shortcuts.map((s, i) => {
        const icon = shortcutIcon(s);
        return (
          <button key={`${i}-${s.target}`} onClick={() => onOpen(i)} title={s.name} aria-label={s.name}>
            {icon ? (
              <img src={icon} alt="" draggable={false} />
            ) : (
              <span>{s.name.charAt(0).toUpperCase()}</span>
            )}
          </button>
        );
      })}
      <button className="shortcuts__edit" onClick={onEdit} title="Atur shortcut" aria-label="Atur shortcut">
        +
      </button>
    </div>
  );
}

function Volume({
  volume,
  app,
  onLevel,
  onMuted,
}: {
  volume: VolumeInfo;
  app: string;
  onLevel: (level: number) => void;
  onMuted: (muted: boolean) => void;
}) {
  const level = volume.muted ? 0 : volume.level;
  return (
    <div className="volume">
      <button
        onClick={() => onMuted(!volume.muted)}
        aria-label={volume.muted ? "Bunyikan" : "Bisukan"}
        title={volume.scope === "app" ? `Volume ${app}` : "Volume sistem"}
      >
        <SpeakerIcon level={level} muted={volume.muted} />
      </button>
      <input
        type="range"
        min={0}
        max={100}
        value={Math.round(level * 100)}
        onChange={(e) => onLevel(Number(e.target.value) / 100)}
        style={{ "--fill": `${level * 100}%` } as React.CSSProperties}
        aria-label="Volume"
        title={volume.scope === "app" ? `Volume ${app}` : "Volume sistem"}
      />
    </div>
  );
}

function Artwork({ src, size, radius }: { src: string | null; size: number; radius: number }) {
  const style = { width: size, height: size, borderRadius: radius };
  if (src) return <img className="art" src={src} style={style} alt="" draggable={false} />;
  return (
    <div className="art art--empty" style={style}>
      <NoteIcon width={size * 0.55} height={size * 0.55} />
    </div>
  );
}

function Equalizer({ playing }: { playing: boolean }) {
  return (
    <div className={`eq ${playing ? "eq--on" : ""}`} aria-hidden>
      {[0, 1, 2, 3].map((i) => (
        <span key={i} style={{ animationDelay: `${i * -0.23}s` }} />
      ))}
    </div>
  );
}

function Battery({ battery, showPercent }: { battery: BatteryInfo | null; showPercent?: boolean }) {
  if (!battery?.hasBattery || battery.percent == null) return null;
  return (
    <span className="battery">
      {showPercent && <span>{battery.percent}%</span>}
      <BatteryIcon percent={battery.percent} plugged={battery.plugged} />
    </span>
  );
}

function formatClock(d: Date) {
  return d.toLocaleTimeString("id-ID", { hour: "2-digit", minute: "2-digit" }).replace(".", ":");
}

function formatDuration(ms: number) {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}
