<script setup>
import { computed, nextTick, onBeforeUnmount, ref } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import QuickTxDialog from '../components/QuickTxDialog.vue'
import { useProductStore } from '../stores/products'
import { useStockStore } from '../stores/stock'
import { useRemarkStore } from '../stores/remarks'
import { useUiStore } from '../stores/ui'
import { useProjectStore } from '../stores/project'
import { fmtInt, fmtTime, locLabel, todayStamp } from '../utils/format'
import { saveCsv } from '../utils/csv'

const products = useProductStore()
const stock = useStockStore()
const remarks = useRemarkStore()
const ui = useUiStore()
const project = useProjectStore()

const q = ref('')

const rows = computed(() => {
  const k = q.value.trim().toLowerCase()
  return stock.inventory
    .filter((r) => r.quantity > 0)
    .map((r) => ({
      ...r,
      name: products.get(r.product_id)?.name ?? r.product_id,
      remark: remarks.get(r.product_id, r.location_id),
    }))
    .filter(
      (r) =>
        !k ||
        r.name.toLowerCase().includes(k) ||
        locLabel(r.location_id).toLowerCase().includes(k) ||
        r.remark.toLowerCase().includes(k),
    )
    .sort((a, b) => a.name.localeCompare(b.name, 'zh-Hant') || a.location_id.localeCompare(b.location_id, 'zh-Hant', { numeric: true }))
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
  <PageHeader title="庫存清單" hint="各品項在各存放地點的即時數量，可直接在列上入庫、出庫或登記損壞">
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
      <table class="table-base min-w-[980px] text-[16px]">
        <thead class="cursor-grab" title="按住表頭左右拖曳可移動顯示範圍" @mousedown="onHeadDown">
          <tr>
            <th class="w-[23%] text-[14px]">品項</th>
            <th class="w-[21%] text-[14px]">存放地點</th>
            <th class="w-[8%] text-right text-[14px]">數量</th>
            <th class="w-[17%] text-[14px]">註記</th>
            <th class="w-[11%] text-[14px]">最後異動</th>
            <th class="text-center text-[14px] whitespace-nowrap">操作</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in rows" :key="rowKey(r)">
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
            <td colspan="6" class="py-14 text-center text-mute">{{ stock.inventory.length ? '沒有符合條件的庫存' : '目前沒有任何庫存，請先入庫' }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>

  <QuickTxDialog v-if="dialog" :mode="dialog.mode" :row="dialog.row" @close="dialog = null" />
</template>
