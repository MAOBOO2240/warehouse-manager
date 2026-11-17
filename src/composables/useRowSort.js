// 庫存清單的拖曳排序：按住左側六點圖示即可拖曳，或在列上長按約半秒。
// 畫面上可以有好幾張表（每個存放地點一張）：一列只在自己那張表裡調整順序。
import { nextTick, onBeforeUnmount, ref } from 'vue'

/**
 * keysOf(group)：這一組（一張表）目前看得到的列 key，依畫面順序
 * canStart()：可不可以開始拖曳（唯讀時由呼叫端提示並回傳 false）
 * save(order, group)：放開後順序有變時存檔；order 是這一組新的 key 順序
 */
export function useRowSort({ keysOf, canStart, save }) {
  const rowEls = new Map()
  const setRowEl = (k, el) => (el ? rowEls.set(k, el) : rowEls.delete(k))
  const sorting = ref(null) // { key, group, order: [這張表目前看得到的列 key…] }
  let ctx = null // { table, y, grab, raf }
  let press = null // 長按偵測 { timer, x, y }

  /** 表格內座標（未縮放的 CSS px）；設定裡的文字大小會整體縮放畫面 */
  function metrics(t = ctx.table) {
    const rect = t.getBoundingClientRect()
    return { top: rect.top, k: t.offsetHeight ? rect.height / t.offsetHeight : 1 }
  }

  function startSort(clientY, key, group) {
    if (sorting.value) return
    if (!canStart()) return
    const el = rowEls.get(key)
    // 表格從被拖的那一列往上找：每個存放地點各有一張表
    const table = el?.closest('table')
    if (!table) return
    const { k } = metrics(table)
    ctx = { table, y: clientY, grab: (clientY - el.getBoundingClientRect().top) / k, raf: 0 }
    sorting.value = { key, group, order: keysOf(group) }
    window.getSelection()?.removeAllRanges()
    document.body.classList.add('is-sorting')
    window.addEventListener('pointermove', onSortMove, { passive: false })
    window.addEventListener('pointerup', finishSort)
    window.addEventListener('pointercancel', cancelSort)
    ctx.raf = requestAnimationFrame(frame)
  }

  function onHandleDown(e, key, group) {
    if (e.button !== 0) return
    e.preventDefault()
    e.stopPropagation()
    startSort(e.clientY, key, group)
  }

  // 長按列（不在按鈕或輸入框上）約 0.45 秒後開始拖曳；期間移動超過 6px 視為一般操作
  function onRowDown(e, key, group) {
    if (e.button !== 0 || sorting.value || e.target.closest('button, input, a, [data-no-sort]')) return
    cancelPress()
    press = {
      x: e.clientX,
      y: e.clientY,
      timer: setTimeout(() => {
        const y = press.y
        press = null
        startSort(y, key, group)
      }, 450),
    }
    window.addEventListener('pointermove', onPressMove)
    window.addEventListener('pointerup', cancelPress)
    window.addEventListener('pointercancel', cancelPress)
  }
  function onPressMove(e) {
    if (!press) return
    if (Math.hypot(e.clientX - press.x, e.clientY - press.y) > 6) cancelPress()
    else press.y = e.clientY
  }
  function cancelPress() {
    if (press) clearTimeout(press.timer)
    press = null
    window.removeEventListener('pointermove', onPressMove)
    window.removeEventListener('pointerup', cancelPress)
    window.removeEventListener('pointercancel', cancelPress)
  }

  function onSortMove(e) {
    if (!ctx) return
    ctx.y = e.clientY
    e.preventDefault()
  }

  // 靠近畫面上下緣時自動捲動頁面
  function autoScroll() {
    const sc = ctx.table.closest('main')
    if (!sc) return
    const r = sc.getBoundingClientRect()
    const edge = 56
    if (ctx.y < r.top + edge) sc.scrollTop -= Math.ceil((r.top + edge - ctx.y) / 4)
    else if (ctx.y > r.bottom - edge) sc.scrollTop += Math.ceil((ctx.y - (r.bottom - edge)) / 4)
  }

  async function frame() {
    if (!ctx || !sorting.value) return
    autoScroll()
    const s = sorting.value
    const { top, k } = metrics()
    const py = (ctx.y - top) / k
    const others = s.order.filter((key) => key !== s.key)
    let idx = 0
    for (const key of others) {
      const el = rowEls.get(key)
      if (el && py > el.offsetTop + el.offsetHeight / 2) idx++
      else break
    }
    if (idx !== s.order.indexOf(s.key)) {
      const before = new Map(others.map((key) => [key, rowEls.get(key)?.offsetTop ?? 0]))
      const order = [...others]
      order.splice(idx, 0, s.key)
      sorting.value = { ...s, order }
      await nextTick()
      // 其他列平滑讓位
      for (const [key, oldTop] of before) {
        const el = rowEls.get(key)
        const dy = el ? oldTop - el.offsetTop : 0
        if (!dy) continue
        el.style.transition = 'none'
        el.style.transform = `translateY(${dy}px)`
        requestAnimationFrame(() => {
          el.style.transition = 'transform 180ms cubic-bezier(.2,.8,.2,1)'
          el.style.transform = ''
        })
      }
    }
    if (!ctx || !sorting.value) return
    const el = rowEls.get(sorting.value.key)
    if (el) {
      // 拖曳中的列跟著游標，但不超出表格範圍
      const m = metrics()
      const body = el.parentElement
      const minTop = body.offsetTop
      const maxTop = body.offsetTop + body.offsetHeight - el.offsetHeight
      const want = Math.min(Math.max((ctx.y - m.top) / m.k - ctx.grab, minTop), maxTop)
      el.style.transform = `translateY(${want - el.offsetTop}px)`
    }
    ctx.raf = requestAnimationFrame(frame)
  }

  function stopSort() {
    if (ctx) cancelAnimationFrame(ctx.raf)
    ctx = null
    window.removeEventListener('pointermove', onSortMove)
    window.removeEventListener('pointerup', finishSort)
    window.removeEventListener('pointercancel', cancelSort)
    document.body.classList.remove('is-sorting')
    for (const el of rowEls.values()) {
      el.style.transition = ''
      el.style.transform = ''
    }
  }

  async function finishSort() {
    const s = sorting.value
    stopSort()
    if (!s) return
    if (s.order.join('\u0001') === keysOf(s.group).join('\u0001')) {
      sorting.value = null
      return
    }
    try {
      await save(s.order, s.group)
    } finally {
      sorting.value = null
    }
  }

  function cancelSort() {
    stopSort()
    sorting.value = null
  }

  onBeforeUnmount(() => {
    cancelPress()
    cancelSort()
  })

  return { sorting, setRowEl, onHandleDown, onRowDown, cancelSort }
}
