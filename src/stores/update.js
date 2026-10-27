import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '../api'
import { useUiStore } from './ui'

/**
 * 檢查更新：開啟程式時自動檢查一次；設定裡也可以手動檢查。
 * 有新版時跳出更新視窗；按「稍後」之後，這次開著的期間不再自動跳（齒輪上會留一個小點提醒）。
 */
export const useUpdateStore = defineStore('update', () => {
  const info = ref(null) // { version, build, commit, can_update, repo_url }
  const result = ref(null) // 最近一次檢查的結果
  const show = ref(false)
  const checking = ref(false)
  const installing = ref(false)
  const dismissed = ref(false)

  async function loadInfo() {
    try {
      info.value = await api.updateInfo()
    } catch {
      info.value = null
    }
  }

  /** manual：使用者自己按「檢查更新」，沒有新版或失敗時也要告訴他 */
  async function check(manual = false) {
    if (checking.value) return
    const ui = useUiStore()
    checking.value = true
    try {
      const r = await api.updateCheck()
      result.value = r
      if (r.has_update) {
        if (manual || !dismissed.value) show.value = true
      } else if (manual) {
        ui.ok(r.can_update ? '已經是最新版本' : '這個版本是自己在電腦上編譯的，不會自動更新')
      }
    } catch (err) {
      if (manual) ui.error(`檢查更新失敗：${err.message}`)
    } finally {
      checking.value = false
    }
  }

  async function install() {
    if (installing.value) return
    installing.value = true
    try {
      // 成功時程式會自己關掉、開啟新版本
      await api.updateInstall()
    } catch (err) {
      installing.value = false
      useUiStore().error(`更新失敗：${err.message}`)
    }
  }

  function later() {
    show.value = false
    dismissed.value = true
  }

  return { info, result, show, checking, installing, dismissed, loadInfo, check, install, later }
})
