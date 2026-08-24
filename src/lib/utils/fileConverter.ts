// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

/**
 * Конвертер файлов для импорта/экспорта табличных данных и документов.
 *
 * Поддерживаемые форматы: CSV, JSON, XML.
 * YAML — через convert-плагин (нужен WASM рантайм).
 */

// ── Типы ──────────────────────────────────────────────────

export type FileFormat = 'csv' | 'json' | 'xml';

export interface ParseResult {
  rows: Record<string, unknown>[];
  columns: string[];
}

// ── CSV парсер (RFC 4180) ────────────────────────────────

/** Разобрать CSV-строку на массив строк с учётом кавычек. */
function splitCsvLine(line: string, delimiter: string): string[] {
  const out: string[] = [];
  let cur = '';
  let inQuotes = false;
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (inQuotes) {
      if (ch === '"') {
        if (i + 1 < line.length && line[i + 1] === '"') { cur += '"'; i++; }
        else inQuotes = false;
      } else cur += ch;
    } else if (ch === '"') {
      inQuotes = true;
    } else if (ch === delimiter) {
      out.push(cur.trim()); cur = '';
    } else cur += ch;
  }
  out.push(cur.trim());
  return out;
}

/** Определить разделитель CSV (запятая или точка с запятой). */
function detectDelimiter(text: string): string {
  const first = text.split(/\r?\n/)[0] ?? '';
  const commas = (first.match(/,/g) ?? []).length;
  const semis = (first.match(/;/g) ?? []).length;
  return semis > commas ? ';' : ',';
}

/** Парсить CSV в массив объектов. Первая строка = заголовки. */
export function parseCsv(text: string): Record<string, unknown>[] {
  const delimiter = detectDelimiter(text);
  const lines = text.split(/\r?\n/).filter((l) => l.trim());
  if (lines.length < 2) return [];

  const headers = splitCsvLine(lines[0], delimiter);
  const rows: Record<string, unknown>[] = [];

  for (let i = 1; i < lines.length; i++) {
    const values = splitCsvLine(lines[i], delimiter);
    const row: Record<string, unknown> = {};
    headers.forEach((h, j) => { row[h] = values[j] ?? ''; });
    rows.push(row);
  }
  return rows;
}

/** Сериализовать объекты в CSV. */
export function toCsv(rows: Record<string, unknown>[], columns?: string[]): string {
  if (!rows.length) return '';
  const cols = columns ?? Object.keys(rows[0]);
  const esc = (v: unknown): string => {
    const s = String(v ?? '');
    return s.includes(',') || s.includes('"') || s.includes('\n') ? `"${s.replace(/"/g, '""')}"` : s;
  };
  const header = cols.map(esc).join(',');
  const body = rows.map((r) => cols.map((c) => esc(r[c])).join(','));
  return [header, ...body].join('\n');
}

// ── JSON ──────────────────────────────────────────────────

export function parseJson(text: string): Record<string, unknown>[] {
  const v = JSON.parse(text);
  if (Array.isArray(v)) return v as Record<string, unknown>[];
  if (v && typeof v === 'object') {
    // Если объект имеет массив внутри (например data.lines)
    for (const val of Object.values(v)) {
      if (Array.isArray(val) && val.length > 0 && typeof val[0] === 'object')
        return val as Record<string, unknown>[];
    }
    return [v as Record<string, unknown>];
  }
  return [];
}

export function toJson(rows: Record<string, unknown>[], pretty = true): string {
  return JSON.stringify(rows, null, pretty ? 2 : undefined);
}

// ── XML ───────────────────────────────────────────────────

/** Парсить простой XML (<row><field>value</field></row>) в объекты. */
export function parseXml(text: string): Record<string, unknown>[] {
  const parser = new DOMParser();
  const doc = parser.parseFromString(text, 'text/xml');
  const err = doc.querySelector('parsererror');
  if (err) throw new Error('Невалидный XML');

  const root = doc.documentElement;
  const rowTags = root.children.length > 0
    ? Array.from(root.children)
    : [root];

  const rows: Record<string, unknown>[] = [];
  for (const rowEl of rowTags) {
    const row: Record<string, unknown> = {};
    for (const child of Array.from(rowEl.children)) {
      row[child.tagName] = child.textContent?.trim() ?? '';
    }
    if (Object.keys(row).length > 0) rows.push(row);
  }
  return rows;
}

/** Сериализовать объекты в XML. */
export function toXml(rows: Record<string, unknown>[], rootTag = 'rows', rowTag = 'row'): string {
  const escXml = (v: unknown): string =>
    String(v ?? '').replace(/&/g, '&amp;').replace(/</g, '&lt;')
      .replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  const inner = rows.map((r) => {
    const fields = Object.entries(r)
      .map(([k, v]) => `<${k}>${escXml(v)}</${k}>`)
      .join('');
    return `  <${rowTag}>${fields}</${rowTag}>`;
  });
  return `<${rootTag}>\n${inner.join('\n')}\n</${rootTag}>`;
}

// ── Универсальный интерфейс ───────────────────────────────

/** Парсить файл по расширению. */
export function parseFile(text: string, format: FileFormat): ParseResult {
  switch (format) {
    case 'csv': {
      const rows = parseCsv(text);
      return { rows, columns: rows.length > 0 ? Object.keys(rows[0]) : [] };
    }
    case 'json': {
      const rows = parseJson(text);
      return { rows, columns: rows.length > 0 ? Object.keys(rows[0]) : [] };
    }
    case 'xml': {
      const rows = parseXml(text);
      return { rows, columns: rows.length > 0 ? Object.keys(rows[0]) : [] };
    }
  }
}

/** Сериализовать строки в формат. */
export function serializeFile(
  rows: Record<string, unknown>[],
  format: FileFormat,
  rootTag?: string,
): string {
  switch (format) {
    case 'csv': return toCsv(rows);
    case 'json': return toJson(rows);
    case 'xml': return toXml(rows, rootTag ?? 'rows', rootTag === 'rows' ? 'item' : 'row');
  }
}

/** Скачать текст как файл. */
export function downloadText(content: string, filename: string, mime = 'text/plain'): void {
  const blob = new Blob(['\uFEFF' + content], { type: `${mime};charset=utf-8` });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url; a.download = filename; a.click();
  URL.revokeObjectURL(url);
}

/** Прочитать файл как текст. */
export function readFileAsText(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = reject;
    reader.readAsText(file, 'utf-8');
  });
}
