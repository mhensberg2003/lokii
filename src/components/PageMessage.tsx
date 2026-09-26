import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { Button } from "./ui/Button";
import styles from "./PageMessage.module.css";

type PageMessageProps = {
  icon: LucideIcon;
  title: string;
  children?: ReactNode;
  onRetry?: () => void;
};

/** Centered message for empty pages and load errors. */
export function PageMessage({ icon: Icon, title, children, onRetry }: PageMessageProps) {
  return (
    <div className={styles.message} role={onRetry ? "alert" : undefined}>
      <Icon className={styles.icon} strokeWidth={1.5} />
      <h2 className={styles.title}>{title}</h2>
      {children && <p className={styles.body}>{children}</p>}
      {onRetry && (
        <Button variant="secondary" onClick={onRetry}>
          Try again
        </Button>
      )}
    </div>
  );
}

export function errorText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Something went wrong.";
}
