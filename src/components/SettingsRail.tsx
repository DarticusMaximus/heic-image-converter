import {
  FORMAT_INFO,
  QUALITY_INFO,
  formatBytes,
  middleTruncate,
} from "../format";
import type { EstimateResult, OutputFormat, Quality, Settings } from "../types";
import {
  IconArrowRight,
  IconCheck,
  IconChevronDown,
  IconFolder,
  IconX,
  Spinner,
} from "./Icons";

export type Phase = "idle" | "converting" | "done";

const FORMATS: OutputFormat[] = ["png", "jpeg", "heic"];
const QUALITIES: Quality[] = ["smallest", "balanced", "high", "maximum"];
const RESIZE_OPTIONS = [4096, 2560, 1920, 1280, 1080];

interface Props {
  settings: Settings;
  phase: Phase;
  fileCount: number;
  inputBytes: number;
  estimate: EstimateResult | null;
  estimating: boolean;
  progress: { completed: number; total: number } | null;
  cancelling: boolean;
  actualBytes: number | null;
  actualCount: number;
  onChange: (patch: Partial<Settings>) => void;
  onPickOutput: () => void;
  onClearOutput: () => void;
  onConvert: () => void;
  onCancel: () => void;
}

export function SettingsRail({
  settings,
  phase,
  fileCount,
  inputBytes,
  estimate,
  estimating,
  progress,
  cancelling,
  actualBytes,
  actualCount,
  onChange,
  onPickOutput,
  onClearOutput,
  onConvert,
  onCancel,
}: Props) {
  const disabled = phase === "converting";
  const lossy = settings.format !== "png";
  const format = FORMAT_INFO[settings.format];

  const shownBytes =
    phase === "done" && actualBytes != null ? actualBytes : estimate?.estimatedTotalBytes ?? null;

  const percent =
    progress && progress.total > 0
      ? Math.min(100, Math.round((progress.completed / progress.total) * 100))
      : 0;

  const delta = (() => {
    if (phase === "done" || !shownBytes || !inputBytes) return null;
    if (shownBytes <= inputBytes) {
      return `${Math.round((1 - shownBytes / inputBytes) * 100)}% smaller`;
    }
    return `${(shownBytes / inputBytes).toFixed(1)}× larger`;
  })();

  return (
    <aside className="rail" aria-label="Conversion settings">
      <div className="rail-scroll">
        <section className="section">
          <h2 className="section-label">Convert to</h2>
          <div className="seg" role="group" aria-label="Output format">
            {FORMATS.map((value) => (
              <button
                key={value}
                type="button"
                aria-pressed={settings.format === value}
                disabled={disabled}
                onClick={() => onChange({ format: value })}
              >
                {FORMAT_INFO[value].label}
              </button>
            ))}
          </div>
          <p className="hint">{format.hint}</p>
        </section>

        <section className="section">
          <h2 className="section-label">Quality</h2>
          {lossy ? (
            <>
              <div className="quality" role="group" aria-label="Quality">
                {QUALITIES.map((value) => (
                  <button
                    key={value}
                    type="button"
                    aria-pressed={settings.quality === value}
                    disabled={disabled}
                    title={QUALITY_INFO[value].hint}
                    onClick={() => onChange({ quality: value })}
                  >
                    {QUALITY_INFO[value].label}
                  </button>
                ))}
              </div>
              <p className="hint">{QUALITY_INFO[settings.quality].long}</p>
            </>
          ) : (
            <p className="note">
              PNG is lossless, so there is no quality to trade away. Pick JPEG or
              HEIC when file size matters.
            </p>
          )}
        </section>

        <section className="section">
          <h2 className="section-label">Options</h2>
          <div className="option-row">
            <label htmlFor="resize">Resize</label>
            <div className="select-wrap compact">
              <select
                id="resize"
                value={settings.maxDimension ?? ""}
                disabled={disabled}
                onChange={(event) =>
                  onChange({
                    maxDimension: event.target.value ? Number(event.target.value) : null,
                  })
                }
              >
                <option value="">Keep original size</option>
                {RESIZE_OPTIONS.map((px) => (
                  <option key={px} value={px}>
                    Longest edge {px} px
                  </option>
                ))}
              </select>
              <IconChevronDown size={15} />
            </div>
          </div>

          <div className="path-field">
            <button
              type="button"
              className="path-btn"
              title={
                settings.outputDir
                  ? `Saving to ${settings.outputDir}`
                  : "Saving next to the originals"
              }
              onClick={onPickOutput}
              disabled={disabled}
            >
              <IconFolder size={16} />
              <span className="path-text">
                {settings.outputDir
                  ? middleTruncate(settings.outputDir, 40)
                  : "Next to the originals"}
              </span>
            </button>
            {settings.outputDir && (
              <button
                type="button"
                className="icon-btn"
                title="Save next to the originals"
                onClick={onClearOutput}
                disabled={disabled}
              >
                <IconX size={14} />
              </button>
            )}
          </div>

          <label className="check">
            <input
              type="checkbox"
              checked={settings.overwrite}
              disabled={disabled}
              onChange={(event) => onChange({ overwrite: event.target.checked })}
            />
            <span className="box">
              <IconCheck size={12} />
            </span>
            Overwrite existing files
          </label>
        </section>
      </div>

      <div className="cta">
        <div className="estimate-card">
          <div className="estimate-top">
            <span>{phase === "done" ? "Result" : "Estimated result"}</span>
            {estimating && phase !== "done" && <Spinner size={13} />}
          </div>
          <div className="estimate-value">
            {shownBytes != null ? (
              formatBytes(shownBytes)
            ) : estimating ? (
              <span className="skeleton" aria-label="Estimating" />
            ) : (
              "—"
            )}
          </div>
          <div className="estimate-sub">
            {phase === "done" ? (
              `${actualCount} ${actualCount === 1 ? "image" : "images"} written`
            ) : estimate ? (
              `for ${fileCount} ${fileCount === 1 ? "image" : "images"} · sampled ${estimate.sampled} of ${fileCount}`
            ) : estimating ? (
              "Sampling a few images…"
            ) : (
              "No estimate available"
            )}
            {delta && (
              <span className={`delta ${shownBytes! > inputBytes ? "larger" : ""}`}>{delta}</span>
            )}
          </div>
        </div>

        {phase === "converting" ? (
          <div className="working">
            <div className="progress" role="progressbar" aria-valuenow={percent} aria-valuemin={0} aria-valuemax={100}>
              <div className="bar" style={{ width: `${percent}%` }} />
            </div>
            <div className="progress-meta">
              <span>{cancelling ? "Finishing current image…" : "Converting…"}</span>
              <span>
                {progress?.completed ?? 0} / {progress?.total ?? fileCount}
              </span>
            </div>
            <button className="btn ghost" onClick={onCancel} disabled={cancelling}>
              Cancel
            </button>
          </div>
        ) : (
          <button
            className="btn primary convert"
            onClick={onConvert}
            disabled={fileCount === 0}
            title="Convert (Ctrl+Enter)"
          >
            <IconArrowRight size={17} />
            Convert {fileCount} {fileCount === 1 ? "image" : "images"}
          </button>
        )}

        <p className="footnote">Originals are never modified · metadata is dropped</p>
      </div>
    </aside>
  );
}
