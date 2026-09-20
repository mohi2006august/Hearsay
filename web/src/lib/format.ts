/** Presentation helpers. No API knowledge, no DOM. */

/** "just now", "4m ago", "2h ago" — absolute dates past a week. */
export function relativeTime(tsMs: number, now = Date.now()): string {
  const seconds = Math.round((now - tsMs) / 1000);
  if (seconds < 0) return 'in the future';
  if (seconds < 10) return 'just now';
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  return new Date(tsMs).toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

/** Full timestamp for a detail view, where precision beats brevity. */
export function absoluteTime(tsMs: number): string {
  return new Date(tsMs).toLocaleString(undefined, {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}

export function duration(ms: number): string {
  if (ms < 1000) return `${ms} ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)} s`;
  const minutes = Math.floor(ms / 60_000);
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m`;
}

/** Two decimals, or an em dash when there is genuinely no score. */
export function score(value: number | null | undefined): string {
  return value === null || value === undefined ? '—' : value.toFixed(2);
}

export function percent(value: number, digits = 0): string {
  return `${(value * 100).toFixed(digits)}%`;
}

/** Shorten a content hash for a table cell, keeping the `b3:` prefix. */
export function shortHash(hash: string): string {
  const body = hash.startsWith('b3:') ? hash.slice(3) : hash;
  return `b3:${body.slice(0, 10)}`;
}

/** First line, clipped, for a list row. */
export function truncate(text: string, max = 96): string {
  const oneLine = text.replace(/\s+/g, ' ').trim();
  if (oneLine.length === 0) return '(no text)';
  return oneLine.length <= max ? oneLine : `${oneLine.slice(0, max - 1)}…`;
}

/**
 * Render invisible characters so an attack is visible in the UI.
 *
 * A zero-width space is the whole trick in several injection families; a
 * dashboard that renders it as nothing hides the evidence it exists to show.
 */
export function revealInvisibles(text: string): string {
  return text
    .replace(/[\u200B-\u200F]/g, '\u2423') // ZWSP, ZWNJ, ZWJ, LRM, RLM
    .replace(/[\u202A-\u202E]/g, '\u21C4') // bidi embedding and override
    .replace(/[\u2060-\u2064\uFEFF]/g, '\u00B7'); // word joiner, invisible ops, BOM
}

/** Does this string contain characters the reader cannot see? */
export function hasInvisibles(text: string): boolean {
  return /[\u200B-\u200F\u202A-\u202E\u2060-\u2064\uFEFF]/.test(text);
}
