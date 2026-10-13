import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { api, ApiError } from '../api'
import { useProjectStore } from './project'

export const useLocationStore = defineStore('locations', () => {
  const items = ref([])

  const sorted = computed(() => [...items.value].sort((a, b) => a.id.localeCompare(b.id, 'zh-Hant', { numeric: true })))
  const byId = computed(() => new Map(items.value.map((l) => [l.id, l])))

  function get(id) {
    return byId.value.get(id)
  }

  /** 顯示用名稱：LOC-A-01-01（A區 01架 01層） */
  function label(id) {
    const l = get(id)
    if (!l) return id
    const parts = [l.zone, l.shelf && `${l.shelf}架`, l.layer && `${l.layer}層`].filter(Boolean).join(' ')
    return parts ? `${l.id}（${parts}）` : l.id
  }

  /** 依倉區、貨架、層號建議代碼，例如 A區 / 01 / 01 → LOC-A-01-01 */
  function suggestId(zone, shelf, layer) {
    const z = String(zone ?? '').trim().replace(/區$/, '')
    const parts = [z, String(shelf ?? '').trim(), String(layer ?? '').trim()].filter(Boolean)
    return parts.length ? `LOC-${parts.join('-')}`.toUpperCase() : ''
  }

  async function persist(list) {
    const project = useProjectStore()
    project.assertWritable()
    await api.writeDataAtomic(project.current, 'locations', list)
    items.value = list
  }

  /** isNew 為 true 時新增；否則依 id 更新（庫位代碼建立後不可修改） */
  async function save(input, isNew) {
    const l = {
      id: String(input.id ?? '').trim(),
      zone: String(input.zone ?? '').trim(),
      shelf: String(input.shelf ?? '').trim(),
      layer: String(input.layer ?? '').trim(),
      description: String(input.description ?? '').trim(),
    }
    if (!l.id) throw new ApiError('INVALID_INPUT', '請輸入庫位代碼')
    if (/[,"\r\n]/.test(l.id)) throw new ApiError('INVALID_INPUT', '庫位代碼不可包含逗號、引號或換行')
    if (isNew && items.value.some((x) => x.id.toLowerCase() === l.id.toLowerCase()))
      throw new ApiError('INVALID_INPUT', `庫位代碼 ${l.id} 已存在`)
    const list = isNew ? [...items.value, l] : items.value.map((x) => (x.id === l.id ? l : x))
    await persist(list)
    return l
  }

  async function remove(id) {
    await persist(items.value.filter((l) => l.id !== id))
  }

  return { items, sorted, byId, get, label, suggestId, save, remove }
})
