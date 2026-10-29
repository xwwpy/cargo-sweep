import type { LatestRustVersions } from "../types";

const CACHE_TTL_MS = 10 * 60 * 1000;

let cached: LatestRustVersions | null = null;
let cachedAt = 0;

export function getCachedRustLatest(): LatestRustVersions | null {
  if (!cached || Date.now() - cachedAt > CACHE_TTL_MS) {
    return null;
  }
  return cached;
}

export function setCachedRustLatest(value: LatestRustVersions): void {
  cached = value;
  cachedAt = Date.now();
}

export function clearCachedRustLatest(): void {
  cached = null;
  cachedAt = 0;
}
