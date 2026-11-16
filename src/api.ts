import { invoke } from "@tauri-apps/api/core";
import type {
  ConversionOutcome,
  ConvertItem,
  EstimateResult,
  ScanResult,
  Settings,
} from "./types";

export const scanPaths = (paths: string[]) =>
  invoke<ScanResult>("scan_paths", { paths });

export const loadThumbnail = (path: string, size = 128) =>
  invoke<string>("load_thumbnail", { path, size });

export const estimateSize = (
  items: { path: string; width: number | null; height: number | null }[],
  settings: Settings,
) => invoke<EstimateResult>("estimate_size", { items, settings });

export const convertFiles = (items: ConvertItem[], settings: Settings) =>
  invoke<ConversionOutcome>("convert_files", { items, settings });

export const cancelConversion = () => invoke<void>("cancel_conversion");

export const revealPath = (path: string) => invoke<void>("reveal_path", { path });

export const startupPaths = () => invoke<string[]>("startup_paths");
