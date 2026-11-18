<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import QuickTxDialog from '../components/QuickTxDialog.vue'
import RemoveLocationDialog from '../components/RemoveLocationDialog.vue'
import { useRowSort } from '../composables/useRowSort'
import { useProductStore } from '../stores/products'
import { locCompare, useStockStore } from '../stores/stock'
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
const keyword = computed(() => q.value.trim().toLowerCase())

const byName = (a, b) => a.name.localeCompare(b.name, 'zh-Hant') || a.location_id.localeCompare(b.location_id, 'zh-Hant', { numeric: true })

/** 每個品項放在幾個地點（只算有庫存的） */
const spread = computed(() => {
  const m = new Map()
  for (const r of stock.inventory) if (r.quantity > 0) m.set(r.product_id, (m.get(r.product_id) || 0) + 1)
  return m
})

/** 所有有庫存的列（每個品項＋存放地點一列，未套搜尋），依自訂順序排列；total 是不分地點的總數 */
const allRows = computed(() =>
  rowOrder.sort(
    stock.inventory
      .filter((r) => r.quantity > 0)
      .map((r) => ({
        ...r,
        name: products.get(r.product_id)?.name ?? r.product_id,
        remark: remarks.get(r.product_id, r.location_id),
        total: stock.total(r.product_id),
        spread: spread.value.get(r.product_id) || 1,
      })),
    byName,
  ),
)

/** 目前選取的存放地點（由左側選單或地點標籤切換）；null = 全部地點 */
const loc = computed(() => ui.invLoc)
const allLocs = computed(() => loc.value === null)

/** 有沒有用到存放地點（全部都是「未指定」時不分區、不顯示存放地點欄，也沒有顯示方式可以切換） */
const hasNamedLoc = computed(() => allRows.value.some((r) => r.location_id))

/**
 * 顯示方式：loc＝每個存放地點各自一區；item＝每個品項一列（不分地點的總數）。
 * 指定了單一地點時一律是 loc（只有那一區）；沒有用存放地點時就是一張不分地點的清單。
 */
const VIEWS = [
  { id: 'loc', label: '依地點', title: '每個存放地點各自一區', icon: 'M12 21s-6.5-5.6-6.5-11a6.5 6.5 0 0 1 13 0c0 5.4-6.5 11-6.5 11z' },
  { id: 'item', label: '品項總數', title: '每個品項一列，數量是不分存放地點的總數', icon: 'M8.5 6.5h11M8.5 12h11M8.5 17.5h11M4.5 6.5h.01M4.5 12h.01M4.5 17.5h.01' },
]
const view = computed(() => {
  if (!allLocs.value) return 'loc'
  if (!hasNamedLoc.value) return 'item'
  return ui.settings.invView === 'item' ? 'item' : 'loc'
})

// 全部都是「未指定」之後（例如移除了最後一個地點），不會停在「只看未指定」
watch(
  () => !hasNamedLoc.value && ui.invLoc !== null,
  (stuck) => {
    if (stuck) ui.invLoc = null
  },
  { immediate: true },
)

const hit = (r, k) => r.name.toLowerCase().includes(k) || r.remark.toLowerCase().includes(k)

/**
 * 依地點：每個有庫存的存放地點各自一區（依名稱排序、「未指定」排最後）。
 * 搜尋的字和地點名稱相符時那個地點整區都顯示；沒有符合的列的地點不顯示。
 */
const sections = computed(() => {
  const k = keyword.value
  const by = new Map()
  for (const r of allRows.value) {
    const list = by.get(r.location_id)
    if (list) list.push(r)
    else by.set(r.location_id, [r])
  }
  return stock.locationStats
    .filter((s) => allLocs.value || s.loc === loc.value)
    .map((s) => {
      const all = by.get(s.loc) || []
      const whole = !k || (allLocs.value && locLabel(s.loc).toLowerCase().includes(k))
      return { key: `loc:${s.loc}`, group: s.loc, loc: s.loc, items: s.items, qty: s.qty, rows: whole ? all : all.filter((r) => hit(r, k)) }
    })
    .filter((s) => s.rows.length)
})

