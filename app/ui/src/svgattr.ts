/** Sets an SVG attribute; `var(--token)` values go through style, since presentation attributes do not reliably resolve them. */
export function setSvgAttr(el: Element, name: string, value: string | number) {
  if (typeof value === "string" && value.startsWith("var(")) {
    (el as SVGElement).style.setProperty(name, value);
  } else {
    (el as SVGElement).style.removeProperty(name); // a stale inline var() would win over the attribute
    el.setAttribute(name, String(value));
  }
}
