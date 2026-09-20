/**
 * Typed client for hearsay-api.
 *
 * Everything on the dashboard comes through here. Two rules the rest of the
 * app relies on:
 *
 *  - Failures are `ApiError`, never a thrown string or a silent `undefined`.
 *    A dashboard that shows stale numbers during an outage is worse than one
 *    that says it cannot reach the service.
 *  - Every request has a timeout. A hung fetch would otherwise leave the
 *    polling loop wedged with no visible cause.
 */

import type {
  DecisionRecord,
  EvaluateRequestInput,
  RulesetView,
  Stats,
  ApiErrorBody,
} from './types';

/**
 * Base URL for the API.
 *
 * - `PUBLIC_HEARSAY_API` wins whenever it is set. Point it at wherever
 *   hearsay-api actually lives.
 * - In dev it is otherwise empty, so requests stay same-origin and the Vite
 *   proxy in `astro.config.mjs` forwards them. No CORS involved.
 * - In a build it otherwise falls back to the default local port, because a
 *   static bundle has no proxy behind it. Serving the bundle and the API from
 *   one origin is the better deployment; this default just means `astro
 *   preview` works out of the box instead of 404ing on every call.
 */
export const API_BASE: string =
  import.meta.env.PUBLIC_HEARSAY_API ?? (import.meta.env.DEV ? '' : 'http://127.0.0.1:8787');

const DEFAULT_TIMEOUT_MS = 8000;

export class ApiError extends Error {
  readonly status: number;
  readonly detail: string;
  readonly kind: 'network' | 'timeout' | 'http' | 'parse';

  constructor(
    kind: ApiError['kind'],
    message: string,
    detail = '',
    status = 0,
  ) {
    super(message);
    this.name = 'ApiError';
    this.kind = kind;
    this.detail = detail;
    this.status = status;
  }

  /** One line suitable for showing to a person, not a stack trace. */
  get userMessage(): string {
    switch (this.kind) {
      case 'network':
        return `Cannot reach hearsay-api at ${API_BASE || 'the dev proxy'}. Is it running?`;
      case 'timeout':
        return 'hearsay-api did not respond in time.';
      case 'parse':
        return 'hearsay-api returned something that is not valid JSON.';
      case 'http':
        return this.detail || this.message;
    }
  }
}

async function request<T>(
  path: string,
  init: RequestInit = {},
  timeoutMs = DEFAULT_TIMEOUT_MS,
): Promise<T> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);

  let response: Response;
  try {
    response = await fetch(`${API_BASE}${path}`, {
      ...init,
      signal: controller.signal,
      headers: {
        Accept: 'application/json',
        ...(init.body ? { 'Content-Type': 'application/json' } : {}),
        ...init.headers,
      },
    });
  } catch (cause) {
    clearTimeout(timer);
    if (cause instanceof DOMException && cause.name === 'AbortError') {
      throw new ApiError('timeout', `GET ${path} timed out after ${timeoutMs}ms`);
    }
    throw new ApiError('network', `GET ${path} failed`, String(cause));
  } finally {
    clearTimeout(timer);
  }

  if (!response.ok) {
    // The API uses one error body shape; fall back to the status text for
    // anything that did not come from our handlers (a proxy, say).
    let detail = response.statusText;
    try {
      const body = (await response.json()) as ApiErrorBody;
      if (body && typeof body.detail === 'string') {
        detail = body.detail;
      }
    } catch {
      /* keep statusText */
    }
    throw new ApiError('http', `${response.status} on ${path}`, detail, response.status);
  }

  try {
    return (await response.json()) as T;
  } catch (cause) {
    throw new ApiError('parse', `Malformed JSON from ${path}`, String(cause));
  }
}

export function getStats(): Promise<Stats> {
  return request<Stats>('/api/stats');
}

export function getRuleset(): Promise<RulesetView> {
  return request<RulesetView>('/api/ruleset');
}

export interface DecisionQuery {
  sinceMs?: number;
  limit?: number;
}

export function getDecisions(q: DecisionQuery = {}): Promise<DecisionRecord[]> {
  const params = new URLSearchParams();
  if (q.sinceMs !== undefined) params.set('since_ms', String(q.sinceMs));
  if (q.limit !== undefined) params.set('limit', String(q.limit));
  const qs = params.toString();
  return request<DecisionRecord[]>(`/api/decisions${qs ? `?${qs}` : ''}`);
}

export function getDecision(id: string): Promise<DecisionRecord> {
  return request<DecisionRecord>(`/api/decisions/${encodeURIComponent(id)}`);
}

export function evaluate(body: EvaluateRequestInput): Promise<DecisionRecord> {
  return request<DecisionRecord>('/api/evaluate', {
    method: 'POST',
    body: JSON.stringify(body),
  });
}

/**
 * Poll a function on an interval, pausing while the tab is hidden and backing
 * off when the API is unreachable.
 *
 * Returns a stop function. The backoff matters: without it, a dashboard left
 * open against a stopped service hammers a dead port every two seconds for
 * the rest of the day.
 */
export function poll(
  fn: () => Promise<void>,
  intervalMs: number,
  onError: (e: ApiError) => void,
): () => void {
  let stopped = false;
  let failures = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const tick = async (): Promise<void> => {
    if (stopped) return;

    if (document.visibilityState === 'hidden') {
      schedule(intervalMs);
      return;
    }

    try {
      await fn();
      failures = 0;
    } catch (e) {
      failures += 1;
      onError(e instanceof ApiError ? e : new ApiError('network', String(e)));
    }
    // Exponential backoff to a 30s ceiling while the service is down.
    schedule(failures === 0 ? intervalMs : Math.min(intervalMs * 2 ** failures, 30_000));
  };

  const schedule = (ms: number): void => {
    if (stopped) return;
    timer = setTimeout(() => void tick(), ms);
  };

  // Come back immediately when the tab is focused again rather than waiting
  // out the remaining interval.
  const onVisible = (): void => {
    if (document.visibilityState === 'visible' && !stopped) {
      if (timer) clearTimeout(timer);
      void tick();
    }
  };
  document.addEventListener('visibilitychange', onVisible);

  void tick();

  return () => {
    stopped = true;
    if (timer) clearTimeout(timer);
    document.removeEventListener('visibilitychange', onVisible);
  };
}
