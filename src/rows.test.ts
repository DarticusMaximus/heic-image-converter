import { describe, expect, it } from "vitest";

import { applyOutcome, toItem } from "./rows";
import type { FileItem, FileOutcome, SourceFile } from "./types";

function sourceFile(): SourceFile {
  return {
    id: "/tmp/in/photo.jpg",
    path: "/tmp/in/photo.jpg",
    name: "photo.jpg",
    kind: "jpeg",
    bytes: 4096,
    width: 1200,
    height: 800,
    root: "/tmp/in",
    relative: "photo.jpg",
  };
}

function outcome(patch: Partial<FileOutcome> = {}): FileOutcome {
  return {
    path: "/tmp/in/photo.jpg",
    outputPath: "/tmp/out/photo.heic",
    outputBytes: 2048,
    inputBytes: 4096,
    width: 1200,
    height: 800,
    error: null,
    cancelled: false,
    ...patch,
  };
}

describe("toItem", () => {
  it("maps a source file to a pending row and preserves every source field", () => {
    const file = sourceFile();

    expect(toItem(file)).toEqual({
      ...file,
      status: "pending",
      result: null,
      error: null,
    });
  });
});

describe("applyOutcome", () => {
  it("lets a cancelled outcome win over a populated error", () => {
    const row = toItem(sourceFile());
    const next = applyOutcome(
      row,
      outcome({ cancelled: true, error: "write failed", outputPath: null }),
    );

    expect(next.status).toBe("cancelled");
    expect(next.error).toBe(null);
    expect(next.result).toBe(null);
  });

  it("records an error outcome with its message and no result", () => {
    const row = toItem(sourceFile());
    const next = applyOutcome(
      row,
      outcome({ error: "unsupported color profile", outputPath: null, outputBytes: null }),
    );

    expect(next.status).toBe("error");
    expect(next.error).toBe("unsupported color profile");
    expect(next.result).toBe(null);
  });

  it("records a success with its output path, bytes, and dimensions", () => {
    const row = toItem(sourceFile());
    const next = applyOutcome(row, outcome());

    expect(next.status).toBe("done");
    expect(next.error).toBe(null);
    expect(next.result).toEqual({
      path: "/tmp/out/photo.heic",
      bytes: 2048,
      width: 1200,
      height: 800,
    });
  });

  it("treats missing output bytes as zero bytes", () => {
    const row = toItem(sourceFile());
    const next = applyOutcome(row, outcome({ outputBytes: null }));

    expect(next.status).toBe("done");
    expect(next.result?.bytes).toBe(0);
  });

  it("records a success without an output path as a done row with no result", () => {
    const row = toItem(sourceFile());
    const next = applyOutcome(row, outcome({ outputPath: null }));

    expect(next.status).toBe("done");
    expect(next.result).toBe(null);
  });

  it("does not mutate the row it is given", () => {
    const row: FileItem = { ...sourceFile(), status: "pending", result: null, error: null };
    const before: FileItem = { ...row };

    applyOutcome(row, outcome());

    expect(row).toEqual(before);
  });
});