/**
 * 品項總數：同一個品項合成一列（數量是各地點加總），順序照該品項排最前面的那一列。
 * where 是各地點的數量（依地點名稱）；放在好幾個地點的品項另有 locs（數量多的在前），操作時在視窗裡選地點。
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
    m.where = [...m.parts].sort((a, b) => locCompare(a.location_id, b.location_id))
    if (m.parts.length === 1) {
      m.location_id = m.parts[0].location_id
      m.remark = m.parts[0].remark
    } else {
      m.location_id = null // 好幾個地點
      m.locs = [...m.parts].sort((a, b) => b.quantity - a.quantity).map((p) => ({ location_id: p.location_id, quantity: p.quantity }))
      const notes = m.where.filter((p) => p.remark)
      m.remark = notes.length === 1 ? notes[0].remark : notes.map((p) => `${locLabel(p.location_id)}：${p.remark}`).join('；')
    }
  }
  return out
})
const itemRows = computed(() => {
  const k = keyword.value
  if (!k) return mergedAll.value
  return mergedAll.value.filter((r) => hit(r, k) || (hasNamedLoc.value && r.parts.some((p) => locLabel(p.location_id).toLowerCase().includes(k))))
})

/** 畫面上的區塊：依地點時每個地點一塊，否則整張清單一塊（未套拖曳中的暫定順序） */
const baseBlocks = computed(() => {
  if (view.value === 'loc') return sections.value
  return itemRows.value.length ? [{ key: 'items', group: null, rows: itemRows.value }] : []
})

// 合成一列的品項用「品項＋*」當作這一列的代號（地點不只一個）
const rowKey = (r) => (r.merged ? `${r.product_id}\u0000*` : `${r.product_id}\u0000${r.location_id}`)

/* ---- 拖曳排序：按住左側六點圖示即可拖曳，或在列上長按約半秒；依地點時一列只在自己那一區裡移動 ---- */
const { sorting, setRowEl, onHandleDown, onRowDown } = useRowSort({
  keysOf: (group) => (baseBlocks.value.find((b) => b.group === group)?.rows ?? []).map(rowKey),
  canStart() {
    if (!project.readOnly) return true
    ui.error('資料檔異常，目前為唯讀模式')
    return false
  },
  save: saveOrder,
})

// 存檔。依地點：只重排看得到的列（那個地點、搜尋結果），其他列維持原位。
// 品項總數：一列是一個品項，只搬被拖曳的那個品項（見 movePartsOnly）
async function saveOrder(order) {
  const merged = view.value === 'item'
  let full = merged ? movePartsOnly(order) : null
  if (!full) {
    const units = merged ? mergedAll.value : allRows.value
    const visible = new Set(order)
    const queue = [...order]
    const byKey = new Map(units.map((r) => [rowKey(r), r]))
    full = units.map((r) => (visible.has(rowKey(r)) ? byKey.get(queue.shift()) : r)).flatMap((u) => u.parts || [u])
  }
  try {
    await rowOrder.save(full)
  } catch (err) {
    ui.error(err)
  }
}

/**
 * 品項總數：拖曳一個品項時，只搬那個品項在各地點的列，其他的列一律留在原位——
 * 各地點那一區自己排好的順序，不會因為在這裡拖了別的品項而被打亂。
 * 搬到哪裡：新順序裡排在它後面的那個品項，最前面的那一列之前；搬到最後面時，接在前一個品項之後。
 * order 是拖曳後畫面上的順序；看不出是哪一個品項被搬動時回傳 null（由呼叫端整個重排）。
 */
function movePartsOnly(order) {
  const before = itemRows.value.map(rowKey)
  // 被搬動的品項：把它拿掉之後，前後兩個順序一模一樣
  const movedKey = order.find((k, i) => {
    if (k === before[i]) return false
    const rest = order.filter((x) => x !== k)
    return before.filter((x) => x !== k).every((x, j) => x === rest[j])
  })
  const byKey = new Map(mergedAll.value.map((m) => [rowKey(m), m]))
  const moved = byKey.get(movedKey)
  if (!moved) return null
  const others = allRows.value.filter((r) => r.product_id !== moved.product_id)
  const firstRowOf = (key) => others.findIndex((r) => r.product_id === byKey.get(key)?.product_id)
  const i = order.indexOf(movedKey)
  let at
  if (i + 1 < order.length) {
    at = firstRowOf(order[i + 1])
  } else {
    // 搬到最後面：放在「前一個品項的下一個品項」最前面那一列之前；前一個品項已經是最後一個就放到最後
    const prev = firstRowOf(order[i - 1])
    if (prev < 0) return null
    const earlier = new Set(others.slice(0, prev + 1).map((r) => r.product_id))
    at = others.findIndex((r, j) => j > prev && !earlier.has(r.product_id))
    if (at < 0) at = others.length
  }
  if (at < 0) return null
  return [...others.slice(0, at), ...moved.parts, ...others.slice(at)]
}

