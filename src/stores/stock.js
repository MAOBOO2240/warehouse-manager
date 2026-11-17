import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { api } from '../api'
import { useProjectStore } from './project'
import { useProductStore } from './products'
import { useRemarkStore } from './remarks'
import { useRowOrderStore } from './rowOrder'

/** 存放地點的排列：依名稱（數字照大小），「未指定」（空白）排最後 */
export const locCompare = (a, b) => (a === '') - (b === '') || a.localeCompare(b, 'zh-Hant', { numeric: true })

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

  /** 被移除的存放地點 → 移除時間（config.json 的 removed_locations） */
  const removedAt = computed(() => useProjectStore().config?.removed_locations ?? {})

  /**
   * 曾使用過的存放地點（不含空白），供入庫時快速選擇。
   * 被移除過的地點：歷史紀錄仍然提到它，但要等「移除之後」又有異動（或現在有庫存）才會再出現。
   */
  const knownLocations = computed(() => {
    const removed = removedAt.value
    const set = new Set()
    for (const r of inventory.value) if (r.location_id) set.add(r.location_id)
    for (const l of logs.value) {
      const loc = l.location_id
      if (!loc || set.has(loc)) continue
      const when = removed[loc]
      if (!when || String(l.timestamp) > when) set.add(loc)
    }
    return [...set].sort(locCompare)
  })

  /** 這個專案有沒有用存放地點（都沒填過的專案，入出庫畫面就不顯示「未指定」這些字） */
  const usesLocations = computed(() => knownLocations.value.length > 0)

  /** 目前有庫存的存放地點：[{ loc, items（品項數）, qty（件數） }]，依名稱排序、「未指定」排最後 */
  const locationStats = computed(() => {
    const m = new Map()
    for (const r of inventory.value) {
      if (r.quantity <= 0) continue
      let s = m.get(r.location_id)
      if (!s) m.set(r.location_id, (s = { loc: r.location_id, items: 0, qty: 0 }))
      s.items += 1
      s.qty += r.quantity
    }
    return [...m.values()].sort((a, b) => locCompare(a.loc, b.loc))
  })

  /** 用過、但目前沒有任何庫存的存放地點（還會出現在建議清單裡，可以移除） */
  const emptyLocations = computed(() => {
    const live = new Set(locationStats.value.map((s) => s.loc))
    return knownLocations.value.filter((l) => !live.has(l))
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
    // 移除存放地點時，設定、註記與排列順序也跟著更新
    if (res.config) project.config = res.config
    if (res.remarks) useRemarkStore().items = res.remarks
    if (res.row_order) useRowOrderStore().items = res.row_order
    return res.new_logs
  }

  /**
   * 移除存放地點：這個地點的庫存全部移到 to（空白＝未指定），註記與排列順序跟著搬；
   * 之後這個地點不再出現在清單與建議裡（再用同一個名稱入庫就會重新出現）。
   */
  function removeLocation(loc, to = '', note = '') {
    return apply({ kind: 'REMOVE_LOCATION', location_id: loc, to_location_id: to, note })
  }

  return {
    inventory, logs, qty, total, totalByProduct, knownLocations, usesLocations, locationStats, emptyLocations,
    rowsOfProduct, apply, removeLocation,
  }
})
