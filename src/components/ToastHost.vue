<script setup>
import { useUiStore } from '../stores/ui'
const ui = useUiStore()
</script>

<template>
  <div class="pointer-events-none fixed right-5 bottom-5 z-[60] flex w-96 flex-col gap-2" aria-live="polite">
    <div
      v-for="t in ui.toasts"
      :key="t.id"
      class="pointer-events-auto rounded-md border-l-4 bg-white px-4 py-3 text-[14px] shadow-lg whitespace-pre-line"
      :class="t.type === 'error' ? 'border-signal text-signal' : 'border-go text-ink'"
    >
      {{ t.text }}
    </div>
  </div>
</template>
