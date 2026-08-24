// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

export interface TransportAdapter {
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
}

export class TauriAdapter implements TransportAdapter {
  async invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(command, args);
  }
}

export class HttpAdapter implements TransportAdapter {
  private baseUrl: string;

  constructor(baseUrl: string) {
    this.baseUrl = baseUrl;
  }

  async invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    const response = await fetch(`${this.baseUrl}/api/${command}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(args ?? {}),
    });

    if (!response.ok) {
      const text = await response.text();
      throw new Error(`Ошибка сервера (${response.status}): ${text}`);
    }

    return response.json();
  }
}

export class MockAdapter implements TransportAdapter {
  private handlers: Map<string, (args: Record<string, unknown>) => unknown> = new Map();

  registerHandler(command: string, handler: (args: Record<string, unknown>) => unknown) {
    this.handlers.set(command, handler);
  }

  async invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    const handler = this.handlers.get(command);
    if (!handler) {
      throw new Error(`Неизвестная команда: ${command}`);
    }
    return handler(args ?? {}) as T;
  }
}

let adapter: TransportAdapter = new TauriAdapter();

export function setAdapter(a: TransportAdapter) {
  adapter = a;
}

export function getAdapter(): TransportAdapter {
  return adapter;
}