/** 顯示用的區塊：拖曳中的那一塊依暫定順序排列 */
const blocks = computed(() => {
  const s = sorting.value
  if (!s) return baseBlocks.value
  return baseBlocks.value.map((b) => {
    if (b.group !== s.group) return b
    const byKey = new Map(b.rows.map((r) => [rowKey(r), r]))
    return { ...b, rows: s.order.map((k) => byKey.get(k)).filter(Boolean) }
  })
})

/* ---- 右上角的統計：目前畫面上顯示的範圍 ---- */
const shownRows = computed(() => baseBlocks.value.flatMap((b) => b.rows))
const locCount = computed(() => new Set(shownRows.value.flatMap((r) => (r.parts || [r]).map((p) => p.location_id))).size)
const totalQty = computed(() => shownRows.value.reduce((t, r) => t + r.quantity, 0))

const hint = computed(() =>
  hasNamedLoc.value
    ? '每個存放地點各自一區；「總數」是不分地點的合計。按住左側六點圖示（或長按該列）可拖曳排序'
    : '按住左側六點圖示（或長按該列）可拖曳調整順序',
)

/** 最後異動：今年的只寫月日時間（滑鼠移上去看完整時間） */
function fmtShort(iso) {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return fmtTime(iso)
  const full = fmtTime(iso)
  return d.getFullYear() === new Date().getFullYear() ? full.slice(5) : full.slice(0, 10)
}

/* ---- 收合／展開某個地點的那一區（只記在這個畫面，不存檔） ---- */
const collapsed = reactive(new Set())
const isOpen = (b) => view.value !== 'loc' || !collapsed.has(b.loc)
function toggleSection(l) {
  if (collapsed.has(l)) collapsed.delete(l)
  else collapsed.add(l)
}
// 指定看某個地點時，那一區一定是展開的
watch(loc, (l) => {
  if (l !== null) collapsed.delete(l)
})
// 搜尋時全部展開，找到的列不會藏在收合的區塊裡
watch(keyword, (k) => {
  if (k) collapsed.clear()
})

/** 「N 個地點」：切到品項總數並搜尋這個品項，各地點各有幾件一次看清楚 */
function showItemSplit(r) {
  q.value = r.name
  ui.invLoc = null
  ui.settings.invView = 'item'
}
/** 放在好幾個地點的品項，註記是各地點各一則：切到依地點並搜尋這個品項，就能分別查看、編輯 */
function showItemByLoc(r) {
  q.value = r.name
  ui.invLoc = null
  ui.settings.invView = 'loc'
}

/* ---- 列上的快速操作（改變地點併在出庫視窗裡） ---- */
const dialog = ref(null) // { mode, row }
function openTx(mode, r) {
  if (project.readOnly) return ui.error('資料檔異常，目前為唯讀模式')
  // 放在好幾個地點的品項：在視窗裡選地點
  dialog.value = { mode, row: { product_id: r.product_id, name: r.name, location_id: r.location_id, locs: r.locs || null } }
}
// 沒有用存放地點的專案不提地點
const outTitle = '出貨，或把這裡的庫存改放到其他地點'

