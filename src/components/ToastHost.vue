<script setup>
import { useUiStore } from '../stores/ui'
const ui = useUiStore()
</script>

<template>
  <div class="pointer-events-none fixed right-6 bottom-6 z-[60] flex w-96 flex-col gap-2.5" aria-live="polite">
    <div
      v-for="t in ui.toasts"
      :key="t.id"
      class="panel pointer-events-auto flex items-start gap-3 rounded-2xl px-4 py-3 text-[14px] whitespace-pre-line"
    >
      <span class="mt-1.5 h-2.5 w-2.5 shrink-0 rounded-full" :class="t.type === 'error' ? 'bg-signal' : 'bg-go'" aria-hidden="true" />
      <span :class="t.type === 'error' ? 'text-signal' : 'text-ink'">{{ t.text }}</span>
    </div>
  </div>
</template>
