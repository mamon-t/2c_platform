// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

/**
 * Каноничные подписываемые строки модуля «Заявки» (контракт SDK ≥1.2).
 *
 * ФРОНТ и ПЛАГИН собирают ИДЕНТИЧНЫЕ строки. Плагин верифицирует
 * CMS-подпись против этой строки через host-fn cms_verify (КриптоПро).
 *
 * Форматы:
 *   submit: `requests.submit|{id}|{version}|{state}`
 *   decide: `requests.decide|{id}|{approve|reject}|{comment}`
 *
 * Неизменность данных гарантируется оптимистичной блокировкой версии:
 * строка связывает решение с конкретным состоянием объекта.
 */

export function canonicalSubmitPayload(o: {
  id: string;
  version: number;
  state: string;
}): string {
  return ['requests.submit', o.id, String(o.version), o.state].join('|');
}

export function canonicalDecisionPayload(
  requestId: string,
  approve: boolean,
  comment: string | null | undefined,
): string {
  return [
    'requests.decide',
    requestId,
    approve ? 'approve' : 'reject',
    comment ?? '',
  ].join('|');
}
