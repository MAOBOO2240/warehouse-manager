import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { api } from '../api'
import { useProjectStore } from './project'

export const rowKey = (pid, loc) => `${pid}\u0000${loc ?? ''}`

/**
 * 庫存清單的自訂排列順序（拖曳排序），存在 row_order.json。
 * 陣列先後即顯示先後；沒有記錄的列排在最後，依品項名稱排序。
 */
export const useRowOrderStore = defineStore('rowOrder', () => {
  const items = ref([])
  const rank = computed(() => new Map(items.value.map((r, i) => [rowKey(r.product_id, r.location_id), i])))
  const hasCustom = computed(() => items.value.length > 0)

  /** 依目前順序排列；fallback 為沒有記錄時的比較函式 */
  function sort(rows, fallback) {
    const r = rank.value
    return [...rows].sort((a, b) => {
      const ia = r.get(rowKey(a.product_id, a.location_id))
      const ib = r.get(rowKey(b.product_id, b.location_id))
      if (ia !== undefined && ib !== undefined) return ia - ib
      if (ia !== undefined) return -1
      if (ib !== undefined) return 1
      return fallback(a, b)
    })
  }

  /** 儲存新的完整順序：rows 為 [{ product_id, location_id }] */
  async function save(rows) {
    const project = useProjectStore()
    project.assertWritable()
    const list = rows.map((r) => ({ product_id: r.product_id, location_id: r.location_id ?? '' }))
    const prev = items.value
    items.value = list // 先更新畫面，失敗再還原
    try {
      await api.writeDataAtomic(project.current, 'row_order', list)
    } catch (err) {
      items.value = prev
      throw err
    }
  }

  async function reset() {
    const project = useProjectStore()
    project.assertWritable()
    await api.writeDataAtomic(project.current, 'row_order', [])
    items.value = []
  }

  return { items, hasCustom, sort, save, reset }
})
