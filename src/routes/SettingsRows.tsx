import type { ReactNode } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import styles from "./Settings.module.css";

type GroupProps = { title: string; description?: string; note?: ReactNode; children: ReactNode };

export function Group({ title, description, note, children }: GroupProps) {
  return (
    <section className={styles.group} aria-label={title}>
      <header className={styles.groupHeader}>
        <h2 className={styles.groupTitle}>{title}</h2>
        {description && <p className={styles.groupDescription}>{description}</p>}
      </header>
      <div className={styles.rows}>{children}</div>
      {note && <p className={styles.note}>{note}</p>}
    </section>
  );
}

type RowProps = { name: string; description: ReactNode; children?: ReactNode; below?: ReactNode };

/** One setting: a name, a one-line description, and one action on the right. */
export function Row({ name, description, children, below }: RowProps) {
  return (
    <div className={styles.row}>
      <div className={styles.rowMain}>
        <div className={styles.rowText}>
          <span className={styles.rowName}>{name}</span>
          <span className={styles.rowDescription}>{description}</span>
        </div>
        {children && <div className={styles.rowAction}>{children}</div>}
      </div>
      {below}
    </div>
  );
}

/** Opens a web page in the default browser. The browser preview opens a new tab. */
export function openLink(url: string) {
  openUrl(url).catch(() => window.open(url, "_blank", "noopener"));
}
