import type { SVGProps } from "react";

const base = { width: 20, height: 20, viewBox: "0 0 24 24", fill: "currentColor" };

export const PlayIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base} {...p}>
    <path d="M8 5.14v13.72a1 1 0 0 0 1.52.85l11.1-6.86a1 1 0 0 0 0-1.7L9.52 4.29A1 1 0 0 0 8 5.14Z" />
  </svg>
);

export const PauseIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base} {...p}>
    <rect x="6" y="4.5" width="4" height="15" rx="1.2" />
    <rect x="14" y="4.5" width="4" height="15" rx="1.2" />
  </svg>
);

export const NextIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base} {...p}>
    <path d="M3 6.6v10.8a.8.8 0 0 0 1.23.67L12 13v4.4a.8.8 0 0 0 1.23.67l8.4-5.4a.8.8 0 0 0 0-1.34l-8.4-5.4A.8.8 0 0 0 12 6.6V11L4.23 5.93A.8.8 0 0 0 3 6.6Z" />
  </svg>
);

export const PrevIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base} {...p} style={{ transform: "scaleX(-1)", ...p.style }}>
    <path d="M3 6.6v10.8a.8.8 0 0 0 1.23.67L12 13v4.4a.8.8 0 0 0 1.23.67l8.4-5.4a.8.8 0 0 0 0-1.34l-8.4-5.4A.8.8 0 0 0 12 6.6V11L4.23 5.93A.8.8 0 0 0 3 6.6Z" />
  </svg>
);

export const NoteIcon = (p: SVGProps<SVGSVGElement>) => (
  <svg {...base} {...p}>
    <path d="M19 3.5v11.25a3.25 3.25 0 1 1-2-3V7.4l-8 1.6v7.75a3.25 3.25 0 1 1-2-3V6.2a1 1 0 0 1 .8-.98l10-2A1 1 0 0 1 19 3.5Z" />
  </svg>
);

export function SpeakerIcon({ level, muted }: { level: number; muted: boolean }) {
  return (
    <svg {...base} width={18} height={18} fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round">
      <path d="M4 9.5h3.2L12 5.5v13l-4.8-4H4a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1Z" fill="currentColor" stroke="none" />
      {muted || level === 0 ? (
        <path d="m16 9.5 5 5m0-5-5 5" />
      ) : (
        <>
          <path d="M15.5 9.2a4 4 0 0 1 0 5.6" />
          {level > 0.5 && <path d="M18.4 6.6a7.6 7.6 0 0 1 0 10.8" />}
        </>
      )}
    </svg>
  );
}

export function BatteryIcon({ percent, plugged }: { percent: number; plugged: boolean }) {
  const color = plugged ? "var(--green)" : percent <= 20 ? "var(--red)" : "var(--fg)";
  return (
    <svg width="26" height="13" viewBox="0 0 26 13" aria-hidden>
      <rect x="0.5" y="0.5" width="22" height="12" rx="3.5" fill="none" stroke="var(--muted)" />
      <rect x="23.5" y="4" width="2" height="5" rx="1" fill="var(--muted)" />
      <rect x="2" y="2" width={Math.max(1.5, (19 * percent) / 100)} height="9" rx="2" fill={color} />
      {plugged && (
        <path d="M12.6 1.8 8.4 7.1h3l-1 4.1 4.2-5.3h-3l1-4.1Z" fill="#000" stroke="#000" strokeWidth="0.4" />
      )}
    </svg>
  );
}
