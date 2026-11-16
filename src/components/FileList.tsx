import { memo } from "react";
import { formatBytes } from "../format";
import type { FileItem } from "../types";
import {
  IconAlert,
  IconCheck,
  IconFiles,
  IconFolderPlus,
  IconImage,
  IconReveal,
  IconX,
  Spinner,
} from "./Icons";

interface Props {
  files: FileItem[];
  thumbs: Record<string, string>;
  estimateFor: (file: FileItem) => number | null;
  totalBytes: number;
  scanning: boolean;
  busy: boolean;
  onPickFiles: () => void;
  onPickFolder: () => void;
  onClear: () => void;
  onRemove: (path: string) => void;
  onReveal: (path: string) => void;
}

export function FileList({
  files,
  thumbs,
  estimateFor,
  totalBytes,
  scanning,
  busy,
  onPickFiles,
  onPickFolder,
  onClear,
  onRemove,
  onReveal,
}: Props) {
  return (
    <section className="file-list" aria-label="Selected images">
      <header className="list-header">
        <div className="list-summary">
          <strong>
            {files.length} {files.length === 1 ? "image" : "images"}
          </strong>
          <span>{formatBytes(totalBytes)}</span>
          {scanning && (
            <span className="scanning">
              <Spinner size={13} /> reading…
            </span>
          )}
        </div>
        <div className="list-actions">
          <button className="btn small" onClick={onPickFiles} disabled={busy}>
            <IconFiles size={15} />
            Add images
          </button>
          <button className="btn small" onClick={onPickFolder} disabled={busy}>
            <IconFolderPlus size={15} />
            Add folder
          </button>
          <button className="btn small ghost" onClick={onClear} disabled={busy}>
            Clear
          </button>
        </div>
      </header>

      <div className="rows">
        {files.map((file) => (
          <FileRow
            key={file.id}
            file={file}
            thumb={thumbs[file.path]}
            estimate={file.status === "pending" ? estimateFor(file) : null}
            onRemove={onRemove}
            onReveal={onReveal}
          />
        ))}
      </div>
    </section>
  );
}

interface RowProps {
  file: FileItem;
  thumb: string | undefined;
  estimate: number | null;
  onRemove: (path: string) => void;
  onReveal: (path: string) => void;
}

const FileRow = memo(function FileRow({ file, thumb, estimate, onRemove, onReveal }: RowProps) {
  const dimensions =
    file.width != null && file.height != null
      ? `${file.width} × ${file.height}`
      : "unknown size";
  const meta = [file.kind.toUpperCase(), dimensions, formatBytes(file.bytes)].join(" · ");

  return (
    <div className={`row ${file.status}`} data-path={file.path}>
      <div className="thumb">
        {thumb ? (
          <img src={thumb} alt="" width={44} height={44} draggable={false} />
        ) : (
          <IconImage size={19} />
        )}
      </div>

      <div className="row-main">
        <div className="row-name" title={file.path}>
          {file.name}
        </div>
        <div className="row-meta">{meta}</div>
        {file.error && (
          <div className="row-error" title={file.error}>
            {file.error}
          </div>
        )}
      </div>

      <div className="row-status">
        {file.status === "converting" && (
          <span className="status working">
            <Spinner size={14} /> Converting
          </span>
        )}
        {file.status === "done" && file.result && (
          <span className="status done" title={file.result.path}>
            <IconCheck size={15} /> {formatBytes(file.result.bytes)}
          </span>
        )}
        {file.status === "error" && (
          <span className="status failed">
            <IconAlert size={15} /> Failed
          </span>
        )}
        {file.status === "cancelled" && <span className="status skipped">Skipped</span>}
        {file.status === "pending" && (
          <span className="status estimate">
            {estimate != null ? `≈ ${formatBytes(estimate)}` : "—"}
          </span>
        )}
      </div>

      <div className="row-tools">
        {file.status === "done" && file.result && (
          <button
            className="icon-btn"
            title="Show in folder"
            onClick={() => onReveal(file.result!.path)}
          >
            <IconReveal size={15} />
          </button>
        )}
        <button
          className="icon-btn"
          title="Remove from list"
          onClick={() => onRemove(file.path)}
        >
          <IconX size={15} />
        </button>
      </div>
    </div>
  );
});
