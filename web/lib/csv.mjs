// RFC 4180 CSV for the organiser's guest export, safe to open in a spreadsheet (TASK-302 CT9 / G11).
// Every field is quoted (commas, quotes and newlines stay inside one cell) and a field that a spreadsheet would
// run as a formula (= + - @, tab, CR at the start) gets a leading apostrophe.

/** @param {unknown} v @returns {string} */
export function csvField(v) {
  let s = v == null ? "" : String(v);
  if (/^[=+\-@\t\r]/.test(s)) s = "'" + s;
  return '"' + s.replace(/"/g, '""') + '"';
}

/** @param {unknown[][]} rows @returns {string} */
export function toCsv(rows) {
  return rows.map((r) => r.map(csvField).join(",")).join("\r\n") + "\r\n";
}
