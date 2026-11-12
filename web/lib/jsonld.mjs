// Serialise data for an inline <script type="application/ld+json"> block.
//
// JSON.stringify does not escape "<", so an event title containing "</script><script>…" closes the JSON-LD
// block and runs as a script on every page that lists the event (anyone can publish an event). Escaping
// <, >, & and the two JS line separators as \uXXXX keeps the JSON identical for parsers and makes it
// impossible to leave the script element. (TASK-302 P1.)

const LS = String.fromCharCode(0x2028);
const PS = String.fromCharCode(0x2029);
const ESC = { "<": "\\u003c", ">": "\\u003e", "&": "\\u0026", [LS]: "\\u2028", [PS]: "\\u2029" };
const RE = new RegExp(`[<>&${LS}${PS}]`, "g");

/** @param {unknown} data @returns {string} */
export function jsonLdString(data) {
  return JSON.stringify(data).replace(RE, (c) => ESC[c]);
}
