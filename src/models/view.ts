import type { ModelEntry, ModelsState } from "../bindings";
import { formatNumber } from "../i18n";

/** The state a Model card shows (models.md "UI"). */
export type CardState =
  | { kind: "notDownloaded" }
  | { kind: "queued" }
  | { kind: "downloading"; percent: number; mbPerSecond: number }
  | { kind: "verifying" }
  | { kind: "paused"; percent: number }
  | { kind: "failed"; entry: Extract<ModelEntry["download"], { state: "failed" }> }
  | { kind: "loading" }
  | { kind: "downloaded" }
  | { kind: "active" };

export function cardState(entry: ModelEntry, models: ModelsState): CardState {
  const isActive = models.active === entry.id;
  if (models.activating === entry.id || (isActive && models.activeState === "loading")) {
    return { kind: "loading" };
  }
  const download = entry.download;
  switch (download.state) {
    case "queued":
      return { kind: "queued" };
    case "downloading":
      return {
        kind: "downloading",
        percent: percentOf(download.downloaded, download.total),
        mbPerSecond: download.bytesPerSecond / MIB,
      };
    case "verifying":
      return { kind: "verifying" };
    case "paused":
      return { kind: "paused", percent: percentOf(download.downloaded, download.total) };
    case "failed":
      return { kind: "failed", entry: download };
    case "idle":
      break;
  }
  if (!entry.downloaded) return { kind: "notDownloaded" };
  return isActive ? { kind: "active" } : { kind: "downloaded" };
}

const MIB = 1024 * 1024;

/** Whole percent, never 100 before the last byte. */
export function percentOf(downloaded: number, total: number): number {
  if (total <= 0) return 0;
  return Math.min(Math.floor((downloaded / total) * 100), downloaded >= total ? 100 : 99);
}

/** Sizes in MB as the spec writes them ("845 MB"; binary megabytes). */
export function megabytes(bytes: number): string {
  return formatNumber(Math.round(bytes / MIB));
}

export function speed(mbPerSecond: number): string {
  return formatNumber(mbPerSecond, { minimumFractionDigits: 1, maximumFractionDigits: 1 });
}

/** The space deleting the Model frees: the file, or what a paused download has kept. */
export function bytesOnDisk(entry: ModelEntry): number {
  if (entry.downloaded) return entry.sizeBytes;
  const download = entry.download;
  if (download.state === "paused" || download.state === "failed") return download.downloaded;
  return 0;
}

/** The Model whose download is running, if any. */
export function downloadingEntry(models: ModelsState): ModelEntry | undefined {
  return models.models.find(
    (m) => m.download.state === "downloading" || m.download.state === "verifying",
  );
}
