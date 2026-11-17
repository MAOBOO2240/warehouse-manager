import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import { api } from '../api'

const SETTINGS_KEY = 'warehouse-ui-settings'
export const FONT_SCALES = { small: 0.9, medium: 1, large: 1.15 }

/** 「清除緩存」後重新整理時，設定與結果經由網址 hash 帶回（localStorage 可能已被清空） */
function readCacheHash() {
  try {
    const p = new URLSearchParams(location.hash.slice(1))
    if (!p.has('cleared')) return null
    return { cleared: Number(p.get('cleared')) || 0, settings: JSON.parse(p.get('s') || '{}') }
  } catch {
    return null
  }
}
const cacheHash = readCacheHash()

function loadSettings() {
  try {
    const raw = cacheHash?.settings ?? JSON.parse(localStorage.getItem(SETTINGS_KEY) || '{}')
    return {
      theme: raw.theme === 'light' ? 'light' : 'dark',
      fontSize: raw.fontSize in FONT_SCALES ? raw.fontSize : 'medium',
      // 開啟程式時發現新版本就自動更新；關掉的話改成先詢問
      autoUpdate: raw.autoUpdate !== false,
      // 庫存清單的顯示方式：loc＝依存放地點分開；item＝每個品項一列（不分地點的總數）
      invView: raw.invView === 'item' ? 'item' : 'loc',
    }
  } catch {
    return { theme: 'dark', fontSize: 'medium', autoUpdate: true, invView: 'loc' }
  }
}

let windowTheme = ''
function applySettings(s) {
  const root = document.documentElement
  root.dataset.theme = s.theme
  root.style.setProperty('--ui-scale', String(FONT_SCALES[s.fontSize]))
  // 視窗標題列跟著換成深色／淺色
  if (windowTheme !== s.theme) {
    windowTheme = s.theme
    api.setWindowTheme(s.theme)
  }
}

export const useUiStore = defineStore('ui', () => {
  const page = ref('dashboard')
  // 跳頁時帶過去的參數
  const params = ref({})
  // 庫存清單目前顯示的存放地點：null = 全部地點，'' = 未指定
  const invLoc = ref(null)
  // 左側選單「庫存清單」底下的地點列表是否展開
  const invListOpen = ref(true)
  const toasts = ref([])
  const settings = ref(loadSettings())
  let seq = 0

  applySettings(settings.value)
  if (cacheHash) {
    try {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings.value))
    } catch {
      /* 忽略 */
    }
    history.replaceState(null, '', location.pathname + location.search)
    setTimeout(() => toast(`緩存已清除${cacheHash.cleared ? `（移除 ${cacheHash.cleared} 個暫存檔）` : ''}，資料已重新從檔案載入`, 'ok', 4200), 300)
  }

  /** 清除緩存後重新整理畫面，保留目前的文字大小與外觀設定 */
  function reloadAfterClear(cleared) {
    const p = new URLSearchParams({ cleared: String(cleared), s: JSON.stringify(settings.value) })
    location.replace(location.pathname + location.search + '#' + p.toString())
    location.reload()
  }

  watch(
    settings,
    (s) => {
      applySettings(s)
      try {
        localStorage.setItem(SETTINGS_KEY, JSON.stringify(s))
      } catch {
        /* 儲存失敗時仍以目前設定運作 */
      }
    },
    { deep: true },
  )

  function go(name, p = {}) {
    params.value = p
    page.value = name
  }

  function toast(text, type = 'ok', ms = 3200) {
    const id = ++seq
    toasts.value.push({ id, text, type })
    setTimeout(() => {
      toasts.value = toasts.value.filter((t) => t.id !== id)
    }, ms)
  }

  const ok = (text) => toast(text, 'ok')
  const error = (err) => toast(typeof err === 'string' ? err : err?.message || String(err), 'error', 6000)

  return { page, params, invLoc, invListOpen, toasts, settings, go, toast, ok, error, reloadAfterClear }
})
