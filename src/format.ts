import type { OutputFormat, Quality, Settings } from "./types";

const UNITS = ["B", "KB", "MB", "GB", "TB"];

export function formatBytes(bytes: number, digits = 1): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const decimals = unit === 0 ? 0 : value >= 100 ? 0 : digits;
  return `${value.toFixed(decimals)} ${UNITS[unit]}`;
}

/** Mirrors `estimate::target_pixels` on the Rust side. */
export function targetPixels(
  width: number,
  height: number,
  maxDimension: number | null,
): number {
  if (!maxDimension) return width * height;
  const longest = Math.max(width, height);
  if (longest <= maxDimension) return width * height;
  const scale = maxDimension / longest;
  return Math.max(1, Math.round(width * scale)) * Math.max(1, Math.round(height * scale));
}

export const FORMAT_INFO: Record<
  OutputFormat,
  { label: string; extension: string; hint: string }
> = {
  png: {
    label: "PNG",
    extension: "png",
    hint: "Lossless and universal. Biggest files.",
  },
  jpeg: {
    label: "JPEG",
    extension: "jpg",
    hint: "Opens everywhere. Small files.",
  },
  heic: {
    label: "HEIC",
    extension: "heic",
    hint: "Tiny files. Apple devices and modern browsers.",
  },
};

export const QUALITY_INFO: Record<
  Quality,
  { label: string; hint: string; long: string }
> = {
  smallest: {
    label: "Smallest",
    hint: "Tiny files",
    long: "Softens fine detail. Fine for screenshots and quick sharing.",
  },
  balanced: {
    label: "Balanced",
    hint: "For sharing",
    long: "The sweet spot for most photos.",
  },
  high: {
    label: "High",
    hint: "Near-original",
    long: "Hard to tell apart from the original at normal size.",
  },
  maximum: {
    label: "Maximum",
    hint: "Archive",
    long: "Chases the last bit of detail. Bigger files, slower encode.",
  },
};

export function middleTruncate(value: string, max = 38): string {
  if (value.length <= max) return value;
  const head = Math.ceil((max - 1) / 2);
  const tail = Math.floor((max - 1) / 2);
  return `${value.slice(0, head)}…${value.slice(value.length - tail)}`;
}

export function dirName(path: string): string {
  const index = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return index > 0 ? path.slice(0, index) : path;
}

const SETTINGS_KEY = "heic-converter.settings.v1";

export const DEFAULT_SETTINGS: Settings = {
  format: "jpeg",
  quality: "high",
  maxDimension: null,
  outputDir: null,
  overwrite: false,
};

export function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    if (!raw) return DEFAULT_SETTINGS;
    const parsed = JSON.parse(raw) as Partial<Settings>;
    return {
      format:
        parsed.format === "png" || parsed.format === "heic" || parsed.format === "jpeg"
          ? parsed.format
          : DEFAULT_SETTINGS.format,
      quality:
        parsed.quality === "smallest" ||
        parsed.quality === "balanced" ||
        parsed.quality === "high" ||
        parsed.quality === "maximum"
          ? parsed.quality
          : DEFAULT_SETTINGS.quality,
      maxDimension:
        typeof parsed.maxDimension === "number" && parsed.maxDimension >= 16
          ? parsed.maxDimension
          : null,
      outputDir: typeof parsed.outputDir === "string" && parsed.outputDir ? parsed.outputDir : null,
      overwrite: parsed.overwrite === true,
    };
  } catch {
    return DEFAULT_SETTINGS;
  }
}

export function saveSettings(settings: Settings) {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
  } catch {
    /* private/incognito storage failures are not worth surfacing */
  }
}
