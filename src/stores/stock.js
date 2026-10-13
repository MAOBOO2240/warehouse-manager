import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { api } from '../api'
import { useProjectStore } from './project'
import { useProductStore } from './products'

/** 即時庫存與異動流水帳。所有數量變動都經由 Rust 端交易 API，前端不直接寫檔。 */
export const useStockStore = defineStore('stock', () => {
  const inventory = ref([])
  const logs = ref([])

  const key = (pid, loc) => `${pid}\u0000${loc}`
  const qtyMap = computed(() => new Map(inventory.value.map((r) => [key(r.product_id, r.location_id), r.quantity])))

  const totalByProduct = computed(() => {
    const m = new Map()
    for (const r of inventory.value) m.set(r.product_id, (m.get(r.product_id) || 0) + r.quantity)
    return m
  })

  /** 曾使用過的存放地點（不含空白），供入庫時快速選擇 */
  const knownLocations = computed(() => {
    const set = new Set()
    for (const r of inventory.value) if (r.location_id) set.add(r.location_id)
    for (const l of logs.value) if (l.location_id) set.add(l.location_id)
    return [...set].sort((a, b) => a.localeCompare(b, 'zh-Hant', { numeric: true }))
  })

  function qty(pid, loc) {
    return qtyMap.value.get(key(pid, loc ?? '')) || 0
  }

  function total(pid) {
    return totalByProduct.value.get(pid) || 0
  }

  /** 某品項有庫存的存放地點（數量由多到少） */
  function rowsOfProduct(pid) {
    return inventory.value.filter((r) => r.product_id === pid && r.quantity > 0).sort((a, b) => b.quantity - a.quantity)
  }

  /** 送出交易；成功後以後端回傳的資料取代前端狀態 */
  async function apply(tx) {
    const project = useProjectStore()
    project.assertWritable()
    const res = await api.applyTransaction(project.current, tx)
    inventory.value = res.inventory
    logs.value = [...logs.value, ...res.new_logs]
    if (res.products) useProductStore().items = res.products
    return res.new_logs
  }

  return { inventory, logs, qty, total, totalByProduct, knownLocations, rowsOfProduct, apply }
})
