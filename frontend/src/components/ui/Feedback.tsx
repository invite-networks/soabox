import { AlertTriangle, CheckCircle2, Inbox, Info, XCircle } from "lucide-react";
import type { ReactNode } from "react";
import { ProgressBar } from "react-aria-components";
import { errorMessage } from "../../api/client";
import { Button } from "./Button";

export type Tone = "neutral" | "accent" | "success" | "warning" | "danger";

const badgeTones: Record<Tone, string> = {
  neutral: "bg-surface-muted text-fg-muted border-border",
  accent: "bg-accent-soft text-accent-text border-transparent",
  success: "bg-success-soft text-success border-transparent",
  warning: "bg-warning-soft text-warning border-transparent",
  danger: "bg-danger-soft text-danger border-transparent",
};

/** A short status label. Pair color with text, and with an icon where it helps. */
export function Badge({ tone = "neutral", children }: { tone?: Tone; children: ReactNode }) {
  return (
    <span
      className={`inline-flex items-center gap-1 rounded border px-1.5 py-0.5 text-xs font-medium whitespace-nowrap [&_svg]:size-3.5 ${badgeTones[tone]}`}
    >
      {children}
    </span>
  );
}

const alertTones = {
  info: { classes: "border-border bg-surface-muted text-fg", icon: Info },
  success: { classes: "border-success/40 bg-success-soft text-fg", icon: CheckCircle2 },
  warning: { classes: "border-warning/40 bg-warning-soft text-fg", icon: AlertTriangle },
  danger: { classes: "border-danger/40 bg-danger-soft text-fg", icon: XCircle },
} as const;

const iconTones = {
  info: "text-fg-muted",
  success: "text-success",
  warning: "text-warning",
  danger: "text-danger",
} as const;

export interface AlertProps {
  tone?: keyof typeof alertTones;
  title?: string;
  children?: ReactNode;
}

/** An inline message. Danger alerts are announced immediately. */
export function Alert({ tone = "info", title, children }: AlertProps) {
  const { classes, icon: Icon } = alertTones[tone];
  return (
    <div
      role={tone === "danger" ? "alert" : "status"}
      className={`flex gap-2.5 rounded-md border px-3 py-2.5 text-sm ${classes}`}
    >
      <Icon aria-hidden className={`mt-0.5 size-4 shrink-0 ${iconTones[tone]}`} />
      <div className="flex min-w-0 flex-col gap-0.5">
        {title && <p className="font-medium">{title}</p>}
        {children && <div className="text-fg">{children}</div>}
      </div>
    </div>
  );
}

/** A loading indicator for a region. */
export function LoadingState({ label = "Loading" }: { label?: string }) {
  return (
    <ProgressBar isIndeterminate aria-label={label} className="flex items-center gap-2 px-3 py-6 text-sm text-fg-muted">
      <span className="size-4 animate-spin rounded-full border-2 border-border-strong border-t-accent" />
      <span>{label}</span>
    </ProgressBar>
  );
}

export interface EmptyStateProps {
  title: string;
  children?: ReactNode;
  action?: ReactNode;
}

/** Shown when a list has nothing in it. */
export function EmptyState({ title, children, action }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center gap-2 px-4 py-14 text-center">
      <span
        aria-hidden
        className="mb-1 flex size-10 items-center justify-center rounded-full bg-surface-muted text-fg-muted"
      >
        <Inbox className="size-5" />
      </span>
      <p className="font-medium text-fg">{title}</p>
      {children && <div className="max-w-md text-sm text-fg-muted">{children}</div>}
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}

export interface ErrorStateProps {
  error: unknown;
  onRetry?: () => void;
  title?: string;
}

/** Shown when a region failed to load. Users see the server's `detail` message only. */
export function ErrorState({ error, onRetry, title = "This could not be loaded" }: ErrorStateProps) {
  return (
    <div className="p-3">
      <Alert tone="danger" title={title}>
        <p>{errorMessage(error)}</p>
        {onRetry && (
          <div className="mt-2">
            <Button size="sm" onPress={onRetry}>
              Try again
            </Button>
          </div>
        )}
      </Alert>
    </div>
  );
}
