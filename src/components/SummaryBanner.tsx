import { formatBytes } from "../format";
import type { Summary } from "../types";
import { IconAlert, IconCheck, IconReveal, IconX } from "./Icons";

interface Props {
  summary: Summary;
  onReveal: (path: string) => void;
  onDismiss: () => void;
}

export function SummaryBanner({ summary, onReveal, onDismiss }: Props) {
  const failed = summary.failed > 0;
  const cancelled = summary.cancelled;
  const tone = failed ? "warn" : cancelled ? "muted" : "ok";

  const title = cancelled
    ? `Cancelled after ${summary.converted} ${summary.converted === 1 ? "image" : "images"}`
    : failed
      ? `${summary.converted} converted, ${summary.failed} failed`
      : `${summary.converted} ${summary.converted === 1 ? "image" : "images"} converted`;

  const detail = cancelled
    ? "The images already written are safe."
    : `${formatBytes(summary.outputBytes)} written${
        summary.inputBytes > 0 && summary.outputBytes > 0
          ? ` · from ${formatBytes(summary.inputBytes)}`
          : ""
      }${failed ? " · hover a failed row for the reason" : ""}`;

  return (
    <div className={`banner ${tone}`} role="status">
      <div className="banner-icon">{failed ? <IconAlert size={18} /> : <IconCheck size={18} />}</div>
      <div className="banner-text">
        <strong>{title}</strong>
        <span>{detail}</span>
      </div>
      {summary.firstOutput && (
        <button className="btn small" onClick={() => onReveal(summary.firstOutput!)}>
          <IconReveal size={14} />
          Show results
        </button>
      )}
      <button className="icon-btn" onClick={onDismiss} title="Dismiss">
        <IconX size={15} />
      </button>
    </div>
  );
}
