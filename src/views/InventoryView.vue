<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
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

/** 所有有庫存的列（每個品項＋存放地點一列，未套搜尋），依自訂順序排列 */
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

/** 目前選取的存放地點（由左側選單切換）；null = 全部地點 */
const loc = computed(() => ui.invLoc)
const allLocs = computed(() => loc.value === null)

/** 有沒有用到存放地點（全部都是「未指定」時，不顯示存放地點欄） */
const hasNamedLoc = computed(() => allRows.value.some((r) => r.location_id))
const showLocCol = computed(() => allLocs.value && hasNamedLoc.value)

/**
 * 全部地點：同一個品項合成一列（數量是各地點加總），順序照該品項排最前面的那一列；
 * 放在好幾個地點的品項，locs 列出各地點（數量多的在前）
 */
const mergedAll = computed(() => {
  const by = new Map()
  const out = []
  for (const r of allRows.value) {
    let m = by.get(r.product_id)
    if (!m) {
      m = { product_id: r.product_id, name: r.name, location_id: r.location_id, quantity: 0, last_updated: '', remark: '', parts: [], merged: true }
      by.set(r.product_id, m)
      out.push(m)
    }
    m.parts.push(r)
    m.quantity += r.quantity
    if (String(r.last_updated) > String(m.last_updated)) m.last_updated = r.last_updated
  }
  for (const m of out) {
    if (m.parts.length === 1) {
      m.location_id = m.parts[0].location_id
      m.remark = m.parts[0].remark
    } else {
      m.location_id = null // 好幾個地點
      m.locs = [...m.parts].sort((a, b) => b.quantity - a.quantity).map((p) => ({ location_id: p.location_id, quantity: p.quantity }))
      const notes = m.parts.filter((p) => p.remark)
      m.remark = notes.length === 1 ? notes[0].remark : notes.map((p) => `${locLabel(p.location_id)}：${p.remark}`).join('；')
    }
  }
  return out
})
/** 放在好幾個地點時，地點欄的說明（滑鼠移上去看各地點數量） */
const locsTitle = (r) => (r.locs ? r.locs.map((l) => `${locLabel(l.location_id)} ${fmtInt(l.quantity)} 件`).join('\n') : locLabel(r.location_id))

const rows = computed(() => {
  const k = q.value.trim().toLowerCase()
  const base = allLocs.value ? mergedAll.value : allRows.value.filter((r) => r.location_id === loc.value)
  if (!k) return base
  return base.filter(
    (r) =>
      r.name.toLowerCase().includes(k) ||
      (allLocs.value && (r.parts || [r]).some((p) => locLabel(p.location_id).toLowerCase().includes(k))) ||
      r.remark.toLowerCase().includes(k),
  )
})

/** 顯示用的列：拖曳中依暫定順序排列 */
const shownRows = computed(() => {
  const s = sorting.value
  if (!s) return rows.value
  const byKey = new Map(rows.value.map((r) => [rowKey(r), r]))
  return s.order.map((k) => byKey.get(k)).filter(Boolean)
})
const locCount = computed(() => new Set(rows.value.flatMap((r) => (r.parts || [r]).map((p) => p.location_id))).size)

const totalQty = computed(() => rows.value.reduce((t, r) => t + r.quantity, 0))
// 合成一列的品項用「品項＋*」當作這一列的代號（地點不只一個）
/** 最後異動：今年的只寫月日時間（滑鼠移上去看完整時間） */
function fmtShort(iso) {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return fmtTime(iso)
  const full = fmtTime(iso)
  return d.getFullYear() === new Date().getFullYear() ? full.slice(5) : full.slice(0, 10)
}
const rowKey = (r) => (r.merged ? `${r.product_id}\u0000*` : `${r.product_id}\u0000${r.location_id}`)

/* ---- 列上的快速操作 ---- */
const dialog = ref(null) // { mode, row }
function openTx(mode, r) {
  if (project.readOnly) return ui.error('資料檔異常，目前為唯讀模式')
  // 放在好幾個地點的品項：在視窗裡選地點
  dialog.value = { mode, row: { product_id: r.product_id, name: r.name, location_id: r.location_id, locs: r.locs || null } }
}

