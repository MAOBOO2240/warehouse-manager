<script setup>
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useUiStore } from '../stores/ui'
import { api } from '../api'
import ModalDialog from './ModalDialog.vue'

const ui = useUiStore()

// 每個項目有自己的顏色：圖示平常就上色，選取時整顆膠囊填滿該顏色
const ICONS = {
  dashboard: 'M4 4h6.5v6.5H4zM13.5 4H20v6.5h-6.5zM4 13.5h6.5V20H4zM13.5 13.5H20V20h-6.5z',
  inbound: 'M12 3.5v10M7.5 9 12 13.5 16.5 9M4 14.5v3a2.5 2.5 0 0 0 2.5 2.5h11a2.5 2.5 0 0 0 2.5-2.5v-3',
  outbound: 'M12 13.5v-10M7.5 8 12 3.5 16.5 8M4 14.5v3a2.5 2.5 0 0 0 2.5 2.5h11a2.5 2.5 0 0 0 2.5-2.5v-3',
  inventory: 'M12 3 3.5 7.5 12 12l8.5-4.5zM3.5 12 12 16.5l8.5-4.5M3.5 16.5 12 21l8.5-4.5',
  logs: 'M8.5 6.5h11M8.5 12h11M8.5 17.5h11M4.5 6.5h.01M4.5 12h.01M4.5 17.5h.01',
}
const groups = [
  { title: '總覽', items: [{ id: 'dashboard', label: '主頁面', tone: 'accent' }] },
  {
    title: '日常作業',
    items: [
      { id: 'inbound', label: '入庫', tone: 'go' },
      { id: 'outbound', label: '出庫/損壞', tone: 'signal' },
    ],
  },
  {
    title: '查詢報表',
    items: [
      { id: 'inventory', label: '庫存清單', tone: 'violet' },
      { id: 'logs', label: '歷史紀錄', tone: 'amber' },
    ],
  },
]
const toneStyle = (tone) => ({ '--tone': `var(--color-${tone})`, '--tone-fg': `var(--color-on-${tone}, #fff)` })

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
const confirmClear = ref(false)
const clearing = ref(false)

async function clearCache() {
  clearing.value = true
  try {
    const removed = await api.clearCache()
    ui.reloadAfterClear(removed)
  } catch (err) {
    ui.error(err)
    clearing.value = false
    confirmClear.value = false
  }
}
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
            class="nav-item"
            :class="ui.page === item.id ? 'is-active' : ''"
            :style="toneStyle(item.tone)"
            :aria-current="ui.page === item.id ? 'page' : undefined"
            @click="ui.go(item.id)"
          >
            <span class="nav-icon" aria-hidden="true">
              <svg viewBox="0 0 24 24" class="h-[17px] w-[17px]" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round">
                <path :d="ICONS[item.id]" />
              </svg>
            </span>
            {{ item.label }}
          </button>
        </div>
      </section>
    </div>

    <!-- 左下角設定 -->
    <div ref="settingsEl" class="relative pt-3">
      <div
        v-if="showSettings"
        class="menu-panel absolute bottom-full left-0 z-40 mb-2 w-64 p-4"
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
        <div class="mt-4 mb-2 text-[13px] font-semibold text-mute">維護</div>
        <button
          type="button"
          class="pill w-full gap-2"
          @click="confirmClear = true; showSettings = false"
        >
          <svg viewBox="0 0 24 24" class="h-4 w-4 text-safety" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 7h16M10 11v6M14 11v6M5.5 7l1 12a2 2 0 0 0 2 1.8h7a2 2 0 0 0 2-1.8l1-12M9 7V4.5h6V7" /></svg>
          清除緩存
        </button>
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

  <ModalDialog v-if="confirmClear" title="清除緩存" subtitle="不會刪除任何資料" tone="safety" :closable="!clearing" @close="confirmClear = false">
    <template #icon>
      <svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7h16M10 11v6M14 11v6M5.5 7l1 12a2 2 0 0 0 2 1.8h7a2 2 0 0 0 2-1.8l1-12M9 7V4.5h6V7" /></svg>
    </template>
    <div class="tone-card text-[14.5px] leading-relaxed">
      <p>將執行以下動作：</p>
      <ul class="mt-2 list-disc space-y-1 pl-5 text-[13.5px] text-ink-2">
        <li>刪除 projects 資料夾內殘留的暫存檔（.tmp）</li>
        <li>清除程式畫面的瀏覽快取</li>
        <li>重新整理畫面，所有資料重新從檔案讀取</li>
      </ul>
      <p class="mt-3 text-[13px] text-ink-2">品項、庫存、歷史紀錄、註記與 .bak 備份都不會受影響；文字大小與外觀設定會保留。</p>
    </div>
    <template #footer>
      <button type="button" class="btn-ghost" :disabled="clearing" @click="confirmClear = false">取消</button>
      <button type="button" class="btn-primary" :disabled="clearing" @click="clearCache">{{ clearing ? '清除中…' : '確認清除' }}</button>
    </template>
  </ModalDialog>
</template>
