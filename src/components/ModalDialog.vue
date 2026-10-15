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
    <div
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/45 p-6 backdrop-blur-sm"
      @mousedown.self="closable && emit('close')"
    >
      <div class="panel w-full overflow-hidden" :class="width" role="dialog" aria-modal="true" :aria-label="title">
        <div class="flex items-center justify-between px-6 pt-5 pb-2">
          <h2 class="text-[17px] font-bold">{{ title }}</h2>
          <button
            v-if="closable"
            type="button"
            class="flex h-8 w-8 items-center justify-center rounded-full bg-raised text-mute hover:bg-hover hover:text-ink"
            aria-label="關閉"
            @click="emit('close')"
          >
            <svg viewBox="0 0 24 24" class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
          </button>
        </div>
        <div class="max-h-[70vh] overflow-auto px-6 py-4">
          <slot />
        </div>
        <div v-if="$slots.footer" class="flex justify-end gap-2.5 px-6 pt-2 pb-5">
          <slot name="footer" />
        </div>
      </div>
    </div>
  </Teleport>
</template>
