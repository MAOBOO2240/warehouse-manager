<script setup>
import { computed, nextTick, onBeforeUnmount, ref } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import QuickTxDialog from '../components/QuickTxDialog.vue'
import { useProductStore } from '../stores/products'
import { useStockStore } from '../stores/stock'
import { useRemarkStore } from '../stores/remarks'
import { useUiStore } from '../stores/ui'
import { useProjectStore } from '../stores/project'
import { useRowOrderStore } from '../stores/rowOrder'
import { fmtInt, fmtTime, locLabel, todayStamp } from '../utils/format'
import { saveCsv } from '../utils/csv'

const products = useProductStore()
const stock = useStockStore()
const remarks = useRemarkStore()
const ui = useUiStore()
const project = useProjectStore()
const rowOrder = useRowOrderStore()

const q = ref('')

const byName = (a, b) => a.name.localeCompare(b.name, 'zh-Hant') || a.location_id.localeCompare(b.location_id, 'zh-Hant', { numeric: true })

/** 所有有庫存的列（未套搜尋），依自訂順序排列 */
const allRows = computed(() =>
  rowOrder.sort(
    stock.inventory
      .filter((r) => r.quantity > 0)
      .map((r) => ({
        ...r,
        name: products.get(r.product_id)?.name ?? r.product_id,
        remark: remarks.get(r.product_id, r.location_id),
      })),
    byName,
  ),
)

const rows = computed(() => {
  const k = q.value.trim().toLowerCase()
  if (!k) return allRows.value
  return allRows.value.filter(
    (r) =>
      r.name.toLowerCase().includes(k) ||
      locLabel(r.location_id).toLowerCase().includes(k) ||
      r.remark.toLowerCase().includes(k),
  )
})

const totalQty = computed(() => rows.value.reduce((t, r) => t + r.quantity, 0))
const rowKey = (r) => `${r.product_id}\u0000${r.location_id}`

/* ---- 列上的快速操作 ---- */
const dialog = ref(null) // { mode, row }
function openTx(mode, r) {
  if (project.readOnly) return ui.error('資料檔異常，目前為唯讀模式')
  dialog.value = { mode, row: { product_id: r.product_id, name: r.name, location_id: r.location_id } }
}

/* ---- 註記：點一下即可編輯，Enter 或離開欄位時儲存 ---- */
const editing = ref('')
const draft = ref('')
async function editRemark(r) {
  if (project.readOnly) return
  editing.value = rowKey(r)
  draft.value = r.remark
  await nextTick()
  document.getElementById('remark-input')?.focus()
}
async function saveRemark(r) {
  if (editing.value !== rowKey(r)) return
  editing.value = ''
  if (draft.value.trim() === r.remark) return
  try {
    await remarks.set(r.product_id, r.location_id, draft.value)
  } catch (err) {
    ui.error(err)
  }
}
function cancelRemark() {
  editing.value = ''
}

/* ---- 按住表頭左右拖曳，捲動表格可視範圍 ---- */
const scroller = ref(null)
let drag = null
function onHeadDown(e) {
  if (e.button !== 0 || !scroller.value) return
  drag = { x: e.clientX, left: scroller.value.scrollLeft }
  document.body.style.cursor = 'grabbing'
  window.addEventListener('mousemove', onDragMove)
  window.addEventListener('mouseup', onDragEnd)
  e.preventDefault()
}
function onDragMove(e) {
  if (!drag || !scroller.value) return
  scroller.value.scrollLeft = drag.left - (e.clientX - drag.x)
}
function onDragEnd() {
  drag = null
  document.body.style.cursor = ''
  window.removeEventListener('mousemove', onDragMove)
  window.removeEventListener('mouseup', onDragEnd)
}
onBeforeUnmount(onDragEnd)

/* ---- 拖曳排序：按住左側六點圖示即可拖曳，或在列上長按約半秒 ---- */
const tableEl = ref(null)
const rowEls = new Map()
const setRowEl = (k, el) => (el ? rowEls.set(k, el) : rowEls.delete(k))
const sorting = ref(null) // { key, order: [列 key…] }
let ctx = null // { y, grab, raf }
let press = null // 長按偵測 { timer, x, y, row }

const viewRows = computed(() => {
  if (!sorting.value) return rows.value
  const byKey = new Map(rows.value.map((r) => [rowKey(r), r]))
  return sorting.value.order.map((k) => byKey.get(k)).filter(Boolean)
})

