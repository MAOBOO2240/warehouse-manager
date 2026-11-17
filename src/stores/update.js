import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '../api'
import { useUiStore } from './ui'
import { useProjectStore } from './project'

// 自動更新試過的版本：萬一裝完還是舊版（發行版本和程式對不上），不會每次開啟都重裝一次
const TRIED_KEY = 'warehouse-auto-update-tried'
// 程式一直開著時，每隔幾小時再檢查一次
const RECHECK_MS = 3 * 60 * 60 * 1000
// 自動更新開始前的倒數秒數（這段時間可以按「稍後」取消）
const COUNTDOWN_SECONDS = 5
// 開啟程式後多久以內查到新版本才自動安裝：網路很慢、過了這段時間才查到的話，使用者可能已經在輸入資料了，改成只詢問
const AUTO_WINDOW_MS = 12 * 1000

// 一個發行版本的代號：commit＋編譯代號（同一個 commit 重新編譯過，代號不同，算另一個版本）
const versionKey = (r) => [r?.latest, r?.latest_id].filter(Boolean).join('#')

function triedBefore(key) {
  try {
    return !!key && localStorage.getItem(TRIED_KEY) === key
  } catch {
    return false
  }
}
function setTried(key) {
  try {
    if (key) localStorage.setItem(TRIED_KEY, key)
    else localStorage.removeItem(TRIED_KEY)
  } catch {
    /* 忽略 */
  }
}

/**
 * 更新：開啟程式時檢查一次，之後每隔幾小時再檢查；設定裡也可以手動檢查。
 * - 「自動更新」開著（預設）：剛開啟程式時發現新版本，倒數幾秒後直接下載安裝、重新開啟（倒數時可以按「稍後」）。
 * - 關掉、或是使用到一半才發現新版本：跳出更新視窗詢問；按「稍後」之後這次開著的期間不再自動跳
 *   （齒輪上會留一個小點提醒）。
 * 每次檢查的結果（成功或失敗）都記在 status，顯示在設定裡——查不到新版本時看得到原因，不會無聲無息。
 */
export const useUpdateStore = defineStore('update', () => {
  const info = ref(null) // { version, build, commit, can_update, repo_url, has_token }
  const result = ref(null) // 最近一次成功檢查的結果
  const show = ref(false)
  const checking = ref(false)
  const installing = ref(false)
  const dismissed = ref(false)
  const auto = ref(false) // 這次更新是「自動更新」開始的（視窗顯示倒數／正在自動更新）
  const countdown = ref(0) // 自動更新開始前還剩幾秒；0 表示沒有在倒數
  const error = ref('') // 最近一次安裝失敗的原因（顯示在更新視窗裡）
  // 最近一次檢查：{ at: Date, ok: true, hasUpdate, canUpdate } 或 { at: Date, ok: false, message }
  const status = ref(null)
  let timer = null
  let ticker = null
  let launchedAt = 0 // start() 被呼叫（程式剛開啟）的時間

  async function loadInfo() {
    try {
      info.value = await api.updateInfo()
    } catch {
      info.value = null
    }
  }

  function stopCountdown() {
    if (ticker) clearInterval(ticker)
    ticker = null
    countdown.value = 0
  }

  /**
   * manual：使用者自己按「檢查更新」，沒有新版或失敗時也要告訴他。
   * startup：剛開啟程式的那一次檢查——只有這時候、而且很快就查到（AUTO_WINDOW_MS 以內）才會自動安裝，
   *          使用到一半不會突然重新開啟。
   */
  async function check(manual = false, startup = false) {
    if (checking.value || installing.value || countdown.value > 0) return
    const ui = useUiStore()
    checking.value = true
    let autoInstall = false
    try {
      const r = await api.updateCheck()
      result.value = r
      status.value = { at: new Date(), ok: true, hasUpdate: !!r.has_update, canUpdate: !!r.can_update }
      if (r.has_update) {
        // 資料檔異常（唯讀、正在詢問要不要還原）時不自動更新，先讓使用者把眼前的事處理完
        const calm = !useProjectStore().readOnly
        const justLaunched = startup && !manual && Date.now() - launchedAt <= AUTO_WINDOW_MS
        if (justLaunched && calm && ui.settings.autoUpdate && r.can_update && !triedBefore(versionKey(r))) autoInstall = true
        else if (manual || !dismissed.value) show.value = true
      } else {
        // 已經是最新版本：之前記下的「自動更新試過的版本」用不到了
        setTried('')
        if (manual) ui.ok(r.can_update ? '已經是最新版本' : '這個版本是自己在電腦上編譯的，不會自動更新')
      }
    } catch (err) {
      status.value = { at: new Date(), ok: false, message: err.message }
      // 自動檢查查不到就算了（沒有網路也能照常使用），原因留在設定裡；自己按的才跳出訊息
      if (manual) ui.error(`檢查更新失敗：${err.message}`)
    } finally {
      checking.value = false
    }
    if (autoInstall) startAuto()
  }

  /** 自動更新：先倒數幾秒（可以按「稍後」取消，或按「立即更新」不用等），時間到就安裝 */
  function startAuto() {
    auto.value = true
    error.value = ''
    show.value = true
    countdown.value = COUNTDOWN_SECONDS
    ticker = setInterval(() => {
      if (countdown.value > 1) countdown.value -= 1
      else install()
    }, 1000)
  }

  async function install() {
    if (installing.value) return
    const wasAuto = auto.value
    stopCountdown()
    // 自動更新同一個版本只裝一次（倒數結束、或倒數時按了「立即更新」都算）
    if (wasAuto) setTried(versionKey(result.value))
    installing.value = true
    error.value = ''
    try {
      // 成功時程式會自己關掉、開啟新版本
      await api.updateInstall()
    } catch (err) {
      installing.value = false
      error.value = err.message
      // 沒裝成功（例如網路中斷）：下次開啟程式時再自動試一次；這次留著視窗讓使用者決定
      if (wasAuto) setTried('')
      auto.value = false
      show.value = true
      useUiStore().error(`更新失敗：${err.message}`)
    }
  }

  function later() {
    stopCountdown()
    auto.value = false
    show.value = false
    dismissed.value = true
  }

  /** 開啟程式後呼叫一次：稍等畫面出來再檢查，之後定時再查 */
  function start() {
    if (timer) return
    launchedAt = Date.now()
    setTimeout(() => check(false, true), 1500)
    timer = setInterval(() => check(false, false), RECHECK_MS)
  }

  return {
    info, result, show, checking, installing, dismissed, auto, countdown, error, status,
    loadInfo, check, install, later, start,
  }
})
