<script setup>
import { onBeforeUnmount, onMounted } from 'vue'

const props = defineProps({
  title: { type: String, default: '' },
  width: { type: String, default: 'max-w-lg' },
  closable: { type: Boolean, default: true },
})
const emit = defineEmits(['close'])

function onKey(e) {
  if (e.key === 'Escape' && props.closable) emit('close')
}
onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <Teleport to="body">
    <div class="fixed inset-0 z-50 flex items-center justify-center bg-ink/45 p-6" @mousedown.self="closable && emit('close')">
      <div class="w-full rounded-lg bg-white shadow-2xl" :class="width" role="dialog" aria-modal="true">
        <div class="flex items-center justify-between border-b border-line px-5 py-3.5">
          <h2 class="text-base font-bold">{{ title }}</h2>
          <button v-if="closable" type="button" class="rounded p-1 text-mute hover:bg-surface" aria-label="關閉" @click="emit('close')">
            <svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2"><path d="M6 6l12 12M18 6L6 18" /></svg>
          </button>
        </div>
        <div class="max-h-[70vh] overflow-auto px-5 py-4">
          <slot />
        </div>
        <div v-if="$slots.footer" class="flex justify-end gap-2 border-t border-line bg-surface/60 px-5 py-3">
          <slot name="footer" />
        </div>
      </div>
    </div>
  </Teleport>
</template>
