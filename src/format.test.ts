import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  DEFAULT_SETTINGS,
  formatBytes,
  loadSettings,
  saveSettings,
  targetPixels,
} from "./format";

describe("formatBytes", () => {
  it("reports zero, negative, and non-finite sizes as zero bytes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(-1)).toBe("0 B");
    expect(formatBytes(NaN)).toBe("0 B");
    expect(formatBytes(Infinity)).toBe("0 B");
  });

  it("keeps sub-kilobyte sizes in whole bytes", () => {
    expect(formatBytes(1)).toBe("1 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });

  it("switches to kilobytes at the 1024 boundary with one decimal", () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(1536)).toBe("1.5 KB");
  });

  it("honors the digits argument for the fractional part", () => {
    expect(formatBytes(1536, 2)).toBe("1.50 KB");
  });

  it("drops decimals once the scaled value reaches 100", () => {
    expect(formatBytes(100 * 1024)).toBe("100 KB");
  });

  it("scales up through megabytes and terabytes", () => {
    expect(formatBytes(1024 ** 2)).toBe("1.0 MB");
    expect(formatBytes(1024 ** 4)).toBe("1.0 TB");
  });

  it("stops scaling at terabytes", () => {
    expect(formatBytes(1024 ** 5)).toBe("1024 TB");
  });
});

describe("targetPixels", () => {
  it("counts every pixel when no cap is set", () => {
    expect(targetPixels(4000, 3000, null)).toBe(12_000_000);
  });

  it("leaves an image that already fits under the cap at its full pixel count", () => {
    expect(targetPixels(1000, 800, 2000)).toBe(800_000);
  });

  it("scales a landscape image so the longest edge matches the cap", () => {
    expect(targetPixels(4000, 3000, 2000)).toBe(2000 * 1500);
  });

  it("scales a portrait image so the longest edge matches the cap", () => {
    expect(targetPixels(3000, 4000, 2000)).toBe(1500 * 2000);
  });

  it("keeps at least one pixel per axis on extreme aspect ratios", () => {
    expect(targetPixels(10000, 1, 100)).toBe(100);
    expect(targetPixels(1, 10000, 100)).toBe(100);
  });

  it("keeps the full pixel count when the cap equals the longest edge", () => {
    expect(targetPixels(4000, 3000, 4000)).toBe(12_000_000);
  });
});

describe("test environment", () => {
  it("provides a DOM with localStorage available", () => {
    expect(typeof localStorage).not.toBe("undefined");
  });
});

describe("settings", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  /** Returns the storage key the app actually uses, discovered by saving once. */
  function settingsKey(): string {
    saveSettings(DEFAULT_SETTINGS);
    const key = localStorage.key(0);
    if (key === null) throw new Error("saveSettings wrote nothing to storage");
    return key;
  }

  /** Seeds stored state with raw text, including values the app would never write. */
  function seedRaw(raw: string) {
    localStorage.setItem(settingsKey(), raw);
  }

  function seedStored(settings: unknown) {
    seedRaw(JSON.stringify(settings));
  }

  it("returns defaults when nothing is stored", () => {
    expect(loadSettings()).toEqual(DEFAULT_SETTINGS);
  });

  it("returns defaults when the stored value is not JSON", () => {
    seedRaw("{not json");
    expect(loadSettings()).toEqual(DEFAULT_SETTINGS);
  });

  it("returns defaults when the stored value parses to null", () => {
    seedRaw("null");
    expect(loadSettings()).toEqual(DEFAULT_SETTINGS);
  });

  it("replaces an unknown format with the default", () => {
    seedStored({ format: "gif" });
    expect(loadSettings().format).toBe("jpeg");
  });

  it("replaces an unknown quality with the default", () => {
    seedStored({ quality: "ultra" });
    expect(loadSettings().quality).toBe("high");
  });

  it("rejects a maxDimension below the minimum", () => {
    seedStored({ maxDimension: 15 });
    expect(loadSettings().maxDimension).toBe(null);
  });

  it("keeps a maxDimension at or above the minimum", () => {
    seedStored({ maxDimension: 16 });
    expect(loadSettings().maxDimension).toBe(16);
  });

  it("rejects a non-numeric maxDimension", () => {
    seedStored({ maxDimension: "16" });
    expect(loadSettings().maxDimension).toBe(null);
  });

  it("keeps an unset maxDimension unset", () => {
    seedStored({ maxDimension: null });
    expect(loadSettings().maxDimension).toBe(null);
  });

  it("rejects an empty output directory", () => {
    seedStored({ outputDir: "" });
    expect(loadSettings().outputDir).toBe(null);
  });

  it("rejects a non-string output directory", () => {
    seedStored({ outputDir: 42 });
    expect(loadSettings().outputDir).toBe(null);
  });

  it("keeps a valid output directory", () => {
    seedStored({ outputDir: "/tmp/out" });
    expect(loadSettings().outputDir).toBe("/tmp/out");
  });

  it("rejects a string overwrite flag", () => {
    seedStored({ overwrite: "true" });
    expect(loadSettings().overwrite).toBe(false);
  });

  it("keeps a boolean overwrite flag", () => {
    seedStored({ overwrite: true });
    expect(loadSettings().overwrite).toBe(true);
  });

  it("round-trips a fully populated settings object", () => {
    const stored = {
      format: "heic" as const,
      quality: "maximum" as const,
      maxDimension: 4096,
      outputDir: "/home/user/out",
      overwrite: true,
    };
    saveSettings(stored);
    expect(loadSettings()).toEqual(stored);
  });

  it("stays silent when storage rejects writes", () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("QuotaExceededError");
    });
    expect(() => saveSettings(DEFAULT_SETTINGS)).not.toThrow();
  });
});