/* ---- 註記：點一下即可編輯，Enter 或離開欄位時儲存 ---- */
const editing = ref('')
const draft = ref('')
async function editRemark(r) {
  if (project.readOnly || r.locs) return
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

/* ---- 表格的寬度：太窄時（大字、小視窗）先收起「最後異動」欄（數量上滑鼠停一下看得到），
   不用左右拖、品項名稱也不會被擠成兩行；真的放不下（最小視窗＋大字＋有存放地點）才左右捲動 ---- */
const scroller = ref(null)
const narrow = ref(false)
const minTableW = computed(() => (showLocCol.value ? 760 : 660))
let ro = null
function watchWidth(el) {
  ro?.disconnect()
  if (!el || typeof ResizeObserver === 'undefined') return
  // clientWidth 和欄寬一樣是未縮放的 CSS px（設定裡的文字大小會整體縮放畫面）
  ro = new ResizeObserver(() => {
    narrow.value = el.clientWidth < 940
  })
  ro.observe(el)
}
onMounted(() => watchWidth(scroller.value))
watch(scroller, (el) => watchWidth(el))
watch(showLocCol, () => watchWidth(scroller.value))
onBeforeUnmount(() => ro?.disconnect())

/* ---- 按住表頭左右拖曳，捲動表格的可視範圍 ---- */
let drag = null
function onHeadDown(e) {
  const sc = scroller.value
  if (e.button !== 0 || !sc) return
  drag = { sc, x: e.clientX, left: sc.scrollLeft }
  document.body.style.cursor = 'grabbing'
  window.addEventListener('mousemove', onDragMove)
  window.addEventListener('mouseup', onDragEnd)
  e.preventDefault()
}
function onDragMove(e) {
  if (drag) drag.sc.scrollLeft = drag.left - (e.clientX - drag.x)
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
const sorting = ref(null) // { key, order: [目前看得到的列 key…] }
let ctx = null // { y, grab, raf }
let press = null // 長按偵測 { timer, x, y }

const visibleKeys = () => rows.value.map(rowKey)

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
  sorting.value = { key, order: visibleKeys() }
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
  press = {
    x: e.clientX,
    y: e.clientY,
    timer: setTimeout(() => {
      const y = press.y
      press = null
      startSort(y, r)
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
  const sc = tableEl.value?.closest('main')
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
  if (s.order.join('\u0001') === visibleKeys().join('\u0001')) {
    sorting.value = null
    return
  }
  // 只重排看得到的列（目前地點、搜尋結果），其他列維持原位；全部地點時一列是一個品項，存檔時展開成各地點
  const units = allLocs.value ? mergedAll.value : allRows.value
  const visible = new Set(s.order)
  const queue = [...s.order]
  const byKey = new Map(units.map((r) => [rowKey(r), r]))
  const full = units.map((r) => (visible.has(rowKey(r)) ? byKey.get(queue.shift()) : r)).flatMap((u) => u.parts || [u])
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
    const suffix = allLocs.value ? '' : `_${locLabel(loc.value)}`
    // CSV 照樣每個地點一列（方便核對），全部地點時依畫面上的品項順序
    const list = rows.value.flatMap((r) => r.parts || [r])
    const path = await saveCsv(
      `${project.current}_庫存清單${suffix}_${todayStamp()}.csv`,
      ['品項', '存放地點', '數量', '註記', '最後異動'],
      list.map((r) => [r.name, locLabel(r.location_id), r.quantity, r.remark, fmtTime(r.last_updated)]),
    )
    if (path) ui.ok(`已匯出 ${list.length} 筆庫存：\n${path}`)
  } catch (err) {
    ui.error(err)
  }
}
</script>

<template>
  <PageHeader title="庫存清單" :hint="hasNamedLoc ? '同一個品項放在好幾個地點時合成一列；從左側選單選地點可以看單一地點。按住左側六點圖示（或長按該列）可拖曳排序' : '按住左側六點圖示（或長按該列）可拖曳調整順序'">
    <button v-if="rowOrder.hasCustom" type="button" class="btn-ghost" @click="resetOrder">恢復名稱排序</button>
    <button type="button" class="btn-primary" :disabled="!rows.length" @click="exportCsv">匯出 CSV</button>
  </PageHeader>

  <div class="mb-5 flex items-center gap-4">
    <div v-if="hasNamedLoc || !allLocs" class="loc-badge" :title="allLocs ? '顯示所有存放地點' : locLabel(loc)">
      <span class="loc-pin" aria-hidden="true">
        <svg v-if="allLocs" viewBox="0 0 24 24" class="h-[17px] w-[17px]" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 3 3.5 7.5 12 12l8.5-4.5zM3.5 12 12 16.5l8.5-4.5M3.5 16.5 12 21l8.5-4.5" />
        </svg>
        <svg v-else viewBox="0 0 24 24" class="h-[17px] w-[17px]" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 21s-6.5-5.6-6.5-11a6.5 6.5 0 0 1 13 0c0 5.4-6.5 11-6.5 11z" /><circle cx="12" cy="10" r="2.3" />
        </svg>
      </span>
      <span class="min-w-0 truncate" :class="!allLocs && !loc ? 'italic' : ''">{{ allLocs ? '全部地點' : locLabel(loc) }}</span>
    </div>
    <input v-model="q" class="field max-w-sm rounded-full text-[16px]" :placeholder="allLocs && hasNamedLoc ? '搜尋品項、存放地點或註記' : '搜尋品項或註記'" />
    <div class="ml-auto flex items-center gap-6 pr-1 text-[15px]">
      <span v-if="showLocCol" class="text-mute"><span class="num text-[19px] text-ink">{{ fmtInt(locCount) }}</span> 個地點</span>
      <span class="text-mute">共 <span class="num text-[19px] text-ink">{{ fmtInt(rows.length) }}</span> {{ allLocs ? '項' : '筆' }}</span>
      <span class="text-mute">合計 <span class="num text-[19px] text-ink">{{ fmtInt(totalQty) }}</span> 件</span>
    </div>
  </div>

  <div v-if="!rows.length" class="panel py-14 text-center text-mute">
    {{ stock.inventory.length ? '沒有符合條件的庫存' : '目前沒有任何庫存，請先入庫' }}
  </div>

  <div v-else ref="scroller" class="panel overflow-x-auto">
    <table ref="tableEl" class="table-base table-fixed text-[16px]" :style="{ minWidth: `${minTableW}px` }">
      <colgroup>
        <col class="w-11" />
        <col />
        <col v-if="showLocCol" class="w-[112px]" />
        <col class="w-[90px]" />
        <col :class="showLocCol ? 'w-[16%]' : 'w-[20%]'" />
        <col v-if="!narrow" class="w-[112px]" />
        <col class="w-[244px]" />
      </colgroup>
      <thead class="cursor-grab" title="畫面太窄時，按住表頭左右拖曳可移動顯示範圍" @mousedown="onHeadDown">
        <tr>
          <th class="px-0" aria-label="排序"></th>
          <th class="text-[14px]">品項</th>
          <th v-if="showLocCol" class="text-[14px]">存放地點</th>
          <th class="text-right text-[14px]">數量</th>
          <th class="text-[14px]">註記</th>
          <th v-if="!narrow" class="text-[14px]">最後異動</th>
          <th class="text-center text-[14px] whitespace-nowrap">操作</th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="r in shownRows"
          :key="rowKey(r)"
          :ref="(el) => setRowEl(rowKey(r), el)"
          :class="sorting?.key === rowKey(r) ? 'is-lifted' : ''"
          @pointerdown="onRowDown($event, r)"
        >
          <td class="py-3 pr-0 pl-3">
            <button type="button" class="drag-handle" aria-label="拖曳調整順序" title="按住拖曳調整順序" @pointerdown="onHandleDown($event, r)">
              <svg viewBox="0 0 12 18" class="h-[18px] w-3" fill="currentColor" aria-hidden="true">
                <circle cx="3" cy="3" r="1.6" /><circle cx="9" cy="3" r="1.6" />
                <circle cx="3" cy="9" r="1.6" /><circle cx="9" cy="9" r="1.6" />
                <circle cx="3" cy="15" r="1.6" /><circle cx="9" cy="15" r="1.6" />
              </svg>
            </button>
          </td>
          <td class="py-3">
            <span class="text-[16.5px] leading-snug font-semibold break-words">{{ r.name }}</span>
          </td>
          <td v-if="showLocCol" class="truncate py-3 text-[15px]" :class="r.locs || r.location_id ? '' : 'italic text-mute'" :title="locsTitle(r)">
            <template v-if="r.locs">
              <span class="loc-multi">{{ r.locs.length }} 個地點</span>
            </template>
            <template v-else>{{ locLabel(r.location_id) }}</template>
          </td>
          <td class="num py-3 text-right text-[20px]" :title="narrow ? `最後異動：${fmtTime(r.last_updated)}` : ''">{{ fmtInt(r.quantity) }}</td>
          <td class="py-3">
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
            <span
              v-else-if="r.locs"
              class="block truncate px-3 py-1.5 text-[14px]"
              :class="r.remark ? 'text-ink-2' : 'text-mute/70'"
              :title="r.remark ? `${r.remark}\n（這個品項放在好幾個地點：從左側選地點後可以編輯各地點的註記）` : '這個品項放在好幾個地點：從左側選地點後可以加註記'"
            >{{ r.remark }}</span>
            <button
              v-else
              type="button"
              class="w-full truncate rounded-full px-3 py-1.5 text-left text-[15px] hover:bg-hover"
              :class="r.remark ? '' : 'text-mute/70'"
              :title="r.remark || '點一下新增註記'"
              @click="editRemark(r)"
            >{{ r.remark || '＋ 新增註記' }}</button>
          </td>
          <td v-if="!narrow" class="num py-3 text-[13.5px] whitespace-nowrap text-mute" :title="fmtTime(r.last_updated)">{{ fmtShort(r.last_updated) }}</td>
          <td class="px-2 py-3">
            <div class="flex justify-center gap-1 whitespace-nowrap">
              <button type="button" class="act" style="--tone: var(--color-go)" @click="openTx('in', r)">入庫</button>
              <button type="button" class="act" style="--tone: var(--color-accent)" @click="openTx('out', r)">出庫</button>
              <button type="button" class="act" style="--tone: var(--color-signal)" @click="openTx('damage', r)">損壞</button>
              <button
                type="button"
                class="act"
                style="--tone: var(--color-violet)"
                :title="r.locs || r.location_id ? '把部分數量移到其他存放地點' : '把部分數量放到指定的存放地點'"
                @click="openTx('move', r)"
              >{{ r.locs || r.location_id ? '改變地點' : '新增地點' }}</button>
            </div>
          </td>
        </tr>
      </tbody>
    </table>
  </div>

  <QuickTxDialog v-if="dialog" :mode="dialog.mode" :row="dialog.row" @close="dialog = null" />
</template>
