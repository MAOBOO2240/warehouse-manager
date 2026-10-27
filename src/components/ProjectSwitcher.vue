<script setup>
// 專案選單：自訂彈出清單（取代系統下拉選單），目前專案打勾；下方是專案的管理動作（新增、匯入／匯出備份、刪除）。
// 可用方向鍵與 Enter 操作
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'

const props = defineProps({
  projects: { type: Array, required: true },
  current: { type: String, default: '' },
  disabled: { type: Boolean, default: false },
  /** 正在匯入／匯出時，管理動作先停用 */
  busy: { type: Boolean, default: false },
})
const emit = defineEmits(['select', 'action'])

const open = ref(false)
const active = ref(0)
const root = ref(null)
const listEl = ref(null)
const btnEl = ref(null)

const ICONS = {
  new: 'M12 5v14M5 12h14',
  import: 'M12 4v10M7.5 9.5 12 14l4.5-4.5M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3',
  export: 'M12 14V4M7.5 8.5 12 4l4.5 4.5M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3',
  delete: 'M4 7h16M10 11v6M14 11v6M5.5 7l1 12a2 2 0 0 0 2 1.8h7a2 2 0 0 0 2-1.8l1-12M9 7V4.5h6V7',
}

/** 清單裡的每一列：先是專案，再來是管理動作（方向鍵依序移動） */
const entries = computed(() => [
  ...props.projects.map((name) => ({ kind: 'project', key: `p:${name}`, name })),
  { kind: 'action', key: 'a:new', id: 'new', label: '新增專案' },
  { kind: 'action', key: 'a:import', id: 'import', label: '匯入備份（ZIP）', hint: '選擇先前匯出的備份，匯入成一個新專案' },
  { kind: 'action', key: 'a:export', id: 'export', label: '匯出這個專案的備份（ZIP）', off: !props.current },
  { kind: 'action', key: 'a:delete', id: 'delete', label: props.current ? `刪除「${props.current}」` : '刪除專案', danger: true, off: !props.current },
])
const projectEntries = computed(() => entries.value.filter((e) => e.kind === 'project'))
const actionEntries = computed(() => entries.value.filter((e) => e.kind === 'action'))
const isOff = (e) => e.kind === 'action' && (e.off || props.busy)

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

function choose(e) {
  if (!e || isOff(e)) return
  close(true)
  if (e.kind === 'project') {
    if (e.name !== props.current) emit('select', e.name)
  } else emit('action', e.id)
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
  const list = entries.value
  const n = list.length
  if (e.key === 'Escape') {
    e.preventDefault()
    close(true)
  } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault()
    if (!n) return
    const step = e.key === 'ArrowDown' ? 1 : -1
    let i = active.value
    for (let k = 0; k < n; k++) {
      i = (i + step + n) % n
      if (!isOff(list[i])) break
    }
    active.value = i
    await nextTick()
    scrollActive()
  } else if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault()
    if (n) choose(list[active.value])
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
      aria-haspopup="menu"
      :aria-expanded="open"
      title="切換、新增專案，匯入或匯出備份"
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

    <div v-if="open" ref="listEl" class="menu-panel absolute top-full left-0 z-40 mt-2 w-80 p-2" role="menu" aria-label="專案">
      <div class="px-3 pt-1 pb-2 text-[12.5px] font-semibold text-mute">切換專案</div>
      <ul class="max-h-72 overflow-y-auto overscroll-contain">
        <li v-for="e in projectEntries" :key="e.key">
          <button
            type="button"
            role="menuitemradio"
            :aria-checked="e.name === current"
            :data-active="entries.indexOf(e) === active"
            class="flex w-full items-center gap-3 rounded-2xl px-3 py-2.5 text-left text-[15.5px] transition-colors"
            :class="[
              e.name === current ? 'font-semibold text-accent' : 'text-ink',
              entries.indexOf(e) === active ? 'bg-accent-soft' : 'hover:bg-hover',
            ]"
            @mousemove="active = entries.indexOf(e)"
            @click="choose(e)"
          >
            <span class="flex h-5 w-5 shrink-0 items-center justify-center" aria-hidden="true">
              <svg v-if="e.name === current" viewBox="0 0 24 24" class="h-[18px] w-[18px]" fill="none" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round">
                <path d="M5 12.5l4.5 4.5L19 7.5" />
              </svg>
            </span>
            <span class="min-w-0 flex-1 truncate">{{ e.name }}</span>
          </button>
        </li>
      </ul>
      <div class="mx-2 my-1.5 border-t border-line" role="separator" />
      <ul>
        <li v-for="e in actionEntries" :key="e.key">
          <div v-if="e.danger" class="mx-2 my-1.5 border-t border-line" role="separator" />
          <button
            type="button"
            role="menuitem"
            :data-active="entries.indexOf(e) === active"
            :disabled="isOff(e)"
            :title="e.hint || ''"
            class="flex w-full items-center gap-3 rounded-2xl px-3 py-2.5 text-left text-[14.5px] transition-colors disabled:cursor-not-allowed disabled:opacity-40"
            :class="[
              e.danger ? 'text-signal' : 'text-ink',
              entries.indexOf(e) === active && !isOff(e) ? (e.danger ? 'bg-signal-soft' : 'bg-accent-soft') : 'hover:bg-hover',
            ]"
            @mousemove="!isOff(e) && (active = entries.indexOf(e))"
            @click="choose(e)"
          >
            <span class="flex h-5 w-5 shrink-0 items-center justify-center" aria-hidden="true">
              <svg viewBox="0 0 24 24" class="h-[17px] w-[17px]" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                <path :d="ICONS[e.id]" />
              </svg>
            </span>
            <span class="min-w-0 flex-1 truncate">{{ e.label }}</span>
          </button>
        </li>
      </ul>
    </div>
  </div>
</template>
