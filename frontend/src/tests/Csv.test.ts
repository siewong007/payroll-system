import { describe, expect, it } from 'vitest';
import { csvCell, senToPlainRm, toCsv } from '@/lib/csv';

/**
 * Minimal RFC 4180 reader, used to assert the *shape* of what we write rather
 * than its literal bytes — a field count is the property that actually breaks
 * when escaping is wrong.
 */
function parseCsv(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = '';
  let quoted = false;

  for (let i = 0; i < text.length; i++) {
    const char = text[i];

    if (quoted) {
      if (char === '"') {
        if (text[i + 1] === '"') {
          field += '"';
          i++;
        } else {
          quoted = false;
        }
      } else {
        field += char;
      }
      continue;
    }

    if (char === '"') {
      quoted = true;
    } else if (char === ',') {
      row.push(field);
      field = '';
    } else if (char === '\r' && text[i + 1] === '\n') {
      row.push(field);
      rows.push(row);
      row = [];
      field = '';
      i++;
    } else {
      field += char;
    }
  }

  row.push(field);
  rows.push(row);
  return rows;
}

describe('csvCell quoting', () => {
  it('quotes a value containing a comma so the row keeps its field count', () => {
    expect(csvCell('Sales, EMEA')).toBe('"Sales, EMEA"');
  });

  it('doubles an embedded quote instead of emitting a broken field', () => {
    // The old hand-rolled `"${name}"` produced `"Ali "Bob" Chan"`, which a
    // conforming parser truncates at the second quote.
    expect(csvCell('Ali "Bob" Chan')).toBe('"Ali ""Bob"" Chan"');
  });

  it('quotes a value with an embedded newline', () => {
    expect(csvCell('Line one\nLine two')).toBe('"Line one\nLine two"');
  });

  it('quotes leading and trailing whitespace so it survives the round trip', () => {
    expect(csvCell('  padded  ')).toBe('"  padded  "');
  });

  it('renders null and undefined as an empty field', () => {
    expect(csvCell(null)).toBe('');
    expect(csvCell(undefined)).toBe('');
  });

  it('leaves ordinary text alone', () => {
    expect(csvCell('Lee Wei')).toBe('Lee Wei');
    expect(csvCell('')).toBe('');
  });
});

describe('csvCell formula neutralisation', () => {
  it('prefixes every leading byte a spreadsheet executes', () => {
    // Same set as backend csv_helpers::neutralize_formula.
    for (const value of ['=1+1', '+cmd|\' /C calc\'!A0', '-2+3', '@SUM(1,2)', '\t=1+1', '\r=1+1']) {
      const cell = csvCell(value);
      const inner = cell.startsWith('"') ? cell.slice(1, -1) : cell;
      expect(inner.startsWith("'"), `not neutralized: ${JSON.stringify(value)}`).toBe(true);
    }
  });

  it('does not prefix a negative number', () => {
    // The carve-out that keeps the helper usable on a money column: without it
    // every negative amount would export as text and break the sheet.
    expect(csvCell('-1700.00')).toBe('-1700.00');
    expect(csvCell('-5')).toBe('-5');
    expect(csvCell(-5)).toBe('-5');
    expect(csvCell('-1.5e3')).toBe('-1.5e3');
  });
});

describe('senToPlainRm', () => {
  it('never groups thousands and always keeps two decimals', () => {
    expect(senToPlainRm(170_000)).toBe('1700.00');
    expect(senToPlainRm(0)).toBe('0.00');
    expect(senToPlainRm(5)).toBe('0.05');
    expect(senToPlainRm(50)).toBe('0.50');
  });

  it('keeps the sign and stays parseable', () => {
    expect(senToPlainRm(-170_000)).toBe('-1700.00');
    expect(senToPlainRm(-1)).toBe('-0.01');
    expect(Number.isFinite(Number(senToPlainRm(-170_000)))).toBe(true);
  });
});

describe('toCsv', () => {
  it('separates records with CRLF per RFC 4180', () => {
    expect(toCsv(['a', 'b'], [['1', '2']])).toBe('a,b\r\n1,2');
  });

  it('keeps exactly one field per column for adversarial input', () => {
    const headers = ['Employee', 'Department', 'Amount'];
    const rows = [
      ['Ali "Bob" Chan', 'Sales, EMEA', '-1700.00'],
      ['=cmd()', 'R&D\nLab', '0.00'],
      [null, undefined, 12],
    ];

    const parsed = parseCsv(toCsv(headers, rows));

    expect(parsed).toHaveLength(4);
    for (const record of parsed) {
      expect(record).toHaveLength(headers.length);
    }
    expect(parsed[1]).toEqual(['Ali "Bob" Chan', 'Sales, EMEA', '-1700.00']);
    expect(parsed[2][0]).toBe("'=cmd()");
    expect(parsed[2][1]).toBe('R&D\nLab');
    expect(parsed[3]).toEqual(['', '', '12']);
  });
});
