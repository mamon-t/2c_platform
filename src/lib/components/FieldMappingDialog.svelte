<script lang="ts">
  /** Диалог сопоставления полей при импорте. */
  export interface MappingColumn {
    source: string;
    sample_value: string;
    matched_target: string | null;
  }

  export interface MappingTarget {
    code: string;
    name: string;
  }

  interface Props {
    columns: MappingColumn[];
    targets: MappingTarget[];
    title?: string;
    onApply: (mapping: Record<string, string>) => void;
    onCancel: () => void;
  }

  let { columns, targets, title = 'Сопоставление полей', onApply, onCancel }: Props = $props();

  let mapping = $state<Record<string, string>>({});
  let initialised = false;

  $effect(() => {
    if (!initialised && columns.length > 0) {
      const m: Record<string, string> = {};
      for (const col of columns) {
        if (col.matched_target) m[col.source] = col.matched_target;
      }
      mapping = m;
      initialised = true;
    }
  });

  function setMapping(source: string, value: string) {
    if (value) mapping[source] = value;
    else delete mapping[source];
    mapping = { ...mapping };
  }

  function fieldLabel(code: string): string {
    return targets.find((t) => t.code === code)?.name ?? code;
  }
</script>

<div class="fixed inset-0 bg-black/50 z-[60] grid place-items-center" role="presentation">
  <div class="card p-5 w-[560px] max-h-[80vh] overflow-y-auto space-y-3 bg-surface-50-950" onclick={(e) => e.stopPropagation()} role="dialog">
    <h3 class="font-semibold text-base"><i class="fa-solid fa-right-left mr-2"></i>{title}</h3>
    <p class="text-xs text-surface-500">
      Сопоставьте колонки файла с полями документа.
    </p>

    <table class="table table-sm">
      <thead><tr><th>Колонка файла</th><th>Пример</th><th>Поле документа</th></tr></thead>
      <tbody>
        {#each columns as col, i (col.source + String(i))}
          <tr>
            <td class="font-mono text-xs">{col.source}</td>
            <td class="text-xs text-surface-400 truncate max-w-24">{col.sample_value}</td>
            <td>
              <select
                class="select select-sm w-full"
                value={mapping[col.source] ?? ''}
                onchange={(e) => setMapping(col.source, (e.target as HTMLSelectElement).value)}
              >
                <option value="" disabled>— пропустить —</option>
                {#each targets as t}
                  <option value={t.code}>{fieldLabel(t.code)}</option>
                {/each}
              </select>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>

    <div class="flex justify-end gap-2 pt-2">
      <button class="btn btn-outline" onclick={() => onCancel()}>Отмена</button>
      <button class="btn btn-primary" onclick={() => onApply({ ...mapping })}>
        <i class="fa-solid fa-check"></i> Применить ({Object.keys(mapping).length})
      </button>
    </div>
  </div>
</div>
