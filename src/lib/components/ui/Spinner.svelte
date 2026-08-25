<!-- 2CPlatform - Copyright (c) 2026 Mikhail Alekseev
     This code is proprietary. See LICENSE file for details. -->

<script lang="ts">
  interface Props {
    label?: string;
    size?: 'sm' | 'md' | 'lg';
  }
  let { label = 'Загрузка…', size = 'md' }: Props = $props();
</script>

<div class="grid place-items-center py-10 text-surface-400" role="status">
  <i class="fa-solid fa-circle-notch fa-spin {size === 'sm' ? 'text-base' : size === 'lg' ? 'text-3xl' : 'text-xl'}"></i>
  {#if size !== 'sm'}<span class="mt-2 text-xs">{label}</span>{/if}
</div>