/** 表格內座標（未縮放的 CSS px）；設定裡的文字大小會整體縮放畫面 */
function metrics() {
  const t = tableEl.value
  const rect = t.getBoundingClientRect()
  return { top: rect.top, k: t.offsetHeight ? rect.height / t.offsetHeight : 1 }
}

function startSort(clientY, r) {
  if (sorting.value) return
  if (project.readOnly) return ui.error('資料檔異常，目前為唯讀模式')
  const key = rowKey(r)
  const el = rowEls.get(key)
  if (!el || !tableEl.value) return
  const { k } = metrics()
  ctx = { y: clientY, grab: (clientY - el.getBoundingClientRect().top) / k, raf: 0 }
  sorting.value = { key, order: rows.value.map(rowKey) }
  window.getSelection()?.removeAllRanges()
  document.body.classList.add('is-sorting')
  window.addEventListener('pointermove', onSortMove, { passive: false })
  window.addEventListener('pointerup', finishSort)
  window.addEventListener('pointercancel', cancelSort)
  ctx.raf = requestAnimationFrame(frame)
}

function onHandleDown(e, r) {
  if (e.button !== 0) return
  e.preventDefault()
  e.stopPropagation()
  startSort(e.clientY, r)
}

// 長按列（不在按鈕或輸入框上）約 0.45 秒後開始拖曳；期間移動超過 6px 視為一般操作
function onRowDown(e, r) {
  if (e.button !== 0 || sorting.value || e.target.closest('button, input, a, [data-no-sort]')) return
  cancelPress()
  press = { x: e.clientX, y: e.clientY, timer: setTimeout(() => {
    const y = press.y
    press = null
    startSort(y, r)
  }, 450) }
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

// 靠近表格上下緣時自動捲動
function autoScroll() {
  const sc = scroller.value
  if (!sc) return
  const r = sc.getBoundingClientRect()
  const edge = 56
  if (ctx.y < r.top + edge) sc.scrollTop -= Math.ceil((r.top + edge - ctx.y) / 4)
  else if (ctx.y > r.bottom - edge) sc.scrollTop += Math.ceil((ctx.y - (r.bottom - edge)) / 4)
}

async function frame() {
  if (!ctx || !sorting.value) return
  autoScroll()
  const { top, k } = metrics()
  const py = (ctx.y - top) / k
  const s = sorting.value
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
    // 拖曳中的列跟著游標，但不超出表格範圍（否則自動捲動會一直往下捲）
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
  const moved = s.order.join('\u0001') !== rows.value.map(rowKey).join('\u0001')
  if (!moved) {
    sorting.value = null
    return
  }
  // 有搜尋時只重排看得到的列，其餘列維持原位
  const visible = new Set(s.order)
  const queue = [...s.order]
  const byKey = new Map(allRows.value.map((r) => [rowKey(r), r]))
  const full = allRows.value.map((r) => (visible.has(rowKey(r)) ? byKey.get(queue.shift()) : r))
  try {
    await rowOrder.save(full)
  } catch (err) {
    ui.error(err)
  } finally {
    sorting.value = null
  }
}

function cancelSort() {
  stopSort()
  sorting.value = null
}

async function resetOrder() {
  try {
    await rowOrder.reset()
    ui.ok('已恢復依品項名稱排序')
  } catch (err) {
    ui.error(err)
  }
}

onBeforeUnmount(() => {
  cancelPress()
  cancelSort()
})

async function exportCsv() {
  try {
    const path = await saveCsv(
      `${project.current}_庫存清單_${todayStamp()}.csv`,
      ['品項', '存放地點', '數量', '註記', '最後異動'],
      rows.value.map((r) => [r.name, locLabel(r.location_id), r.quantity, r.remark, fmtTime(r.last_updated)]),
    )
    if (path) ui.ok(`已匯出 ${rows.value.length} 筆庫存：\n${path}`)
  } catch (err) {
    ui.error(err)
  }
}
</script>

<template>
  <PageHeader title="庫存清單" hint="各品項在各存放地點的即時數量；按住列左側的六點圖示（或長按該列）可拖曳調整順序">
    <button v-if="rowOrder.hasCustom" type="button" class="btn-ghost" @click="resetOrder">恢復名稱排序</button>
    <button type="button" class="btn-primary" :disabled="!rows.length" @click="exportCsv">匯出 CSV</button>
  </PageHeader>

  <div class="mb-5 flex items-center gap-4">
    <input v-model="q" class="field max-w-md rounded-full text-[16px]" placeholder="搜尋品項、存放地點或註記" />
    <div class="ml-auto flex items-center gap-6 pr-1 text-[15px]">
      <span class="text-mute">共 <span class="num text-[19px] text-ink">{{ fmtInt(rows.length) }}</span> 筆</span>
      <span class="text-mute">合計 <span class="num text-[19px] text-ink">{{ fmtInt(totalQty) }}</span> 件</span>
    </div>
  </div>

  <div class="panel overflow-hidden">
    <div ref="scroller" class="max-h-[calc(100vh-15.5rem)] overflow-auto">
      <table ref="tableEl" class="table-base min-w-[1020px] text-[16px]">
        <thead class="cursor-grab" title="按住表頭左右拖曳可移動顯示範圍" @mousedown="onHeadDown">
          <tr>
            <th class="w-12 px-0" aria-label="排序"></th>
            <th class="w-[22%] text-[14px]">品項</th>
            <th class="w-[20%] text-[14px]">存放地點</th>
            <th class="w-[8%] text-right text-[14px]">數量</th>
            <th class="w-[17%] text-[14px]">註記</th>
            <th class="w-[11%] text-[14px]">最後異動</th>
            <th class="text-center text-[14px] whitespace-nowrap">操作</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="r in viewRows"
            :key="rowKey(r)"
            :ref="(el) => setRowEl(rowKey(r), el)"
            :class="sorting?.key === rowKey(r) ? 'is-lifted' : ''"
            @pointerdown="onRowDown($event, r)"
          >
            <td class="w-12 py-5 pr-0 pl-3">
              <button
                type="button"
                class="drag-handle"
                aria-label="拖曳調整順序"
                title="按住拖曳調整順序"
                @pointerdown="onHandleDown($event, r)"
              >
                <svg viewBox="0 0 12 18" class="h-[18px] w-3" fill="currentColor" aria-hidden="true">
                  <circle cx="3" cy="3" r="1.6" /><circle cx="9" cy="3" r="1.6" />
                  <circle cx="3" cy="9" r="1.6" /><circle cx="9" cy="9" r="1.6" />
                  <circle cx="3" cy="15" r="1.6" /><circle cx="9" cy="15" r="1.6" />
                </svg>
              </button>
            </td>
            <td class="py-5 text-[16.5px] font-semibold">{{ r.name }}</td>
            <td class="py-5">
              <div class="flex items-center gap-2">
                <span :class="r.location_id ? '' : 'italic text-mute'">{{ locLabel(r.location_id) }}</span>
                <button
                  type="button"
                  class="flex h-7 items-center gap-1 rounded-full bg-raised px-2.5 text-[12.5px] font-semibold text-accent hover:bg-accent-soft"
                  title="把部分數量移到新的存放地點"
                  @click="openTx('move', r)"
                >
                  <svg viewBox="0 0 24 24" class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round"><path d="M12 5v14M5 12h14" /></svg>
                  新增地點
                </button>
              </div>
            </td>
            <td class="num py-5 text-right text-[22px]">{{ fmtInt(r.quantity) }}</td>
            <td class="py-5">
              <input
                v-if="editing === rowKey(r)"
                id="remark-input"
                v-model="draft"
                class="field rounded-full py-1.5 text-[15px]"
                maxlength="200"
                placeholder="輸入註記"
                @keydown.enter.prevent="saveRemark(r)"
                @keydown.esc.prevent="cancelRemark"
                @blur="saveRemark(r)"
              />
              <button
                v-else
                type="button"
                class="w-full truncate rounded-full px-3 py-1.5 text-left text-[15px] hover:bg-hover"
                :class="r.remark ? '' : 'text-mute/80'"
                :title="r.remark || '點一下新增註記'"
                @click="editRemark(r)"
              >{{ r.remark || '＋ 新增註記' }}</button>
            </td>
            <td class="num py-5 text-[13.5px] whitespace-nowrap text-mute">{{ fmtTime(r.last_updated) }}</td>
            <td class="py-5">
              <div class="flex justify-center gap-1.5 whitespace-nowrap">
                <button type="button" class="btn-go btn-sm" @click="openTx('in', r)">入庫</button>
                <button type="button" class="btn-primary btn-sm" @click="openTx('out', r)">出庫</button>
                <button type="button" class="btn-danger btn-sm" @click="openTx('damage', r)">損壞</button>
              </div>
            </td>
          </tr>
          <tr v-if="!rows.length">
            <td colspan="7" class="py-14 text-center text-mute">{{ stock.inventory.length ? '沒有符合條件的庫存' : '目前沒有任何庫存，請先入庫' }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>

  <QuickTxDialog v-if="dialog" :mode="dialog.mode" :row="dialog.row" @close="dialog = null" />
</template>
