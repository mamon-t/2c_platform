// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    icon?: string;
    title: string;
    description?: string;
    children?: Snippet;
  }
  let { icon = 'fa-regular fa-folder-open', title, description = '', children }: Props = $props();
</script>

<div class="grid place-items-center py-12 text-center">
  <i class="{icon} mb-3 text-4xl text-surface-300 dark:text-surface-700"></i>
  <p class="text-sm font-medium text-surface-500">{title}</p>
  {#if description}<p class="mt-1 max-w-md text-xs text-surface-400">{description}</p>{/if}
  {#if children}<div class="mt-3">{@render children()}</div>{/if}
</div>
