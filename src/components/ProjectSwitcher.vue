<script setup>
// 專案切換：自訂彈出清單（取代系統下拉選單），目前專案打勾，可用方向鍵與 Enter 操作
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'

const props = defineProps({
  projects: { type: Array, required: true },
  current: { type: String, default: '' },
  disabled: { type: Boolean, default: false },
})
const emit = defineEmits(['select'])

const open = ref(false)
const active = ref(0)
const root = ref(null)
const listEl = ref(null)
const btnEl = ref(null)

const currentIndex = computed(() => Math.max(0, props.projects.indexOf(props.current)))

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

function choose(name) {
  close(true)
  if (name !== props.current) emit('select', name)
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
  const n = props.projects.length
  if (e.key === 'Escape') {
    e.preventDefault()
    close(true)
  } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault()
    if (!n) return
    active.value = (active.value + (e.key === 'ArrowDown' ? 1 : -1) + n) % n
    await nextTick()
    scrollActive()
  } else if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault()
    if (n) choose(props.projects[active.value])
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
      id="project-select"
      ref="btnEl"
      type="button"
      class="menu-trigger flex w-64 items-center gap-2.5 rounded-full py-2 pr-3 pl-3.5 text-left text-[15px] font-semibold"
      :class="open ? 'is-open' : ''"
      :disabled="disabled"
      aria-haspopup="listbox"
      :aria-expanded="open"
      @click="toggle"
    >
      <svg viewBox="0 0 24 24" class="h-[18px] w-[18px] shrink-0 text-accent" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round" aria-hidden="true">
        <path d="M3.5 7.5a2 2 0 0 1 2-2h4l2 2.2h7a2 2 0 0 1 2 2V17a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z" />
      </svg>
      <span class="min-w-0 flex-1 truncate">{{ current || '尚未開啟專案' }}</span>
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

    <div v-if="open" class="menu-panel absolute top-full left-0 z-40 mt-2 w-72 p-2">
      <div class="px-3 pt-1 pb-2 text-[12.5px] font-semibold text-mute">切換專案</div>
      <ul ref="listEl" class="max-h-80 overflow-y-auto overscroll-contain" role="listbox" aria-label="專案">
        <li v-for="(p, i) in projects" :key="p">
          <button
            type="button"
            role="option"
            :aria-selected="p === current"
            :data-active="i === active"
            class="flex w-full items-center gap-3 rounded-2xl px-3 py-3 text-left text-[15.5px] transition-colors"
            :class="[
              p === current ? 'font-semibold text-accent' : 'text-ink',
              i === active ? 'bg-accent-soft' : 'hover:bg-hover',
            ]"
            @mousemove="active = i"
            @click="choose(p)"
          >
            <span class="flex h-5 w-5 shrink-0 items-center justify-center" aria-hidden="true">
              <svg v-if="p === current" viewBox="0 0 24 24" class="h-[18px] w-[18px]" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round">
                <path d="M5 12.5l4.5 4.5L19 7.5" />
              </svg>
            </span>
            <span class="min-w-0 flex-1 truncate">{{ p }}</span>
          </button>
        </li>
      </ul>
    </div>
  </div>
</template>
