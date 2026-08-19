import { getAdapter } from '$lib/adapters/transport';

export interface DiagnosticsReport {
  app_version: string;
  mongodb: {
    connected: boolean;
    host: string;
    version: string | null;
    replica_set: string | null;
    ok: boolean;
  };
  modules: Array<{
    code: string;
    name: string;
    version: string;
    active: boolean;
  }>;
}

export interface RhaiValidationResult {
  valid: boolean;
  error?: string;
}

export const api = {
  async getDiagnostics(): Promise<DiagnosticsReport> {
    return getAdapter().invoke<DiagnosticsReport>('get_diagnostics');
  },

  async validateRhaiScript(source: string): Promise<void> {
    return getAdapter().invoke<void>('validate_rhai_script', { source });
  },

  async executeRhaiScript(
    source: string,
    context: string
  ): Promise<unknown> {
    return getAdapter().invoke<unknown>('execute_rhai_script', { source, context });
  },
};
