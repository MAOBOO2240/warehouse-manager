import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { api } from '../api'
import { useProjectStore } from './project'

const key = (pid, loc) => `${pid}\u0000${loc ?? ''}`

/** 庫存清單的「註記」：每個品項＋存放地點一則，存在 remarks.json */
export const useRemarkStore = defineStore('remarks', () => {
  const items = ref([])
  const map = computed(() => new Map(items.value.map((r) => [key(r.product_id, r.location_id), r.remark])))

  function get(pid, loc) {
    return map.value.get(key(pid, loc)) || ''
  }

  async function set(pid, loc, text) {
    const remark = String(text ?? '').trim()
    const project = useProjectStore()
    project.assertWritable()
    const rest = items.value.filter((r) => key(r.product_id, r.location_id) !== key(pid, loc))
    const list = remark ? [...rest, { product_id: pid, location_id: loc ?? '', remark }] : rest
    await api.writeDataAtomic(project.current, 'remarks', list)
    items.value = list
  }

  return { items, get, set }
})