/* ---- 移除存放地點 ---- */
const removing = ref(null) // 要移除的地點名稱
function askRemove(name) {
  if (project.readOnly) return ui.error('資料檔異常，目前為唯讀模式')
  removing.value = name
}
function closeRemove() {
  const name = removing.value
  removing.value = null
  // 移除之後，原本只看這個地點的話回到全部地點
  if (ui.invLoc === name && !stock.locationStats.some((s) => s.loc === name)) ui.invLoc = null
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

/* ---- 版面寬度：太窄時（大字、小視窗）先收起「最後異動」欄（數量上滑鼠停一下看得到），
   再窄就把「總數」欄收窄（「N 個地點」排到數字下面）；「品項總數」多了地點欄，再窄時連「註記」欄也收起
   （註記是各地點各一則，切到「依地點」看得到、也可以編輯），操作按鈕才不會被擠出畫面。
   品項名稱可以換行，數字與操作按鈕不換行；真的還放不下才讓表格左右捲動 ---- */
const pageEl = ref(null)
const narrow = ref(false)
const tight = ref(false)
const hideRemark = computed(() => tight.value && view.value === 'item' && hasNamedLoc.value)
const minTableW = computed(() => (view.value === 'item' && hasNamedLoc.value ? (hideRemark.value ? 620 : 740) : 660))
const totalColW = computed(() => (tight.value ? 120 : 176))
const remarkCol = computed(() => {
  if (view.value === 'loc') return tight.value ? 'w-[15%]' : 'w-[16%]'
  return hasNamedLoc.value ? 'w-[15%]' : 'w-[20%]'
})
let ro = null
onMounted(() => {
  const el = pageEl.value
  if (!el || typeof ResizeObserver === 'undefined') return
  // clientWidth 和欄寬一樣是未縮放的 CSS px（設定裡的文字大小會整體縮放畫面）。
  // 換回寬版面的門檻多留 12px：欄位一變、頁面捲軸跟著出現或消失時，不會在兩種版面之間來回跳
  ro = new ResizeObserver(() => {
    const w = el.clientWidth
    narrow.value = w < (narrow.value ? 952 : 940)
    tight.value = w < (tight.value ? 782 : 770)
  })
  ro.observe(el)
})
onBeforeUnmount(() => ro?.disconnect())

/* ---- 按住表頭左右拖曳，捲動那張表格的可視範圍 ---- */
let drag = null
function onHeadDown(e) {
  const sc = e.currentTarget.closest('.inv-scroll')
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

async function resetOrder() {
  try {
    await rowOrder.reset()
    ui.ok('已恢復依品項名稱排序')
  } catch (err) {
    ui.error(err)
  }
}

async function exportCsv() {
  try {
    const suffix = allLocs.value ? '' : `_${locLabel(loc.value)}`
    // 每個品項＋存放地點一列（方便核對），順序和畫面相同；收合起來的地點也照樣匯出
    const list = shownRows.value.flatMap((r) => r.parts || [r])
    const path = await saveCsv(
      `${project.current}_庫存清單${suffix}_${todayStamp()}.csv`,
      ['品項', '存放地點', '數量', '總數（不分地點）', '註記', '最後異動'],
      list.map((r) => [r.name, locLabel(r.location_id), r.quantity, r.total, r.remark, fmtTime(r.last_updated)]),
    )
    if (path) ui.ok(`已匯出 ${list.length} 筆庫存：\n${path}`)
  } catch (err) {
    ui.error(err)
  }
}
</script>

<template>
  <PageHeader title="庫存清單" :hint="hint">
    <button v-if="rowOrder.hasCustom" type="button" class="btn-ghost" @click="resetOrder">恢復名稱排序</button>
    <button type="button" class="btn-primary" :disabled="!shownRows.length" @click="exportCsv">匯出 CSV</button>
  </PageHeader>

  <div ref="pageEl">
    <div class="mb-5 flex flex-wrap items-center gap-x-4 gap-y-3">
      <template v-if="!allLocs">
        <!-- 只看一個存放地點（從左側選單或地點標籤點進來） -->
        <div class="loc-badge" :title="locLabel(loc)">
          <span class="loc-pin" aria-hidden="true">
            <svg viewBox="0 0 24 24" class="h-[17px] w-[17px]" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 21s-6.5-5.6-6.5-11a6.5 6.5 0 0 1 13 0c0 5.4-6.5 11-6.5 11z" /><circle cx="12" cy="10" r="2.3" />
            </svg>
          </span>
          <span class="min-w-0 truncate" :class="loc ? '' : 'italic'">{{ locLabel(loc) }}</span>
        </div>
        <button type="button" class="btn-ghost btn-sm" @click="ui.invLoc = null">顯示全部地點</button>
      </template>
      <div
        v-else-if="hasNamedLoc"
        class="seg"
        role="group"
        aria-label="顯示方式"
        :style="{ '--tone': 'var(--color-violet)', '--tone-fg': 'var(--color-on-violet)' }"
      >
        <button
          v-for="v in VIEWS"
          :key="v.id"
          type="button"
          class="seg-item"
          :class="view === v.id ? 'is-on' : ''"
          :aria-pressed="view === v.id"
          :title="v.title"
          @click="ui.settings.invView = v.id"
        >
          <svg viewBox="0 0 24 24" class="h-[15px] w-[15px]" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path :d="v.icon" /><circle v-if="v.id === 'loc'" cx="12" cy="10" r="2.3" />
          </svg>
          {{ v.label }}
        </button>
      </div>

      <!-- 搜尋框（surface：清除鈕疊在深色的輸入框上，圖示要用淺色） -->
      <div class="surface relative max-w-sm min-w-52 flex-1">
        <input v-model="q" class="field rounded-full text-[16px]" :class="q ? 'pr-11' : ''" :placeholder="allLocs && hasNamedLoc ? '搜尋品項、存放地點或註記' : '搜尋品項或註記'" />
        <button
          v-if="q"
          type="button"
          class="absolute top-1/2 right-2 flex h-8 w-8 -translate-y-1/2 items-center justify-center rounded-full text-mute hover:bg-hover hover:text-ink"
          aria-label="清除搜尋"
          title="清除搜尋"
          @click="q = ''"
        >
          <svg viewBox="0 0 24 24" class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
        </button>
      </div>

      <div class="ml-auto flex items-center gap-6 pr-1 text-[15px]">
        <span v-if="allLocs && hasNamedLoc" class="text-mute"><span class="num text-[19px] text-ink">{{ fmtInt(locCount) }}</span> 個地點</span>
        <span class="text-mute">共 <span class="num text-[19px] text-ink">{{ fmtInt(shownRows.length) }}</span> {{ view === 'item' ? '項' : '筆' }}</span>
        <span class="text-mute">合計 <span class="num text-[19px] text-ink">{{ fmtInt(totalQty) }}</span> 件</span>
      </div>
    </div>

    <div v-if="!blocks.length" class="panel py-14 text-center text-mute">
      {{ stock.inventory.length ? '沒有符合條件的庫存' : '目前沒有任何庫存，請先入庫' }}
    </div>

    <div v-else class="space-y-5">
      <section v-for="b in blocks" :key="b.key" class="panel overflow-hidden">
        <!-- 依地點：這一區的標題列（地點名稱、幾項幾件、移除地點、收合） -->
        <header v-if="view === 'loc'" class="sec-head flex items-center gap-3 py-3 pr-4 pl-3" :class="[b.loc ? '' : 'is-plain', isOpen(b) ? 'is-open' : '']">
          <span class="loc-pin" :class="b.loc ? '' : 'bg-raised text-mute'" aria-hidden="true">
            <svg viewBox="0 0 24 24" class="h-[17px] w-[17px]" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 21s-6.5-5.6-6.5-11a6.5 6.5 0 0 1 13 0c0 5.4-6.5 11-6.5 11z" /><circle cx="12" cy="10" r="2.3" />
            </svg>
          </span>
          <button
            type="button"
            class="ml-2 min-w-0 truncate rounded-lg text-left text-[17px] font-bold hover:text-violet"
            :class="b.loc ? '' : 'italic'"
            :aria-expanded="isOpen(b)"
            :title="`${locLabel(b.loc)}（點一下${isOpen(b) ? '收合' : '展開'}）`"
            @click="toggleSection(b.loc)"
          >{{ locLabel(b.loc) }}</button>
          <span class="shrink-0 text-[14px] whitespace-nowrap text-mute">
            <span class="num text-[16.5px] text-ink">{{ fmtInt(b.items) }}</span> 項・<span class="num text-[16.5px] text-ink">{{ fmtInt(b.qty) }}</span> 件
          </span>
          <span v-if="b.rows.length < b.items" class="shrink-0 text-[13px] whitespace-nowrap text-mute">（符合 {{ fmtInt(b.rows.length) }} 項）</span>
          <span class="flex-1" />
          <button
            v-if="b.loc"
            type="button"
            class="act shrink-0 gap-1.5"
            style="--tone: var(--color-signal)"
            title="移除這個存放地點，這裡的庫存改放到別處"
            @click="askRemove(b.loc)"
          >
            <svg viewBox="0 0 24 24" class="h-[15px] w-[15px]" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M4 7h16M10 11v6M14 11v6M5.5 7l1 12a2 2 0 0 0 2 1.8h7a2 2 0 0 0 2-1.8l1-12M9 7V4.5h6V7" />
            </svg>
            移除地點
          </button>
          <button
            type="button"
            class="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-raised text-ink-2 transition-colors hover:bg-hover hover:text-ink"
            :aria-expanded="isOpen(b)"
            :aria-label="`${isOpen(b) ? '收合' : '展開'}「${locLabel(b.loc)}」`"
            :title="isOpen(b) ? '收合' : '展開'"
            @click="toggleSection(b.loc)"
          >
            <svg viewBox="0 0 24 24" class="h-4 w-4 transition-transform" :class="isOpen(b) ? '' : '-rotate-90'" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M6 9l6 6 6-6" />
            </svg>
          </button>
        </header>

        <div v-if="isOpen(b)" class="inv-scroll overflow-x-auto">
          <table class="table-base table-fixed text-[16px]" :style="{ minWidth: `${minTableW}px` }">
            <colgroup>
              <col class="w-11" />
              <col />
              <col class="w-[88px]" />
              <col v-if="view === 'loc'" :style="{ width: `${totalColW}px` }" />
              <col v-else-if="hasNamedLoc" :class="hideRemark ? 'w-[27%]' : 'w-[23%]'" />
              <col v-if="!hideRemark" :class="remarkCol" />
              <col v-if="!narrow" class="w-[112px]" />
              <col class="w-[166px]" />
            </colgroup>
            <thead class="cursor-grab" title="畫面太窄時，按住表頭左右拖曳可移動顯示範圍" @mousedown="onHeadDown">
              <tr>
                <th class="px-0" aria-label="排序"></th>
                <th class="text-[14px]">品項</th>
                <th v-if="view === 'loc'" class="pr-4 pl-2 text-right text-[14px]" title="放在這個地點的數量">數量</th>
                <th v-else class="pr-4 pl-2 text-right text-[14px]" :title="hasNamedLoc ? '不分存放地點，這個品項全部的數量' : undefined">{{ hasNamedLoc ? '總數' : '數量' }}</th>
                <th v-if="view === 'loc'" class="pr-2 pl-4 text-[14px]" title="不分存放地點，這個品項全部的數量">
                  <span class="inline-block min-w-12 text-right">總數</span>
                </th>
                <th v-else-if="hasNamedLoc" class="pr-2 pl-4 text-[14px]">存放地點</th>
                <th v-if="!hideRemark" class="pr-2 pl-5 text-[14px]">註記</th>
                <th v-if="!narrow" class="px-3 text-[14px]">最後異動</th>
                <th class="text-center text-[14px] whitespace-nowrap">操作</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="r in b.rows"
                :key="rowKey(r)"
                :ref="(el) => setRowEl(rowKey(r), el)"
                :class="sorting?.key === rowKey(r) ? 'is-lifted' : ''"
                @pointerdown="onRowDown($event, rowKey(r), b.group)"
              >
                <td class="py-3 pr-0 pl-3">
                  <button type="button" class="drag-handle" aria-label="拖曳調整順序" title="按住拖曳調整順序" @pointerdown="onHandleDown($event, rowKey(r), b.group)">
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
                <!-- 依地點：這個地點的數量；品項總數：各地點加總 -->
                <td class="num py-3 pr-4 pl-2 text-right text-[20px] whitespace-nowrap" :title="narrow ? `最後異動：${fmtTime(r.last_updated)}` : ''">{{ fmtInt(r.quantity) }}</td>
                <!-- 依地點：不分地點的總數；不只放在這裡時，按「N 個地點」看各地點各有幾件 -->
                <td v-if="view === 'loc'" class="py-3 pr-2 pl-4">
                  <div class="flex flex-wrap items-center gap-x-2 gap-y-1">
                    <span class="num min-w-12 text-right text-[17px] whitespace-nowrap" :class="r.spread > 1 ? '' : 'text-mute'">{{ fmtInt(r.total) }}</span>
                    <button
                      v-if="r.spread > 1"
                      type="button"
                      class="loc-multi gap-0.5"
                      :title="`這個品項放在 ${r.spread} 個地點：點一下看各地點各有幾件`"
                      @click="showItemSplit(r)"
                    >
                      {{ r.spread }} 個地點
                      <svg viewBox="0 0 24 24" class="h-3 w-3" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 6l6 6-6 6" /></svg>
                    </button>
                  </div>
                </td>
                <!-- 品項總數：各地點各有幾件，點一下只看那個地點 -->
                <td v-else-if="hasNamedLoc" class="py-2.5 pr-2 pl-4">
                  <div class="flex flex-wrap gap-1.5">
                    <button
                      v-for="p in r.where"
                      :key="p.location_id"
                      type="button"
                      class="chip"
                      :title="`${locLabel(p.location_id)} ${fmtInt(p.quantity)} 件（點一下只看這個地點）`"
                      @click="ui.invLoc = p.location_id"
                    >
                      <span class="min-w-0 truncate" :class="p.location_id ? '' : 'italic'">{{ locLabel(p.location_id) }}</span>
                      <span class="chip-num">{{ fmtInt(p.quantity) }}</span>
                    </button>
                  </div>
                </td>
                <td v-if="!hideRemark" class="px-2 py-3">
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
                  <!-- 放在好幾個地點：註記是各地點各一則，這裡只顯示合起來的內容；點一下到「依地點」分別編輯 -->
                  <button
                    v-else-if="r.locs"
                    type="button"
                    class="w-full truncate rounded-full px-3 py-1.5 text-left text-[14px] hover:bg-hover"
                    :class="r.remark ? 'text-ink-2' : 'text-mute'"
                    :title="`${r.remark ? `${r.remark}\n` : ''}（這個品項放在好幾個地點：點一下切到「依地點」，分別編輯各地點的註記）`"
                    @click="showItemByLoc(r)"
                  >{{ r.remark || '＋ 新增註記' }}</button>
                  <button
                    v-else
                    type="button"
                    class="w-full truncate rounded-full px-3 py-1.5 text-left text-[15px] hover:bg-hover"
                    :class="r.remark ? '' : 'text-mute'"
                    :title="r.remark || '點一下新增註記'"
                    @click="editRemark(r)"
                  >{{ r.remark || '＋ 新增註記' }}</button>
                </td>
                <td v-if="!narrow" class="num px-3 py-3 text-[13.5px] whitespace-nowrap text-mute" :title="fmtTime(r.last_updated)">{{ fmtShort(r.last_updated) }}</td>
                <td class="px-2 py-3">
                  <div class="flex justify-center gap-1 whitespace-nowrap">
                    <button type="button" class="act" style="--tone: var(--color-go)" @click="openTx('in', r)">入庫</button>
                    <button type="button" class="act" style="--tone: var(--color-accent)" :title="outTitle" @click="openTx('out', r)">出庫</button>
                    <button type="button" class="act" style="--tone: var(--color-signal)" @click="openTx('damage', r)">損壞</button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>
    </div>

    <!-- 用過、但現在沒有庫存的地點：還會出現在選擇清單裡，可以在這裡移除 -->
    <section v-if="stock.emptyLocations.length" class="panel mt-6 px-6 py-5">
      <h2 class="card-title">沒有庫存的存放地點</h2>
      <p class="mt-1 text-[13px] text-mute">這些地點以前用過，還會出現在選擇清單裡；用不到的可以移除。</p>
      <ul class="mt-3.5 flex flex-wrap gap-2">
        <li v-for="name in stock.emptyLocations" :key="name" class="max-w-full">
          <span class="chip py-1 pr-1 pl-3.5 text-[14px]">
            <span class="min-w-0 truncate" :title="name">{{ name }}</span>
            <button type="button" class="act shrink-0 gap-1" style="--tone: var(--color-signal)" :aria-label="`移除存放地點「${name}」`" @click="askRemove(name)">
              <svg viewBox="0 0 24 24" class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
              移除
            </button>
          </span>
        </li>
      </ul>
    </section>
  </div>

  <QuickTxDialog v-if="dialog" :mode="dialog.mode" :row="dialog.row" @close="dialog = null" />
  <RemoveLocationDialog v-if="removing" :location="removing" @close="closeRemove" />
</template>

<style scoped>
/* 每個存放地點一區的標題列：淡淡的紫色底（未指定是灰色），展開時和下面的表格之間有一條線 */
.sec-head {
  --sec: var(--color-violet);
  background: color-mix(in oklab, var(--sec) 9%, var(--color-panel));
}
.sec-head.is-plain {
  --sec: var(--color-mute);
}
.sec-head.is-open {
  box-shadow: inset 0 -1px 0 color-mix(in oklab, var(--sec) 24%, var(--color-line));
}

/* 「N 個地點」與地點標籤是可以按的：滑鼠移上去底色加深 */
button.loc-multi {
  white-space: nowrap;
  transition: background-color 0.15s;
}
button.loc-multi:hover {
  background: color-mix(in oklab, var(--color-violet) 24%, transparent);
}
button.chip {
  transition: background-color 0.15s;
}
button.chip:hover {
  background: var(--color-hover);
}
</style>
