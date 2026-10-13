import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useUiStore = defineStore('ui', () => {
  const page = ref('dashboard')
  // 跳頁時帶過去的參數，例如從低庫存卡片跳到入庫時預選的商品
  const params = ref({})
  const toasts = ref([])
  let seq = 0

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

  return { page, params, toasts, go, toast, ok, error }
})
