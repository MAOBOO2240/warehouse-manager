<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useUiStore } from '../stores/ui'
import { useStockStore } from '../stores/stock'
import { useProjectStore } from '../stores/project'
import { locLabel, fmtInt } from '../utils/format'
import { api } from '../api'
import { useUpdateStore } from '../stores/update'
import ModalDialog from './ModalDialog.vue'

const ui = useUiStore()
const stock = useStockStore()
const project = useProjectStore()
const update = useUpdateStore()

// 設定裡的「版本」：有新版時按鈕直接變成「更新到新版本」
function onUpdateClick() {
  if (update.result?.has_update) {
    showSettings.value = false
    update.show = true
  } else update.check(true)
}

// 自己在電腦上編譯的版本沒有對應的發行版本，不會自動更新（開關照樣可以切換，換成正式版就會生效）
const selfBuilt = computed(() => !!update.info && !update.info.can_update)

// 上次檢查更新的時間與結果：查不到新版本時直接寫出原因（例如私人倉庫沒有設定更新權杖），不會無聲無息
const lastCheck = computed(() => {
  const s = update.status
  if (!s) return null
  const d = s.at
  const pad = (n) => String(n).padStart(2, '0')
  const now = new Date()
  const sameDay = d.getFullYear() === now.getFullYear() && d.getMonth() === now.getMonth() && d.getDate() === now.getDate()
  const time = `${sameDay ? '' : `${d.getMonth() + 1}/${d.getDate()} `}${pad(d.getHours())}:${pad(d.getMinutes())}`
  if (!s.ok) return { failed: true, text: `${time} 檢查失敗：${s.message}` }
  if (s.hasUpdate) return { failed: false, text: `${time} 檢查：有新版本可以更新` }
  return { failed: false, text: s.canUpdate ? `${time} 檢查：已經是最新版本` : `${time} 檢查：自己編譯的版本無法比對新舊` }
})

/* ---- 庫存清單底下列出目前的存放地點（有庫存的），點選後只顯示該地點 ---- */
// stock.locationStats：[{ loc, items（品項數）, qty（件數） }]，已依名稱排好、「未指定」在最後
const locations = computed(() => stock.locationStats)
const allCount = computed(() => locations.value.reduce((t, l) => t + l.items, 0))
// 只有「未指定」一個地點時（沒有用到存放地點）不列出子選單
const showLocs = computed(() => locations.value.some((l) => l.loc))
// 滑鼠停在地點上：完整名稱（太長時畫面上會截短）與「幾項・幾件」
const locTitle = (l) => `${locLabel(l.loc)}：${fmtInt(l.items)} 項・${fmtInt(l.qty)} 件`

function openInventory(loc = null) {
  ui.invLoc = loc
  ui.invListOpen = true
  ui.go('inventory')
}
// 點「庫存清單」：從其他頁面進來時展開地點列表；已在庫存清單時再點一次則收起／展開
function clickInventory() {
  if (ui.page === 'inventory') ui.invListOpen = !ui.invListOpen
  else openInventory()
}
const subActive = (loc) => ui.page === 'inventory' && ui.invLoc === loc

// 換專案時回到「全部地點」；選取的地點已沒有庫存時也回到全部
watch(
  () => project.current,
  () => (ui.invLoc = null),
)
watch(locations, (list) => {
  if (ui.invLoc !== null && !list.some((l) => l.loc === ui.invLoc)) ui.invLoc = null
})

