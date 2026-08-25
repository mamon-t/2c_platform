// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

/**
 * Валидация данных по метамодели (порт validate_field_value из Rust).
 * Используется DataMapper для пред-импортной проверки.
 */

import type { FieldKind } from '$lib/services/api';

export interface EntityFieldMeta {
  code: string;
  name: string;
  field_kind: FieldKind;
  is_required: boolean;
  is_readonly: boolean;
  enum_values?: string[];
  reference_entity?: string;
}

export interface FieldValidationError {
  field_code: string;
  field_name: string;
  message: string;
  row?: number;
}

// ── UUID helpers ──────────────────────────────────────────

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function isValidUuid(s: string): boolean {
  return UUID_RE.test(s);
}

// ── Date helpers ──────────────────────────────────────────

function isValidDate(s: string): boolean {
  if (s.length !== 10 || s[4] !== '-' || s[7] !== '-') return false;
  const d = new Date(s);
  return !isNaN(d.getTime());
}

function isValidDateTime(s: string): boolean {
  return !isNaN(Date.parse(s));
}

// ── Основная функция валидации ────────────────────────────

export function validateFieldValue(
  value: unknown,
  field: EntityFieldMeta,
): string | null {
  if (value === null || value === undefined || value === '') {
    if (field.is_required) return 'обязательное поле';
    return null;
  }

  switch (field.field_kind) {
    case 'string': {
      if (typeof value !== 'string' && typeof value !== 'number' && typeof value !== 'boolean') {
        return 'ожидается строка или число';
      }
      return null;
    }
    case 'text': {
      if (typeof value !== 'string') return 'ожидается строка';
      return null;
    }
    case 'integer': {
      if (typeof value === 'number') {
        if (!Number.isInteger(value)) return 'ожидается целое число';
        return null;
      }
      if (typeof value === 'string') {
        if (/^-?\d+$/.test(value)) return null;
        return 'ожидается целое число';
      }
      return 'ожидается целое число';
    }
    case 'money': {
      if (typeof value === 'number') return null;
      if (typeof value === 'string') {
        const normalized = value.replace(',', '.').replace(/\s/g, '');
        if (!isNaN(Number(normalized))) return null;
        return 'невалидное денежное значение';
      }
      return 'ожидается денежная сумма (число или строка)';
    }
    case 'date': {
      if (typeof value !== 'string') return 'ожидается строка даты';
      if (!isValidDate(value)) return 'невалидная дата (ожидается YYYY-MM-DD)';
      return null;
    }
    case 'datetime': {
      if (typeof value !== 'string') return 'ожидается строка даты/времени';
      if (!isValidDateTime(value)) return 'ожидается формат ISO 8601';
      return null;
    }
    case 'boolean': {
      if (typeof value !== 'boolean') return 'ожидается boolean';
      return null;
    }
    case 'enum': {
      if (typeof value !== 'string') return 'ожидается строковое значение перечисления';
      if (field.enum_values && field.enum_values.length > 0) {
        if (!field.enum_values.includes(value)) {
          return `допустимые значения: ${field.enum_values.join(', ')}`;
        }
      }
      return null;
    }
    case 'reference': {
      if (typeof value === 'string') {
        if (isValidUuid(value) || value === '') return null;
        return 'ожидается UUID ссылки';
      }
      if (value === null) return null;
      return 'ожидается UUID или null';
    }
    case 'array': {
      if (!Array.isArray(value)) return 'ожидается массив';
      return null;
    }
    case 'table': {
      if (!Array.isArray(value)) return 'ожидается массив объектов (таблица)';
      for (let i = 0; i < value.length; i++) {
        if (typeof value[i] !== 'object' || value[i] === null || Array.isArray(value[i])) {
          return `таблица: строка ${i + 1} — ожидается объект`;
        }
      }
      return null;
    }
    case 'json':
    case 'file':
    case 'user':
    case 'company':
    case 'formula':
    case 'computed':
      return null;
  }
}

// ── Валидация всего объекта ────────────────────────────────

export function validateObjectData(
  data: Record<string, unknown>,
  fields: EntityFieldMeta[],
): FieldValidationError[] {
  const errors: FieldValidationError[] = [];

  for (const field of fields) {
    const value = data[field.code];
    const err = validateFieldValue(value, field);
    if (err) {
      errors.push({
        field_code: field.code,
        field_name: field.name,
        message: err,
      });
    }
  }

  return errors;
}

// ── Предпросмотр значений для формы маппинга ──────────────

export function coerceForImport(
  value: unknown,
  fieldKind: FieldKind,
): unknown {
  if (value === null || value === undefined || value === '') return null;

  switch (fieldKind) {
    case 'integer': {
      const n = Number(String(value).replace(/\s/g, ''));
      return Number.isInteger(n) ? n : null;
    }
    case 'money': {
      const s = String(value).replace(',', '.').replace(/\s/g, '');
      const n = Number(s);
      return isNaN(n) ? null : n;
    }
    case 'boolean': {
      const s = String(value).toLowerCase();
      if (['true', '1', 'да', 'yes'].includes(s)) return true;
      if (['false', '0', 'нет', 'no'].includes(s)) return false;
      return null;
    }
    default:
      return value;
  }
}
