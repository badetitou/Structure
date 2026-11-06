/** Sets an SVG attribute; `var(--token)` values go through style, since presentation attributes do not reliably resolve them. */
export function setSvgAttr(el: Element, name: string, value: string | number) {
  if (typeof value === "string" && value.startsWith("var(")) {
    (el as SVGElement).style.setProperty(name, value);
  } else {
    (el as SVGElement).style.removeProperty(name); // a stale inline var() would win over the attribute
    el.setAttribute(name, String(value));
  }
}

const SVG_NS = "http://www.w3.org/2000/svg";

/** Creates an SVG element with attributes set through `setSvgAttr`. */
export function s<K extends keyof SVGElementTagNameMap>(
  tag: K,
  attrs: Record<string, string | number> = {},
): SVGElementTagNameMap[K] {
  const el = document.createElementNS(SVG_NS, tag);
  for (const [k, v] of Object.entries(attrs)) setSvgAttr(el, k, v);
  return el;
}
