export type OutputFormat = "png" | "jpeg" | "heic";
export type Quality = "smallest" | "balanced" | "high" | "maximum";
export type FileStatus = "pending" | "converting" | "done" | "error" | "cancelled";

export interface Settings {
  format: OutputFormat;
  quality: Quality;
  maxDimension: number | null;
  outputDir: string | null;
  overwrite: boolean;
}

export interface SourceFile {
  id: string;
  path: string;
  name: string;
  kind: string;
  bytes: number;
  width: number | null;
  height: number | null;
  root: string | null;
  relative: string | null;
}

export interface SkippedFile {
  path: string;
  reason: string;
}

export interface ScanResult {
  files: SourceFile[];
  skipped: SkippedFile[];
  truncated: boolean;
}

export interface ConvertItem {
  path: string;
  root: string | null;
  relative: string | null;
}

export interface FileOutcome {
  path: string;
  outputPath: string | null;
  outputBytes: number | null;
  inputBytes: number;
  width: number | null;
  height: number | null;
  error: string | null;
  cancelled: boolean;
}

export interface ConversionOutcome {
  results: FileOutcome[];
  cancelled: boolean;
}

export interface ProgressEvent {
  completed: number;
  total: number;
  path: string;
  outcome: FileOutcome;
}

export interface SampleEstimate {
  name: string;
  inputBytes: number;
  outputBytes: number;
  width: number;
  height: number;
}

export interface EstimateResult {
  bytesPerPixel: number;
  estimatedTotalBytes: number;
  totalPixels: number;
  sampled: number;
  samples: SampleEstimate[];
}

export interface FileItem extends SourceFile {
  status: FileStatus;
  result: {
    path: string;
    bytes: number;
    width: number | null;
    height: number | null;
  } | null;
  error: string | null;
}

export interface Summary {
  converted: number;
  failed: number;
  cancelled: boolean;
  outputBytes: number;
  inputBytes: number;
  firstOutput: string | null;
}
