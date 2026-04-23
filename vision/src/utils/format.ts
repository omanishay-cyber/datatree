// Formatting helpers used across views.

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[unit]}`;
}

export function formatNumber(n: number): string {
  if (!Number.isFinite(n)) return "—";
  return new Intl.NumberFormat("en-US").format(n);
}

export function formatRelativeTime(ts: number, now: number = Date.now()): string {
  const diff = now - ts;
  const abs = Math.abs(diff);
  const sec = 1000;
  const min = sec * 60;
  const hr = min * 60;
  const day = hr * 24;
  const week = day * 7;
  const formatter = new Intl.RelativeTimeFormat("en", { numeric: "auto" });
  if (abs < min) return formatter.format(-Math.round(diff / sec), "second");
  if (abs < hr) return formatter.format(-Math.round(diff / min), "minute");
  if (abs < day) return formatter.format(-Math.round(diff / hr), "hour");
  if (abs < week) return formatter.format(-Math.round(diff / day), "day");
  return new Date(ts).toLocaleDateString();
}

export function truncate(text: string, max = 64): string {
  if (text.length <= max) return text;
  return `${text.slice(0, max - 1)}…`;
}

export function formatHash(hash: string, len = 7): string {
  return hash.length > len ? hash.slice(0, len) : hash;
}

export function formatPercent(value: number, fractionDigits = 0): string {
  if (!Number.isFinite(value)) return "—";
  return `${value.toFixed(fractionDigits)}%`;
}
