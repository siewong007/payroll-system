/**
 * RFC 4180 CSV writing for browser-produced exports.
 *
 * Deliberately mirrors `backend/src/services/csv_helpers.rs` so a file the user
 * downloads from a report page and the same data pulled from the API escape
 * identically. If the neutralisation character set or the RM formatting changes
 * on one side, change it on the other in the same commit.
 */

/**
 * Leading bytes a spreadsheet executes as a formula.
 *
 * Matches `neutralize_formula`: an employee name beginning with `=` is an
 * injection vector into whoever opens the export, and `\t` / `\r` are here
 * because Excel strips them before deciding.
 */
const FORMULA_PREFIXES = ['=', '+', '-', '@', '\t', '\r'];

/**
 * Force a spreadsheet to treat a value as text.
 *
 * The numeric carve-out is what keeps this usable on a *mixed* row. The backend
 * applies `neutralize_formula` only to text fields, so it needs no such rule;
 * here one helper runs over every cell, and without the carve-out every negative
 * money cell would export as `'-1700.00` and break the sheet it was meant to
 * protect.
 */
function neutralize(value: string): string {
  if (!FORMULA_PREFIXES.includes(value.charAt(0))) {
    return value;
  }

  const trimmed = value.trim();
  if (trimmed !== '' && Number.isFinite(Number(trimmed))) {
    return value;
  }

  return `'${value}`;
}

/**
 * Escape one value into a CSV field.
 *
 * Quoting and neutralisation are orthogonal and compose: the apostrophe handles
 * the leading-byte execution vector, the quoting handles the field boundary. A
 * department of `Sales, EMEA` used to shift every later column of its row, and
 * `Ali "Bob" Chan` produced `"Ali "Bob" Chan"`, which RFC-4180 parsers truncate.
 */
export function csvCell(value: unknown): string {
  if (value === null || value === undefined) {
    return '';
  }

  const text = neutralize(String(value));
  const needsQuoting = /["\n\r,]/.test(text) || text !== text.trim();
  return needsQuoting ? `"${text.replace(/"/g, '""')}"` : text;
}

/**
 * Machine-readable ringgit from sen: no thousands separators, always two
 * decimals. Mirrors `sen_to_plain_rm`; integer arithmetic deliberately, because
 * `sen / 100` is a float rounding decision on a figure that is already exact.
 */
export function senToPlainRm(sen: number): string {
  const whole = Math.trunc(sen);
  const sign = whole < 0 ? '-' : '';
  const abs = Math.abs(whole);
  return `${sign}${Math.floor(abs / 100)}.${String(abs % 100).padStart(2, '0')}`;
}

/** Render a header row plus body rows as an RFC 4180 document. */
export function toCsv(headers: string[], rows: unknown[][]): string {
  return [headers, ...rows]
    .map((row) => row.map(csvCell).join(','))
    .join('\r\n');
}

/** Build the CSV and hand it to the browser as a download. */
export function downloadCsv(filename: string, headers: string[], rows: unknown[][]): void {
  const blob = new Blob([toCsv(headers, rows)], { type: 'text/csv;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  // Revoking synchronously races the click in some browsers, which download an
  // empty file. One turn of the event loop is enough for the navigation to take
  // the blob.
  setTimeout(() => URL.revokeObjectURL(url), 0);
}
