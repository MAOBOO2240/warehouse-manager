import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { api } from '../api'
import { useProjectStore } from './project'
import { useProductStore } from './products'

/** 即時庫存與異動流水帳。所有數量變動都經由 Rust 端交易 API，前端不直接寫檔。 */
export const useStockStore = defineStore('stock', () => {
  const inventory = ref([])
  const logs = ref([])

  const key = (pid, lid) => `${pid}\u0000${lid}`
  const qtyMap = computed(() => new Map(inventory.value.map((r) => [key(r.product_id, r.location_id), r.quantity])))

  const totalByProduct = computed(() => {
    const m = new Map()
    for (const r of inventory.value) m.set(r.product_id, (m.get(r.product_id) || 0) + r.quantity)
    return m
  })

  const totalByLocation = computed(() => {
    const m = new Map()
    for (const r of inventory.value) m.set(r.location_id, (m.get(r.location_id) || 0) + r.quantity)
    return m
  })

  function qty(pid, lid) {
    return qtyMap.value.get(key(pid, lid)) || 0
  }

  function total(pid) {
    return totalByProduct.value.get(pid) || 0
  }

  /** 某商品有庫存的庫位清單（數量由多到少） */
  function rowsOfProduct(pid) {
    return inventory.value.filter((r) => r.product_id === pid && r.quantity > 0).sort((a, b) => b.quantity - a.quantity)
  }

  function rowsOfLocation(lid) {
    return inventory.value.filter((r) => r.location_id === lid && r.quantity > 0)
  }

  /** 低庫存：有設定最低安全庫存（> 0），且全庫總量 ≤ 最低安全庫存 */
  const lowStock = computed(() => {
    const products = useProductStore()
    return products.items
      .filter((p) => Number(p.min_stock) > 0 && total(p.id) <= Number(p.min_stock))
      .map((p) => ({ product: p, total: total(p.id) }))
      .sort((a, b) => a.total / a.product.min_stock - b.total / b.product.min_stock)
  })

  /** 送出交易；成功後以後端回傳的庫存全貌取代前端狀態 */
  async function apply(tx) {
    const project = useProjectStore()
    project.assertWritable()
    const res = await api.applyTransaction(project.current, tx)
    inventory.value = res.inventory
    logs.value = [...logs.value, ...res.new_logs]
    return res.new_logs
  }

  return { inventory, logs, qty, total, totalByProduct, totalByLocation, rowsOfProduct, rowsOfLocation, lowStock, apply }
})
