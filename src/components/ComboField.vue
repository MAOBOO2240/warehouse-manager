<script setup>
// 可自行輸入、也可從清單挑選的欄位（取代系統 datalist 與瀏覽器記憶的輸入建議）
// 清單外觀與「專案切換」相同；點欄位或開始輸入時才展開
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

const props = defineProps({
  modelValue: { type: String, default: '' },
  suggestions: { type: Array, default: () => [] }, // 字串陣列
  id: { type: String, default: undefined },
  title: { type: String, default: '' },
  placeholder: { type: String, default: '' },
  maxlength: { type: [String, Number], default: undefined },
})
const emit = defineEmits(['update:modelValue'])

const open = ref(false)
const active = ref(-1)
const root = ref(null)
const inputEl = ref(null)
const listEl = ref(null)
const panelEl = ref(null)
const pos = ref({})

// 欄位內容和某個選項完全相同時顯示全部，方便改選；否則依輸入的字篩選
const items = computed(() => {
  const k = props.modelValue.trim().toLowerCase()
  if (!k || props.suggestions.some((s) => s.toLowerCase() === k)) return props.suggestions
  return props.suggestions.filter((s) => s.toLowerCase().includes(k))
})
const shown = computed(() => open.value && items.value.length > 0)

watch(items, () => {
  if (active.value >= items.value.length) active.value = items.value.length - 1
})

// 清單掛在 body 上以 fixed 定位，避免被彈窗的捲動區裁切；畫面有整體縮放，需換算回未縮放的 px
function place() {
  const el = inputEl.value
  if (!el) return
  const r = el.getBoundingClientRect()
  const k = el.offsetHeight ? r.height / el.offsetHeight : 1
  const vh = window.innerHeight
  const below = vh - r.bottom - 12
  const above = r.top - 12
  const up = below < 200 && above > below
  pos.value = {
    left: `${r.left / k}px`,
    width: `${r.width / k}px`,
    maxHeight: `${Math.max(140, Math.min(360, (up ? above : below) - 8)) / k}px`,
    ...(up ? { bottom: `${(vh - r.top) / k + 8}px` } : { top: `${r.bottom / k + 8}px` }),
  }
}
watch(open, (v) => {
  if (v) {
    place()
    window.addEventListener('scroll', place, true)
    window.addEventListener('resize', place)
  } else {
    window.removeEventListener('scroll', place, true)
    window.removeEventListener('resize', place)
  }
})

function show() {
  open.value = true
  active.value = items.value.indexOf(props.modelValue)
}
function close() {
  open.value = false
  active.value = -1
}

function onInput(e) {
  emit('update:modelValue', e.target.value)
  open.value = true
  active.value = -1
}

function choose(s) {
  emit('update:modelValue', s)
  close()
  inputEl.value?.focus()
}

async function scrollActive() {
  await nextTick()
  listEl.value?.querySelector('[data-active="true"]')?.scrollIntoView({ block: 'nearest' })
}

function onKey(e) {
  const n = items.value.length
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault()
    if (!open.value) return show()
    if (!n) return
    const step = e.key === 'ArrowDown' ? 1 : -1
    active.value = active.value < 0 ? (step > 0 ? 0 : n - 1) : (active.value + step + n) % n
    scrollActive()
  } else if (e.key === 'Enter') {
    if (shown.value && active.value >= 0) {
      e.preventDefault()
      choose(items.value[active.value])
    } else {
      close()
    }
  } else if (e.key === 'Escape') {
    if (shown.value) {
      e.preventDefault()
      e.stopPropagation()
      close()
    }
  } else if (e.key === 'Tab') {
    close()
  }
}

function onDocDown(e) {
  if (!open.value) return
  if (root.value?.contains(e.target) || panelEl.value?.contains(e.target)) return
  close()
}
onMounted(() => document.addEventListener('mousedown', onDocDown))
onBeforeUnmount(() => {
  document.removeEventListener('mousedown', onDocDown)
  window.removeEventListener('scroll', place, true)
  window.removeEventListener('resize', place)
})

defineExpose({
  focus: () => inputEl.value?.focus(),
  select: () => inputEl.value?.select(),
})
</script>

<template>
  <div ref="root" class="relative">
    <input
      :id="id"
      ref="inputEl"
      :value="modelValue"
      class="field pr-11"
      :maxlength="maxlength"
      :placeholder="placeholder"
      autocomplete="off"
      role="combobox"
      :aria-expanded="shown"
      aria-autocomplete="list"
      @input="onInput"
      @mousedown="open ? null : show()"
      @keydown="onKey"
    />
    <button
      v-if="suggestions.length"
      type="button"
      tabindex="-1"
      class="absolute top-1/2 right-2 flex h-8 w-8 -translate-y-1/2 items-center justify-center rounded-full text-mute hover:bg-hover hover:text-ink"
      aria-label="顯示已使用過的選項"
      @mousedown.prevent
      @click="open ? close() : (show(), inputEl?.focus())"
    >
      <svg
        viewBox="0 0 24 24"
        class="h-4 w-4 transition-transform"
        :class="shown ? 'rotate-180' : ''"
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

    <Teleport to="body">
      <div v-if="shown" ref="panelEl" class="menu-panel fixed z-[70] flex flex-col p-2" :style="pos">
        <div v-if="title" class="px-3 pt-1 pb-2 text-[12.5px] font-semibold text-mute">{{ title }}</div>
        <ul ref="listEl" class="min-h-0 overflow-y-auto overscroll-contain" role="listbox">
          <li v-for="(s, i) in items" :key="s">
            <button
              type="button"
              role="option"
              tabindex="-1"
              :aria-selected="s === modelValue"
              :data-active="i === active"
              class="flex w-full items-center gap-3 rounded-2xl px-3 py-2.5 text-left text-[15.5px] transition-colors"
              :class="[
                s === modelValue ? 'font-semibold text-accent' : 'text-ink',
                i === active ? 'bg-accent-soft' : 'hover:bg-hover',
              ]"
              @mousedown.prevent
              @mousemove="active = i"
              @click="choose(s)"
            >
              <span class="flex h-5 w-5 shrink-0 items-center justify-center" aria-hidden="true">
                <svg v-if="s === modelValue" viewBox="0 0 24 24" class="h-[18px] w-[18px]" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M5 12.5l4.5 4.5L19 7.5" />
                </svg>
              </span>
              <span class="min-w-0 flex-1 truncate">{{ s }}</span>
            </button>
          </li>
        </ul>
      </div>
    </Teleport>
  </div>
</template>
