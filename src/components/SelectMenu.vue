<script setup>
// 自訂下拉選單：外觀與操作方式和「專案切換」相同（取代系統原生 <select>）
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'

const props = defineProps({
  modelValue: { type: String, default: '' },
  options: { type: Array, required: true }, // [{ value, label }]
  id: { type: String, default: undefined },
  title: { type: String, default: '' }, // 清單上方的小標題
  disabled: { type: Boolean, default: false },
})
const emit = defineEmits(['update:modelValue'])

const open = ref(false)
const active = ref(0)
const root = ref(null)
const listEl = ref(null)
const btnEl = ref(null)

const currentIndex = computed(() => Math.max(0, props.options.findIndex((o) => o.value === props.modelValue)))
const currentLabel = computed(() => props.options.find((o) => o.value === props.modelValue)?.label ?? '')

async function toggle() {
  if (props.disabled) return
  open.value = !open.value
  if (open.value) {
    active.value = currentIndex.value
    await nextTick()
    scrollActive()
  }
}

function close(refocus = false) {
  open.value = false
  if (refocus) btnEl.value?.focus()
}

function choose(o) {
  close(true)
  if (o.value !== props.modelValue) emit('update:modelValue', o.value)
}

function scrollActive() {
  listEl.value?.querySelector('[data-active="true"]')?.scrollIntoView({ block: 'nearest' })
}

async function onKey(e) {
  if (!open.value) {
    if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key)) {
      e.preventDefault()
      toggle()
    }
    return
  }
  const n = props.options.length
  if (e.key === 'Escape') {
    e.preventDefault()
    e.stopPropagation()
    close(true)
  } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault()
    if (!n) return
    active.value = (active.value + (e.key === 'ArrowDown' ? 1 : -1) + n) % n
    await nextTick()
    scrollActive()
  } else if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault()
    if (n) choose(props.options[active.value])
  } else if (e.key === 'Tab') {
    close()
  }
}

function onDocDown(e) {
  if (open.value && root.value && !root.value.contains(e.target)) close()
}
onMounted(() => document.addEventListener('mousedown', onDocDown))
onBeforeUnmount(() => document.removeEventListener('mousedown', onDocDown))
</script>

<template>
  <div ref="root" class="relative" @keydown="onKey">
    <button
      :id="id"
      ref="btnEl"
      type="button"
      class="menu-trigger flex w-full items-center gap-2.5 rounded-full py-2.5 pr-3 pl-4 text-left text-[15px] font-semibold text-ink"
      :class="open ? 'is-open' : ''"
      :disabled="disabled"
      aria-haspopup="listbox"
      :aria-expanded="open"
      @click="toggle"
    >
      <span class="min-w-0 flex-1 truncate">{{ currentLabel }}</span>
      <svg
        viewBox="0 0 24 24"
        class="h-4 w-4 shrink-0 text-mute transition-transform"
        :class="open ? 'rotate-180' : ''"
        fill="none"
        stroke="currentColor"
        stroke-width="2.4"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M6 9l6 6 6-6" />
      </svg>
    </button>

    <div v-if="open" class="menu-panel absolute top-full left-0 z-40 mt-2 w-full min-w-56 p-2">
      <div v-if="title" class="px-3 pt-1 pb-2 text-[12.5px] font-semibold text-mute">{{ title }}</div>
      <ul ref="listEl" class="max-h-80 overflow-y-auto overscroll-contain" role="listbox">
        <li v-for="(o, i) in options" :key="o.value">
          <button
            type="button"
            role="option"
            :aria-selected="o.value === modelValue"
            :data-active="i === active"
            class="flex w-full items-center gap-3 rounded-2xl px-3 py-3 text-left text-[15.5px] transition-colors"
            :class="[
              o.value === modelValue ? 'font-semibold text-accent' : 'text-ink',
              i === active ? 'bg-accent-soft' : 'hover:bg-hover',
            ]"
            @mousemove="active = i"
            @click="choose(o)"
          >
            <span class="flex h-5 w-5 shrink-0 items-center justify-center" aria-hidden="true">
              <svg v-if="o.value === modelValue" viewBox="0 0 24 24" class="h-[18px] w-[18px]" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round">
                <path d="M5 12.5l4.5 4.5L19 7.5" />
              </svg>
            </span>
            <span class="min-w-0 flex-1 truncate">{{ o.label }}</span>
          </button>
        </li>
      </ul>
    </div>
  </div>
</template>
