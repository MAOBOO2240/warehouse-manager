import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { api, ApiError } from '../api'
import { useProductStore } from './products'
import { useLocationStore } from './locations'
import { useStockStore } from './stock'

export const FILE_LABELS = {
  config: '專案設定 config.json',
  products: '商品主檔 products.json',
  locations: '庫位主檔 locations.json',
  inventory: '即時庫存 inventory.json',
  logs: '異動流水帳 logs.json',
}

export const useProjectStore = defineStore('project', () => {
  const ready = ref(false)
  const fatal = ref(null) // 無法啟動的錯誤（例如程式放在唯讀資料夾）
  const rootPath = ref('')
  const version = ref('')
  const projects = ref([])
  const current = ref('')
  const loading = ref(false)
  const issues = ref([]) // 開啟專案時偵測到的異常資料檔
  const recovered = ref(false)

  /** 有資料檔異常時進入唯讀，避免以空資料覆蓋掉損壞但可能可救回的檔案 */
  const readOnly = computed(() => issues.value.length > 0)

  function assertWritable() {
    if (readOnly.value) {
      throw new ApiError('READ_ONLY', '資料檔異常，目前為唯讀模式。請先從備份還原後再操作。')
    }
    if (!current.value) throw new ApiError('NO_PROJECT', '尚未開啟專案')
  }

  async function init() {
    try {
      const info = await api.getAppInfo()
      version.value = info.version
      if (info.root_error) {
        fatal.value = info.root_error.message
        return
      }
      rootPath.value = info.root
      projects.value = info.projects
      const target = projects.value.includes(info.last_project) ? info.last_project : projects.value[0]
      if (target) await open(target)
    } catch (err) {
      fatal.value = err.message
    } finally {
      ready.value = true
    }
  }

  async function refreshList() {
    projects.value = await api.getProjects()
  }

  async function open(name) {
    loading.value = true
    try {
      const res = await api.openProject(name)
      current.value = res.name
      issues.value = res.issues
      recovered.value = res.recovered
      useProductStore().items = res.data.products ?? []
      useLocationStore().items = res.data.locations ?? []
      const stock = useStockStore()
      stock.inventory = res.data.inventory ?? []
      stock.logs = res.data.logs ?? []
    } finally {
      loading.value = false
    }
  }

  async function create(name) {
    const created = await api.createProject(name)
    await refreshList()
    await open(created)
    return created
  }

  /** 對每個有備份的異常檔案執行還原，之後重新開啟專案。回傳仍無法還原的檔案。 */
  async function restoreFromBackups() {
    const failed = []
    for (const issue of issues.value) {
      if (!issue.has_backup || issue.code === 'FILE_LOCKED') {
        failed.push(issue.file)
        continue
      }
      try {
        await api.restoreBackup(current.value, issue.file)
      } catch {
        failed.push(issue.file)
      }
    }
    await open(current.value)
    return failed
  }

  async function exportZip() {
    return api.exportProjectZip(current.value)
  }

  return {
    ready, fatal, rootPath, version, projects, current, loading, issues, recovered, readOnly,
    assertWritable, init, refreshList, open, create, restoreFromBackups, exportZip,
  }
})
