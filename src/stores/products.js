import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { api, ApiError } from '../api'
import { useProjectStore } from './project'

export const EMPTY_PRODUCT = () => ({
  id: '',
  sku: '',
  barcode: '',
  name: '',
  category: '',
  spec: '',
  unit: '',
  cost_price: 0,
  sale_price: 0,
  min_stock: 0,
  max_stock: 0,
  created_at: '',
})

const norm = (s) => String(s ?? '').trim().toLowerCase()

export const useProductStore = defineStore('products', () => {
  const items = ref([])

  const byId = computed(() => new Map(items.value.map((p) => [p.id, p])))
  const categories = computed(() =>
    [...new Set(items.value.map((p) => p.category).filter(Boolean))].sort((a, b) => a.localeCompare(b, 'zh-Hant')),
  )

  function get(id) {
    return byId.value.get(id)
  }

  /** 掃描槍 / 手動輸入：條碼、SKU、編號完全相符才算命中 */
  function findExact(code) {
    const c = norm(code)
    if (!c) return null
    return (
      items.value.find((p) => norm(p.barcode) === c) ||
      items.value.find((p) => norm(p.sku) === c) ||
      items.value.find((p) => norm(p.id) === c) ||
      null
    )
  }

  /** 模糊搜尋：名稱、條碼、SKU、分類 */
  function search(q, category = '') {
    const k = norm(q)
    return items.value.filter((p) => {
      if (category && p.category !== category) return false
      if (!k) return true
      return [p.name, p.barcode, p.sku, p.id, p.category, p.spec].some((v) => norm(v).includes(k))
    })
  }

  function nextId() {
    const max = items.value.reduce((m, p) => {
      const n = /^PROD-(\d+)$/.exec(p.id)
      return n ? Math.max(m, Number(n[1])) : m
    }, 0)
    return `PROD-${String(max + 1).padStart(3, '0')}`
  }

  function validate(p, list) {
    if (!p.name.trim()) throw new ApiError('INVALID_INPUT', '請輸入商品名稱')
    for (const other of list) {
      if (other.id === p.id) continue
      if (p.barcode && norm(other.barcode) === norm(p.barcode))
        throw new ApiError('INVALID_INPUT', `條碼 ${p.barcode} 已被「${other.name}」使用`)
      if (p.sku && norm(other.sku) === norm(p.sku))
        throw new ApiError('INVALID_INPUT', `SKU ${p.sku} 已被「${other.name}」使用`)
    }
    for (const key of ['cost_price', 'sale_price', 'min_stock', 'max_stock']) {
      if (!(Number(p[key]) >= 0)) throw new ApiError('INVALID_INPUT', '價格與庫存水位不可為負數')
    }
    if (Number(p.max_stock) > 0 && Number(p.min_stock) > Number(p.max_stock))
      throw new ApiError('INVALID_INPUT', '最低安全庫存不可大於最高庫存')
  }

  async function persist(list) {
    const project = useProjectStore()
    project.assertWritable()
    await api.writeDataAtomic(project.current, 'products', list)
    items.value = list
  }

  /** 新增或更新商品，回傳儲存後的商品 */
  async function save(input) {
    const p = {
      ...EMPTY_PRODUCT(),
      ...input,
      sku: String(input.sku ?? '').trim(),
      barcode: String(input.barcode ?? '').trim(),
      name: String(input.name ?? '').trim(),
      category: String(input.category ?? '').trim(),
      spec: String(input.spec ?? '').trim(),
      unit: String(input.unit ?? '').trim(),
      cost_price: Number(input.cost_price) || 0,
      sale_price: Number(input.sale_price) || 0,
      min_stock: Math.trunc(Number(input.min_stock) || 0),
      max_stock: Math.trunc(Number(input.max_stock) || 0),
    }
    const isNew = !p.id
    if (isNew) {
      p.id = nextId()
      p.created_at = new Date().toISOString().replace(/\.\d{3}Z$/, 'Z')
    }
    validate(p, items.value)
    const list = isNew ? [...items.value, p] : items.value.map((x) => (x.id === p.id ? p : x))
    await persist(list)
    return p
  }

  async function remove(id) {
    await persist(items.value.filter((p) => p.id !== id))
  }

  return { items, byId, categories, get, findExact, search, nextId, save, remove }
})
