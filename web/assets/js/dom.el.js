// @agents-index Element helpers that take the document as a parameter; text always goes in as a text node, never as HTML.

const SVG = 'http://www.w3.org/2000/svg';

/** An HTML element with classes and an optional text; the text is set with textContent. */
export function el(doc, tag, cls, text) {
  const e = doc.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined && text !== null) e.textContent = String(text);
  return e;
}

/** An SVG element with attributes. */
export function svg(doc, tag, attrs = {}) {
  const e = doc.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  return e;
}

/** A card with a label, for a section of the page. */
export function card(doc, label, cls = 'card') {
  const c = el(doc, 'section', cls);
  c.append(el(doc, 'h2', 'label', label));
  return c;
}