// 每個項目有自己的顏色：圖示平常就上色，選取時整顆膠囊填滿該顏色
const ICONS = {
  dashboard: 'M4 4h6.5v6.5H4zM13.5 4H20v6.5h-6.5zM4 13.5h6.5V20H4zM13.5 13.5H20V20h-6.5z',
  inbound: 'M12 3.5v10M7.5 9 12 13.5 16.5 9M4 14.5v3a2.5 2.5 0 0 0 2.5 2.5h11a2.5 2.5 0 0 0 2.5-2.5v-3',
  outbound: 'M12 13.5v-10M7.5 8 12 3.5 16.5 8M4 14.5v3a2.5 2.5 0 0 0 2.5 2.5h11a2.5 2.5 0 0 0 2.5-2.5v-3',
  inventory: 'M12 3 3.5 7.5 12 12l8.5-4.5zM3.5 12 12 16.5l8.5-4.5M3.5 16.5 12 21l8.5-4.5',
  logs: 'M8.5 6.5h11M8.5 12h11M8.5 17.5h11M4.5 6.5h.01M4.5 12h.01M4.5 17.5h.01',
}
const groups = [
  {
    title: '總覽',
    items: [{ id: 'dashboard', label: '主頁面', tone: 'slate' }],
  },
  {
    title: '日常作業',
    items: [
      { id: 'inbound', label: '入庫', tone: 'go' },
      { id: 'outbound', label: '出庫/損壞', tone: 'accent' },
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
const toneStyle = (tone) => ({
  '--tone': `var(--color-${tone})`,
  '--tone-fg': `var(--color-on-${tone}, #fff)`,
})

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

function openClearConfirm() {
  confirmClear.value = true
  showSettings.value = false
}

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
    </div>

    <div class="flex-1 overflow-auto">
      <section v-for="g in groups" :key="g.title" class="mt-4">
        <h3 class="px-3 pb-1.5 text-[12px] font-semibold text-mute">
          {{ g.title }}
        </h3>
        <div class="space-y-1">
          <template v-for="item in g.items" :key="item.id">
            <button
              type="button"
              class="nav-item"
              :class="ui.page === item.id ? 'is-active' : ''"
              :style="toneStyle(item.tone)"
              :aria-current="ui.page === item.id ? 'page' : undefined"
              @click="item.id === 'inventory' ? clickInventory() : ui.go(item.id)"
            >
              <span class="nav-icon" aria-hidden="true">
                <svg
                  viewBox="0 0 24 24"
                  class="h-[17px] w-[17px]"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2.1"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                >
                  <path :d="ICONS[item.id]" />
                </svg>
              </span>
              {{ item.label }}
              <svg
                v-if="item.id === 'inventory' && ui.page === 'inventory' && showLocs"
                viewBox="0 0 24 24"
                class="ml-auto h-4 w-4 shrink-0 opacity-80 transition-transform"
                :class="ui.invListOpen ? '' : '-rotate-90'"
                fill="none"
                stroke="currentColor"
                stroke-width="2.6"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
              >
                <path d="M6 9l6 6 6-6" />
              </svg>
            </button>
            <!-- 庫存清單：存放地點子選單 -->
            <div v-if="item.id === 'inventory' && ui.page === 'inventory' && ui.invListOpen && showLocs" class="nav-sub max-h-[42vh] overflow-y-auto overscroll-contain" :style="toneStyle(item.tone)" role="group" aria-label="存放地點">
              <button
                type="button"
                class="nav-sub-item"
                :class="subActive(null) ? 'is-active' : ''"
                :aria-current="subActive(null) ? 'page' : undefined"
                @click="openInventory(null)"
              >
                <svg viewBox="0 0 24 24" class="h-[14px] w-[14px] shrink-0 opacity-80" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 3 3.5 7.5 12 12l8.5-4.5zM3.5 12 12 16.5l8.5-4.5M3.5 16.5 12 21l8.5-4.5" /></svg>
                <span class="min-w-0 flex-1 truncate">全部地點</span>
                <span class="num nav-sub-count">{{ fmtInt(allCount) }}</span>
              </button>
              <button
                v-for="l in locations"
                :key="l.loc"
                type="button"
                class="nav-sub-item"
                :class="subActive(l.loc) ? 'is-active' : ''"
                :aria-current="subActive(l.loc) ? 'page' : undefined"
                :title="locTitle(l)"
                @click="openInventory(l.loc)"
              >
                <svg
                  viewBox="0 0 24 24"
                  class="h-[14px] w-[14px] shrink-0 opacity-80"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2.2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  aria-hidden="true"
                >
                  <path d="M12 21s-6.5-5.6-6.5-11a6.5 6.5 0 0 1 13 0c0 5.4-6.5 11-6.5 11z" />
                  <circle cx="12" cy="10" r="2.3" />
                </svg>
                <span class="min-w-0 flex-1 truncate" :class="l.loc ? '' : 'italic'">{{ locLabel(l.loc) }}</span>
                <span class="num nav-sub-count">{{ fmtInt(l.items) }}</span>
              </button>
            </div>
          </template>
        </div>
      </section>
    </div>

    <!-- 左下角設定 -->
    <div ref="settingsEl" class="relative pt-3">
      <div v-if="showSettings" class="menu-panel absolute bottom-full left-0 z-40 mb-2 w-72 p-4" role="dialog" aria-label="設定">
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
          >
            {{ s.label }}
          </button>
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
          >
            {{ t.label }}
          </button>
        </div>
        <div class="mt-4 mb-2 text-[13px] font-semibold text-mute">維護</div>
        <button
          type="button"
          class="pill w-full gap-2"
          @click="openClearConfirm"
        >
          <svg
            viewBox="0 0 24 24"
            class="h-4 w-4 text-safety"
            fill="none"
            stroke="currentColor"
            stroke-width="2.2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d="M4 7h16M10 11v6M14 11v6M5.5 7l1 12a2 2 0 0 0 2 1.8h7a2 2 0 0 0 2-1.8l1-12M9 7V4.5h6V7" />
          </svg>
          清除緩存
        </button>
        <div class="mt-4 mb-2 text-[13px] font-semibold text-mute">版本</div>
        <div class="flex items-center gap-2 pl-1">
          <span class="min-w-0 flex-1 truncate text-[13px] text-ink-2">
            <span class="num">{{ update.info?.version || project.version }}</span><span v-if="update.info?.build">・編譯 #<span class="num">{{ update.info.build }}</span></span>
          </span>
          <button type="button" class="btn-ghost btn-sm" :class="update.result?.has_update ? 'text-accent' : ''" :disabled="update.checking || update.installing" @click="onUpdateClick">
            {{ update.checking ? '檢查中…' : update.result?.has_update ? '更新到新版本' : '檢查更新' }}
          </button>
        </div>
        <!-- 上次檢查的結果；失敗時寫出原因（可以選取複製） -->
        <p
          v-if="lastCheck"
          class="selectable mt-1.5 pl-1 text-[12px] leading-snug wrap-anywhere"
          :class="lastCheck.failed ? 'text-signal' : 'text-mute'"
          :role="lastCheck.failed ? 'alert' : undefined"
        >{{ lastCheck.text }}</p>
        <!-- 自動更新：整列都可以點，右邊的開關顯示目前狀態 -->
        <button
          type="button"
          role="switch"
          class="-mx-2 mt-2 flex w-[calc(100%+1rem)] items-center gap-3 rounded-2xl px-3 py-2 text-left transition-colors hover:bg-hover"
          :aria-checked="ui.settings.autoUpdate"
          @click="ui.settings.autoUpdate = !ui.settings.autoUpdate"
        >
          <span class="min-w-0 flex-1">
            <span class="block text-[13.5px] font-semibold text-ink">自動更新</span>
            <span class="mt-0.5 block text-[12px] leading-snug text-mute">{{ selfBuilt ? '這個版本是自己編譯的，不會自動更新' : '開啟程式時有新版本就直接安裝' }}</span>
          </span>
          <span class="switch" :class="{ 'is-on': ui.settings.autoUpdate }" aria-hidden="true" />
        </button>
      </div>
      <button
        type="button"
        class="relative flex h-11 w-11 items-center justify-center rounded-full bg-raised text-ink-2 transition-colors hover:bg-hover hover:text-ink"
        style="box-shadow: var(--gloss), var(--bubble-shadow)"
        :aria-expanded="showSettings"
        :aria-label="update.result?.has_update ? '設定（有新版本）' : '設定'"
        :title="update.result?.has_update ? '設定（有新版本可以更新）' : '設定'"
        @click="showSettings = !showSettings"
      >
        <span v-if="update.result?.has_update" class="absolute top-0.5 right-0.5 h-3 w-3 rounded-full bg-accent ring-2 ring-nav" aria-hidden="true" />
        <svg
          viewBox="0 0 24 24"
          class="h-5 w-5"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <circle cx="12" cy="12" r="3" />
          <path
            d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
          />
        </svg>
      </button>
    </div>
  </nav>

  <ModalDialog v-if="confirmClear" title="清除緩存" subtitle="不會刪除任何資料" tone="safety" :closable="!clearing" @close="confirmClear = false">
    <template #icon>
      <svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M4 7h16M10 11v6M14 11v6M5.5 7l1 12a2 2 0 0 0 2 1.8h7a2 2 0 0 0 2-1.8l1-12M9 7V4.5h6V7" />
      </svg>
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
      <button type="button" class="btn-primary" :disabled="clearing" @click="clearCache">
        {{ clearing ? '清除中…' : '確認清除' }}
      </button>
    </template>
  </ModalDialog>
</template>
