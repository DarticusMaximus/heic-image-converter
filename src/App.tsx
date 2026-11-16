import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import { DropZone } from "./components/DropZone";
import { FileList } from "./components/FileList";
import { IconDrop, IconShield, IconX, LogoMark, Spinner } from "./components/Icons";
import { SettingsRail, type Phase } from "./components/SettingsRail";
import { SummaryBanner } from "./components/SummaryBanner";
import {
  loadSettings,
  saveSettings,
  targetPixels,
} from "./format";
import type {
  EstimateResult,
  FileItem,
  FileOutcome,
  ProgressEvent,
  Settings,
  SourceFile,
  Summary,
} from "./types";

const IMAGE_EXTENSIONS = [
  "heic",
  "heif",
  "hif",
  "jpg",
  "jpeg",
  "jpe",
  "jfif",
  "png",
  "webp",
  "tif",
  "tiff",
  "bmp",
  "gif",
];

interface Notice {
  kind: "info" | "warn" | "error";
  text: string;
}

function toItem(file: SourceFile): FileItem {
  return { ...file, status: "pending", result: null, error: null };
}

function applyOutcome(file: FileItem, outcome: FileOutcome): FileItem {
  if (outcome.cancelled) {
    return { ...file, status: "cancelled", error: null, result: null };
  }
  if (outcome.error) {
    return { ...file, status: "error", error: outcome.error, result: null };
  }
  return {
    ...file,
    status: "done",
    error: null,
    result: outcome.outputPath
      ? {
          path: outcome.outputPath,
          bytes: outcome.outputBytes ?? 0,
          width: outcome.width,
          height: outcome.height,
        }
      : null,
  };
}

