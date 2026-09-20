/**
 * Minimal DOM construction.
 *
 * Every string this dashboard renders is, by the project's own threat model,
 * attacker-controlled: it is text extracted from an image someone planted.
 * So nothing here builds markup by interpolation. Children are appended as
 * text nodes, which the platform escapes, and attributes go through
 * `setAttribute`. An XSS in the tool that exists to display injections would
 * be a poor look.
 */

import type { RegionAction, RecordSource, ChannelLabel } from './types';

type Attrs = Record<string, string | number | boolean | undefined | null>;
type Child = Node | string | null | undefined | false;

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Attrs = {},
  ...children: Child[]
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (value === undefined || value === null || value === false) continue;
    node.setAttribute(key, String(value));
  }
  append(node, children);
  return node;
}

export function append(parent: Node, children: Child[]): void {
  for (const child of children) {
    if (child === null || child === undefined || child === false) continue;
    parent.appendChild(typeof child === 'string' ? document.createTextNode(child) : child);
  }
}

export function clear(node: Element): void {
  node.replaceChildren();
}

/** Look up an element, or throw loudly — a silent null here is a blank page. */
export function need<T extends Element = HTMLElement>(id: string): T {
  const node = document.getElementById(id);
  if (!node) throw new Error(`missing element #${id}`);
  return node as unknown as T;
}

/**
 * A severity chip. `action` picks the colour from the semantic ramp; `label`
 * is free text so a request-level outcome ("Redacted (2)") can wear the
 * region-level colour.
 */
export function chip(action: RegionAction, label: string = action): HTMLSpanElement {
  return el('span', { class: `chip chip--${action}` }, label);
}

export function sourceChip(source: RecordSource): HTMLSpanElement | null {
  // Live records need no badge; seeded ones do, so nobody reads a fixture
  // timing as a measurement.
  if (source === 'live') return null;
  return el('span', { class: 'chip chip--seed', title: 'Fixture seeded at boot, not a measurement' }, 'seed');
}

export function channelChip(channel: ChannelLabel): HTMLSpanElement {
  return el('span', { class: `chip chip--${channel}` }, channel);
}

/** A definition row for the detail panels. */
export function field(label: string, ...value: Child[]): DocumentFragment {
  const frag = document.createDocumentFragment();
  frag.appendChild(el('dt', {}, label));
  const dd = el('dd', {});
  append(dd, value);
  frag.appendChild(dd);
  return frag;
}

/** Renders text with invisible characters made visible and marked up. */
export function revealed(text: string, reveal: (s: string) => string, hasHidden: boolean): HTMLElement {
  const node = el('span', { class: hasHidden ? 'has-invisibles' : undefined }, reveal(text));
  if (hasHidden) {
    node.title = 'Contains zero-width or bidi characters, shown as ␣ ⇄ ·';
  }
  return node;
}

export function errorNotice(message: string, detail?: string): HTMLElement {
  return el(
    'div',
    { class: 'notice notice--error', role: 'status' },
    el('div', {}, el('strong', {}, message), detail ? el('div', { class: 'dim' }, detail) : null),
  );
}
