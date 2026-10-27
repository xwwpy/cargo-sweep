const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB"] as const;
const WINDOWS = typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);

export function pathKey(path: string): string {
  const normalized = path.replace(/[\\/]+$/, "");
  return WINDOWS ? normalized.replace(/\//g, "\\").toLowerCase() : normalized;
}

export interface ByteParts {
  value: string;
  unit: string;
}

export function splitBytes(bytes: number): ByteParts {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return { value: "0", unit: "B" };
  }

  const exponent = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    BYTE_UNITS.length - 1,
  );
  const value = bytes / 1024 ** exponent;
  const decimals = exponent === 0 ? 0 : value >= 100 ? 0 : value >= 10 ? 1 : 2;

  return {
    value: value.toFixed(decimals),
    unit: BYTE_UNITS[exponent],
  };
}

export function formatBytes(bytes: number): string {
  const parts = splitBytes(bytes);
  return `${parts.value} ${parts.unit}`;
}

export function formatCount(count: number): string {
  return new Intl.NumberFormat("zh-CN").format(count);
}

export function formatRelativeTime(timestamp: number | null): string {
  if (!timestamp) {
    return "尚未扫描";
  }

  const deltaSeconds = Math.max(0, Math.floor((Date.now() - timestamp) / 1000));
  if (deltaSeconds < 30) {
    return "刚刚";
  }
  if (deltaSeconds < 60 * 60) {
    return `${Math.floor(deltaSeconds / 60)} 分钟前`;
  }
  if (deltaSeconds < 60 * 60 * 24) {
    return `${Math.floor(deltaSeconds / 3600)} 小时前`;
  }
  if (deltaSeconds < 60 * 60 * 24 * 2) {
    return "昨天";
  }
  return new Intl.DateTimeFormat("zh-CN", {
    month: "short",
    day: "numeric",
  }).format(timestamp);
}

export function formatDateTime(timestamp: number): string {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(timestamp);
}

export function formatDuration(milliseconds: number): string {
  if (milliseconds < 1000) {
    return `${Math.max(1, Math.round(milliseconds))} ms`;
  }
  return `${(milliseconds / 1000).toFixed(1)} s`;
}

export function cleanPath(path: string): string {
  if (path.startsWith("\\\\?\\UNC\\")) {
    return `\\\\${path.slice(8)}`;
  }
  if (path.startsWith("\\\\?\\")) {
    return path.slice(4);
  }
  return path;
}

export function basename(path: string): string {
  const normalized = cleanPath(path).replace(/[\\/]+$/, "");
  const segments = normalized.split(/[\\/]/);
  return segments[segments.length - 1] || normalized;
}