export default function App() {
  const [files, setFiles] = useState<FileItem[]>([]);
  const [settings, setSettings] = useState<Settings>(() => loadSettings());
  const [thumbs, setThumbs] = useState<Record<string, string>>({});
  const [scanning, setScanning] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [phase, setPhase] = useState<Phase>("idle");
  const [progress, setProgress] = useState<{ completed: number; total: number } | null>(null);
  const [cancelling, setCancelling] = useState(false);
  const [estimate, setEstimate] = useState<EstimateResult | null>(null);
  const [estimating, setEstimating] = useState(false);
  const [summary, setSummary] = useState<Summary | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);

  const phaseRef = useRef<Phase>("idle");
  const convertingRef = useRef(false);
  const estimateSeq = useRef(0);

  useEffect(() => {
    phaseRef.current = phase;
  }, [phase]);

  useEffect(() => {
    saveSettings(settings);
  }, [settings]);

  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => setNotice(null), 8000);
    return () => clearTimeout(timer);
  }, [notice]);

  /* ---------------------------------------------------------------- adding */

  const addPaths = useCallback(async (paths: string[]) => {
    if (!paths.length) return;
    if (phaseRef.current === "converting") {
      setNotice({ kind: "warn", text: "Let the current conversion finish first." });
      return;
    }
    setScanning(true);
    try {
      const result = await api.scanPaths(paths);
      const incoming = result.files.map(toItem);
      setFiles((previous) => {
        const seen = new Set(previous.map((file) => file.path));
        return [...previous, ...incoming.filter((file) => !seen.has(file.path))];
      });
      setSummary(null);
      setPhase("idle");

      if (result.truncated) {
        setNotice({
          kind: "warn",
          text: "That is a lot of images — only the first 20,000 were added.",
        });
      } else if (result.skipped.length > 0 && result.files.length === 0) {
        setNotice({
          kind: "warn",
          text: `Nothing to add: ${result.skipped[0].reason}.`,
        });
      } else if (result.skipped.length > 0) {
        setNotice({
          kind: "info",
          text: `${result.skipped.length} ${
            result.skipped.length === 1 ? "file was" : "files were"
          } skipped (unsupported or unreadable).`,
        });
      }
    } catch (error) {
      setNotice({ kind: "error", text: String(error) });
    } finally {
      setScanning(false);
    }
  }, []);

  const pickFiles = useCallback(async () => {
    if (phaseRef.current === "converting") return;
    try {
      const picked = await open({
        multiple: true,
        title: "Choose images",
        filters: [{ name: "Images", extensions: IMAGE_EXTENSIONS }],
      });
      if (!picked) return;
      await addPaths(Array.isArray(picked) ? picked : [picked]);
    } catch (error) {
      setNotice({ kind: "error", text: String(error) });
    }
  }, [addPaths]);

  const pickFolder = useCallback(async () => {
    if (phaseRef.current === "converting") return;
    try {
      const picked = await open({
        directory: true,
        multiple: true,
        title: "Choose a folder of images",
      });
      if (!picked) return;
      await addPaths(Array.isArray(picked) ? picked : [picked]);
    } catch (error) {
      setNotice({ kind: "error", text: String(error) });
    }
  }, [addPaths]);

  const pickOutput = useCallback(async () => {
    if (phaseRef.current === "converting") return;
    try {
      const picked = await open({
        directory: true,
        multiple: false,
        title: "Choose where converted images go",
      });
      if (!picked || Array.isArray(picked)) return;
      setSettings((previous) => ({ ...previous, outputDir: picked }));
      setPhase("idle");
      setSummary(null);
      setFiles((previous) =>
        previous.map((file) => ({ ...file, status: "pending", result: null, error: null })),
      );
    } catch (error) {
      setNotice({ kind: "error", text: String(error) });
    }
  }, []);

  const removeFile = useCallback((path: string) => {
    setFiles((previous) => previous.filter((file) => file.path !== path));
    setSummary(null);
  }, []);

  const clearAll = useCallback(() => {
    setFiles([]);
    setThumbs({});
    setSummary(null);
    setEstimate(null);
    setPhase("idle");
    setProgress(null);
  }, []);

  /* ------------------------------------------------------------ drag & drop */

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload = event.payload;
        if (payload.type === "enter" || payload.type === "over") {
          setDragging(true);
        } else if (payload.type === "drop") {
          setDragging(false);
          if (payload.paths.length > 0) void addPaths(payload.paths);
        } else {
          setDragging(false);
        }
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [addPaths]);

  /* --------------------------------------------------------------- startup */

  useEffect(() => {
    let cancelled = false;
    api
      .startupPaths()
      .then((paths) => {
        if (!cancelled && paths.length > 0) void addPaths(paths);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [addPaths]);

  /* ------------------------------------------------------------ thumbnails */
  const thumbQueue = useRef<string[]>([]);
  const thumbActive = useRef(0);
  const thumbSeen = useRef<Set<string>>(new Set());

  const pumpThumbs = useCallback(() => {
    while (thumbActive.current < 4 && thumbQueue.current.length > 0) {
      const path = thumbQueue.current.shift() as string;
      thumbActive.current += 1;
      api
        .loadThumbnail(path)
        .then((url) => setThumbs((previous) => ({ ...previous, [path]: url })))
        .catch(() => undefined)
        .finally(() => {
          thumbActive.current -= 1;
          pumpThumbs();
        });
    }
  }, []);

  useEffect(() => {
    for (const file of files) {
      if (thumbSeen.current.has(file.path)) continue;
      thumbSeen.current.add(file.path);
      thumbQueue.current.push(file.path);
    }
    pumpThumbs();
  }, [files, pumpThumbs]);

  /* -------------------------------------------------------------- estimating */

  const estimateKey = useMemo(
    () =>
      JSON.stringify({
        files: files.map((file) => [file.path, file.width, file.height, file.bytes]),
        format: settings.format,
        quality: settings.quality,
        maxDimension: settings.maxDimension,
      }),
    [files, settings.format, settings.quality, settings.maxDimension],
  );

  useEffect(() => {
    if (files.length === 0) {
      setEstimate(null);
      setEstimating(false);
      return;
    }
    if (phase === "converting" || phase === "done") return;

    const sequence = ++estimateSeq.current;
    setEstimating(true);
    const timer = setTimeout(() => {
      api
        .estimateSize(
          files.map((file) => ({
            path: file.path,
            width: file.width,
            height: file.height,
          })),
          settings,
        )
        .then((result) => {
          if (sequence === estimateSeq.current) setEstimate(result);
        })
        .catch(() => {
          if (sequence === estimateSeq.current) setEstimate(null);
        })
        .finally(() => {
          if (sequence === estimateSeq.current) setEstimating(false);
        });
    }, 450);

    return () => clearTimeout(timer);
    // estimateKey captures files + settings; phase gates the work.
  }, [estimateKey, phase]);

  /* --------------------------------------------------------------- converting */

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    listen<ProgressEvent>("convert-progress", ({ payload }) => {
      setProgress({ completed: payload.completed, total: payload.total });
      setFiles((previous) =>
        previous.map((file) =>
          file.path === payload.path ? applyOutcome(file, payload.outcome) : file,
        ),
      );
    })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const handleConvert = useCallback(async () => {
    if (convertingRef.current || files.length === 0) return;
    convertingRef.current = true;

    setSummary(null);
    setCancelling(false);
    setPhase("converting");
    setProgress({ completed: 0, total: files.length });
    setFiles((previous) =>
      previous.map((file) => ({ ...file, status: "pending", result: null, error: null })),
    );

    const items = files.map((file) => ({
      path: file.path,
      root: file.root,
      relative: file.relative,
    }));
    const inputBytes = files.reduce((sum, file) => sum + file.bytes, 0);

    try {
      const outcome = await api.convertFiles(items, settings);
      const byPath = new Map(outcome.results.map((result) => [result.path, result]));
      setFiles((previous) =>
        previous.map((file) => {
          const result = byPath.get(file.path);
          return result ? applyOutcome(file, result) : file;
        }),
      );

      const succeeded = outcome.results.filter((result) => !result.error && !result.cancelled);
      setSummary({
        converted: succeeded.length,
        failed: outcome.results.filter((result) => result.error).length,
        cancelled: outcome.cancelled,
        outputBytes: succeeded.reduce((sum, result) => sum + (result.outputBytes ?? 0), 0),
        inputBytes,
        firstOutput: succeeded.find((result) => result.outputPath)?.outputPath ?? null,
      });
      setPhase("done");
    } catch (error) {
      setPhase("idle");
      setNotice({ kind: "error", text: String(error) });
    } finally {
      convertingRef.current = false;
      setProgress(null);
      setCancelling(false);
    }
  }, [files, settings]);

  const handleCancel = useCallback(async () => {
    setCancelling(true);
    try {
      await api.cancelConversion();
    } catch {
      setCancelling(false);
    }
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const modifier = event.ctrlKey || event.metaKey;
      if (modifier && event.key === "Enter") {
        event.preventDefault();
        void handleConvert();
      } else if (modifier && event.key.toLowerCase() === "o") {
        event.preventDefault();
        void pickFiles();
      } else if (event.key === "Escape" && phaseRef.current === "converting") {
        event.preventDefault();
        void handleCancel();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [handleConvert, handleCancel, pickFiles]);

  const reveal = useCallback(async (path: string) => {
    try {
      await api.revealPath(path);
    } catch (error) {
      setNotice({ kind: "error", text: String(error) });
    }
  }, []);

  const updateSettings = useCallback((patch: Partial<Settings>) => {
    if (phaseRef.current === "converting") return;
    setSettings((previous) => ({ ...previous, ...patch }));
    setPhase("idle");
    setSummary(null);
    setFiles((previous) =>
      previous.map((file) => ({ ...file, status: "pending", result: null, error: null })),
    );
  }, []);

  const clearOutput = useCallback(() => {
    updateSettings({ outputDir: null });
  }, [updateSettings]);

  /* ------------------------------------------------------------------ derived */

  const totalInputBytes = useMemo(
    () => files.reduce((sum, file) => sum + file.bytes, 0),
    [files],
  );

  const estimateFor = useCallback(
    (file: FileItem): number | null => {
      if (!estimate || file.width == null || file.height == null) return null;
      const pixels = targetPixels(file.width, file.height, settings.maxDimension);
      return Math.round(estimate.bytesPerPixel * pixels);
    },
    [estimate, settings.maxDimension],
  );

  const actualBytes =
    phase === "done"
      ? files.reduce((sum, file) => sum + (file.result?.bytes ?? 0), 0)
      : null;
  const actualCount = files.filter((file) => file.status === "done").length;

  const percent =
    progress && progress.total > 0
      ? Math.min(100, Math.round((progress.completed / progress.total) * 100))
      : 0;

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <LogoMark size={28} />
          <span className="brand-name">HEIC Image Converter</span>
        </div>
        <div className="topbar-right">
          {scanning && (
            <span className="scanning">
              <Spinner size={13} /> Reading folder…
            </span>
          )}
          <span className="offline" title="No network access is used for conversion">
            <IconShield size={14} />
            Offline
          </span>
        </div>
      </header>

      {phase === "converting" && (
        <div className="top-progress">
          <div className="top-progress-bar" style={{ width: `${percent}%` }} />
        </div>
      )}

      {files.length === 0 ? (
        <main className="main empty">
          <DropZone onPickFiles={pickFiles} onPickFolder={pickFolder} busy={scanning} />
        </main>
      ) : (
        <main className="main">
          <div className="workspace">
            {summary && (
              <SummaryBanner
                summary={summary}
                onReveal={reveal}
                onDismiss={() => setSummary(null)}
              />
            )}
            <FileList
              files={files}
              thumbs={thumbs}
              estimateFor={estimateFor}
              totalBytes={totalInputBytes}
              scanning={scanning}
              busy={phase === "converting"}
              onPickFiles={pickFiles}
              onPickFolder={pickFolder}
              onClear={clearAll}
              onRemove={removeFile}
              onReveal={reveal}
            />
          </div>
          <SettingsRail
            settings={settings}
            phase={phase}
            fileCount={files.length}
            inputBytes={totalInputBytes}
            estimate={estimate}
            estimating={estimating}
            progress={progress}
            cancelling={cancelling}
            actualBytes={actualBytes}
            actualCount={actualCount}
            onChange={updateSettings}
            onPickOutput={pickOutput}
            onClearOutput={clearOutput}
            onConvert={handleConvert}
            onCancel={handleCancel}
          />
        </main>
      )}

      {notice && (
        <div className={`toast ${notice.kind}`} role="status">
          <span>{notice.text}</span>
          <button className="icon-btn" onClick={() => setNotice(null)} title="Dismiss">
            <IconX size={14} />
          </button>
        </div>
      )}

      <div className={`drop-overlay ${dragging ? "visible" : ""}`}>
        <div className="drop-overlay-inner">
          <IconDrop size={36} />
          <strong>Drop to add</strong>
          <span>Images or folders</span>
        </div>
      </div>
    </div>
  );
}
