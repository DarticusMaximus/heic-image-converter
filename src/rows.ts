import type { FileItem, FileOutcome, SourceFile } from "./types";

export function toItem(file: SourceFile): FileItem {
  return { ...file, status: "pending", result: null, error: null };
}

export function applyOutcome(file: FileItem, outcome: FileOutcome): FileItem {
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
