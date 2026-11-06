/** Long symbol lists render a page at a time ("Show more"). */
export const CAP = 200;

export interface Caps {
  changed: number;
  impacted: number;
}

/** What shows when an impact arrives (or is replaced). */
export const initialCaps = (): Caps => ({ changed: CAP, impacted: CAP });

/** Show one more page of `list`. */
export const showMore = (caps: Caps, list: keyof Caps): Caps => ({ ...caps, [list]: caps[list] + CAP });

/** Label of the "Show more" button, or null when everything is visible. */
export const moreLabel = (shown: number, total: number): string | null =>
  total > shown ? `Show more (${shown} / ${total})` : null;
