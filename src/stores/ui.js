import { defineStore } from 'pinia'
import { ref, watch } from 'vue'

const SETTINGS_KEY = 'warehouse-ui-settings'
export const FONT_SCALES = { small: 0.9, medium: 1, large: 1.15 }

function loadSettings() {
  try {
    const raw = JSON.parse(localStorage.getItem(SETTINGS_KEY) || '{}')
    return {
      theme: raw.theme === 'light' ? 'light' : 'dark',
      fontSize: raw.fontSize in FONT_SCALES ? raw.fontSize : 'medium',
    }
  } catch {
    return { theme: 'dark', fontSize: 'medium' }
  }
}

function applySettings(s) {
  const root = document.documentElement
  root.dataset.theme = s.theme
  root.style.setProperty('--ui-scale', String(FONT_SCALES[s.fontSize]))
}

export const useUiStore = defineStore('ui', () => {
  const page = ref('dashboard')
  // 跳頁時帶過去的參數
  const params = ref({})
  const toasts = ref([])
  const settings = ref(loadSettings())
  let seq = 0

  applySettings(settings.value)
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

  return { page, params, toasts, settings, go, toast, ok, error }
})
