<script setup>
import { computed, onBeforeUnmount, onMounted } from 'vue'

const props = defineProps({
  title: { type: String, default: '' },
  subtitle: { type: String, default: '' },
  width: { type: String, default: 'max-w-lg' },
  closable: { type: Boolean, default: true },
  /** 主題色：accent / go / signal / safety / violet，影響色帶、外框、欄位標題圓點與焦點色 */
  tone: { type: String, default: 'accent' },
})
const emit = defineEmits(['close'])

const toneStyle = computed(() => ({ '--tone': `var(--color-${props.tone})`, '--tone-fg': `var(--color-on-${props.tone}, #fff)` }))

function onKey(e) {
  if (e.key === 'Escape' && props.closable) emit('close')
}
onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <Teleport to="body">
    <div
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/55 p-6 backdrop-blur-[3px]"
      @mousedown.self="closable && emit('close')"
    >
      <div class="modal-panel" :class="width" :style="toneStyle" role="dialog" aria-modal="true" :aria-label="title">
        <div class="modal-head flex items-center gap-3.5 px-6 py-4">
          <div v-if="$slots.icon" class="modal-icon" aria-hidden="true">
            <slot name="icon" />
          </div>
          <div class="min-w-0 flex-1">
            <h2 class="truncate text-[18px] font-bold">{{ title }}</h2>
            <p v-if="subtitle" class="mt-0.5 truncate text-[13px] text-ink-2">{{ subtitle }}</p>
          </div>
          <button
            v-if="closable"
            type="button"
            class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-panel/70 text-mute ring-1 ring-line hover:bg-hover hover:text-ink"
            aria-label="關閉"
            @click="emit('close')"
          >
            <svg viewBox="0 0 24 24" class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
          </button>
        </div>
        <div class="max-h-[70vh] overflow-auto px-6 py-5">
          <slot />
        </div>
        <div v-if="$slots.footer" class="modal-foot flex justify-end gap-2.5 px-6 py-4">
          <slot name="footer" />
        </div>
      </div>
    </div>
  </Teleport>
</template>
