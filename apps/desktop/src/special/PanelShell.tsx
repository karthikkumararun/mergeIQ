import type { ReactNode } from "react";
import { splitPath } from "../repo/describe";
import shell from "./Special.module.css";

export type Tone = "neutral" | "danger" | "info" | "warn" | "ok" | "purple";

const TONES: Record<Tone, string> = {
  neutral: "",
  danger: shell["tone-danger"],
  info: shell["tone-info"],
  warn: shell["tone-warn"],
  ok: shell["tone-ok"],
  purple: shell["tone-purple"],
};

export interface PanelShellProps {
  /** The conflicted path, as displayed. */
  display: string;
  badge?: string | null;
  tone?: Tone;
  /** Conflict type, e.g. "Deleted on right". */
  subtitle?: string;
  /** One line saying what happened. */
  title?: ReactNode;
  /** A short explanation under the title. */
  lead?: ReactNode;
  /** Widest the content grows, in px. */
  maxWidth?: number;
  /** Test id and accessible name of the panel. */
  label: string;
  onBack: () => void;
  children: ReactNode;
}

/** Header, title and explanation shared by every special-conflict panel. */
export function PanelShell({
  display,
  badge,
  tone = "neutral",
  subtitle,
  title,
  lead,
  maxWidth = 1180,
  label,
  onBack,
  children,
}: PanelShellProps) {
  const { dir, file } = splitPath(display);
  return (
    <section className={shell.page} aria-label={label}>
      <header className={shell.header}>
        <button type="button" className={shell.back} onClick={onBack}>
          ← Conflicts
        </button>
        <span className={shell.path}>
          {dir && <span className={shell.dir}>{dir}</span>}
          {file}
        </span>
        {badge && (
          <span className={`${shell.badge} ${TONES[tone]}`}>{badge}</span>
        )}
        {subtitle && <span className={shell.subtitle}>{subtitle}</span>}
      </header>
      <div className={shell.main} style={{ maxWidth }}>
        {(title || lead) && (
          <div className={shell.intro}>
            {title && <h1 className={shell.title}>{title}</h1>}
            {lead && <p className={shell.lead}>{lead}</p>}
          </div>
        )}
        {children}
      </div>
    </section>
  );
}
