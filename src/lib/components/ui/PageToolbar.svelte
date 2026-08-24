// 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
// This code is proprietary. See LICENSE file for details.

<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    title: string;
    icon?: string;
    children?: Snippet;
  }
  let { title, icon = '', children }: Props = $props();
</script>

<div class="flex flex-wrap items-center gap-2 border-b border-surface-300-700 px-3 py-2">
  {#if icon}<i class="{icon} text-base text-surface-400"></i>{/if}
  <h2 class="text-sm font-semibold">{title}</h2>
  {#if children}
    <div class="ml-auto flex flex-wrap items-center gap-2">{@render children()}</div>
  {/if}
</div>
