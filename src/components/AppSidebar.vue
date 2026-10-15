<script setup>
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useUiStore } from '../stores/ui'

const ui = useUiStore()

const groups = [
  { title: '總覽', items: [{ id: 'dashboard', label: '儀表板' }] },
  {
    title: '日常作業',
    items: [
      { id: 'inbound', label: '入庫' },
      { id: 'outbound', label: '出庫/損壞' },
    ],
  },
  {
    title: '查詢報表',
    items: [
      { id: 'inventory', label: '庫存清單' },
      { id: 'logs', label: '異動流水帳' },
    ],
  },
]

const sizes = [
  { id: 'large', label: '大' },
  { id: 'medium', label: '中' },
  { id: 'small', label: '小' },
]
const themes = [
  { id: 'dark', label: '深色' },
  { id: 'light', label: '淺色' },
]

const showSettings = ref(false)
const settingsEl = ref(null)

function onDocDown(e) {
  if (showSettings.value && settingsEl.value && !settingsEl.value.contains(e.target)) showSettings.value = false
}
function onKey(e) {
  if (e.key === 'Escape') showSettings.value = false
}
onMounted(() => {
  document.addEventListener('mousedown', onDocDown)
  window.addEventListener('keydown', onKey)
})
onBeforeUnmount(() => {
  document.removeEventListener('mousedown', onDocDown)
  window.removeEventListener('keydown', onKey)
})
</script>

<template>
  <nav class="flex w-56 shrink-0 flex-col bg-nav px-3 pt-5 pb-3">
    <div class="px-3 pb-4">
      <div class="text-[17px] font-bold tracking-wide text-ink">倉儲管理系統</div>
      <div class="mt-0.5 text-[12px] text-mute">單機離線版</div>
    </div>

    <div class="flex-1 overflow-auto">
      <section v-for="g in groups" :key="g.title" class="mt-4">
        <h3 class="px-3 pb-1.5 text-[12px] font-semibold text-mute">{{ g.title }}</h3>
        <div class="space-y-1">
          <button
            v-for="item in g.items"
            :key="item.id"
            type="button"
            class="flex w-full items-center rounded-full px-4 py-2.5 text-left text-[14.5px] transition-all"
            :class="ui.page === item.id ? 'bg-accent font-semibold text-on-accent' : 'text-ink-2 hover:bg-hover'"
            :style="ui.page === item.id ? 'box-shadow: var(--gloss), var(--bubble-shadow)' : ''"
            @click="ui.go(item.id)"
          >
            {{ item.label }}
          </button>
        </div>
      </section>
    </div>

    <!-- 左下角設定 -->
    <div ref="settingsEl" class="relative pt-3">
      <div
        v-if="showSettings"
        class="panel absolute bottom-full left-0 z-40 mb-2 w-64 p-4"
        role="dialog"
        aria-label="設定"
      >
        <div class="mb-2 text-[13px] font-semibold text-mute">文字大小</div>
        <div class="grid grid-cols-3 gap-2">
          <button
            v-for="s in sizes"
            :key="s.id"
            type="button"
            class="pill"
            :class="ui.settings.fontSize === s.id ? 'pill-on' : ''"
            :aria-pressed="ui.settings.fontSize === s.id"
            @click="ui.settings.fontSize = s.id"
          >{{ s.label }}</button>
        </div>
        <div class="mt-4 mb-2 text-[13px] font-semibold text-mute">外觀</div>
        <div class="grid grid-cols-2 gap-2">
          <button
            v-for="t in themes"
            :key="t.id"
            type="button"
            class="pill"
            :class="ui.settings.theme === t.id ? 'pill-on' : ''"
            :aria-pressed="ui.settings.theme === t.id"
            @click="ui.settings.theme = t.id"
          >{{ t.label }}</button>
        </div>
      </div>
      <button
        type="button"
        class="flex h-11 w-11 items-center justify-center rounded-full bg-raised text-ink-2 transition-colors hover:bg-hover hover:text-ink"
        style="box-shadow: var(--gloss), var(--bubble-shadow)"
        :aria-expanded="showSettings"
        aria-label="設定"
        title="設定"
        @click="showSettings = !showSettings"
      >
        <svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
        </svg>
      </button>
    </div>
  </nav>
</template>
