import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { api, ApiError } from '../api'
import { useProjectStore } from './project'

const norm = (s) => String(s ?? '').trim().toLowerCase()

/**
 * 品項清單。品項不需預先建檔：入庫時輸入新名稱即自動建立。
 * first_qty 為該品項第一次入庫的數量，之後出入庫選擇此品項時自動帶入。
 */
export const useProductStore = defineStore('products', () => {
  const items = ref([])

  const byId = computed(() => new Map(items.value.map((p) => [p.id, p])))
  const sorted = computed(() => [...items.value].sort((a, b) => a.name.localeCompare(b.name, 'zh-Hant')))

  function get(id) {
    return byId.value.get(id)
  }

  /** 名稱完全相符（不分大小寫、忽略前後空白） */
  function findByName(name) {
    const k = norm(name)
    if (!k) return null
    return items.value.find((p) => norm(p.name) === k) || null
  }

  /** 名稱關鍵字搜尋 */
  function search(q) {
    const k = norm(q)
    if (!k) return sorted.value
    return sorted.value.filter((p) => norm(p.name).includes(k))
  }

  function nextId() {
    const max = items.value.reduce((m, p) => {
      const n = /^PROD-(\d+)$/.exec(p.id)
      return n ? Math.max(m, Number(n[1])) : m
    }, 0)
    return `PROD-${String(max + 1).padStart(3, '0')}`
  }

  /** 新增品項並存檔，回傳新品項；同名品項已存在則直接回傳既有品項 */
  async function create(name) {
    const clean = String(name ?? '').trim()
    if (!clean) throw new ApiError('INVALID_INPUT', '請輸入品項名稱')
    if (clean.length > 100) throw new ApiError('INVALID_INPUT', '品項名稱請在 100 個字以內')
    const existing = findByName(clean)
    if (existing) return existing
    const project = useProjectStore()
    project.assertWritable()
    const p = {
      id: nextId(),
      name: clean,
      first_qty: 0,
      created_at: new Date().toISOString().replace(/\.\d{3}Z$/, 'Z'),
    }
    const list = [...items.value, p]
    await api.writeDataAtomic(project.current, 'products', list)
    items.value = list
    return p
  }

  return { items, sorted, byId, get, findByName, search, create }
})
