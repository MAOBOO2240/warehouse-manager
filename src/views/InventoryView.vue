<script setup>
import { computed, ref } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import { useProductStore } from '../stores/products'
import { useStockStore } from '../stores/stock'
import { useUiStore } from '../stores/ui'
import { useProjectStore } from '../stores/project'
import { fmtInt, fmtTime, locLabel, todayStamp } from '../utils/format'
import { saveCsv } from '../utils/csv'

const products = useProductStore()
const stock = useStockStore()
const ui = useUiStore()
const project = useProjectStore()

const q = ref('')

const rows = computed(() => {
  const k = q.value.trim().toLowerCase()
  return stock.inventory
    .filter((r) => r.quantity > 0)
    .map((r) => ({ ...r, name: products.get(r.product_id)?.name ?? r.product_id }))
    .filter((r) => !k || r.name.toLowerCase().includes(k) || locLabel(r.location_id).toLowerCase().includes(k))
    .sort((a, b) => a.name.localeCompare(b.name, 'zh-Hant') || a.location_id.localeCompare(b.location_id, 'zh-Hant', { numeric: true }))
})

const totalQty = computed(() => rows.value.reduce((t, r) => t + r.quantity, 0))

async function exportCsv() {
  try {
    const path = await saveCsv(
      `${project.current}_即時庫存清冊_${todayStamp()}.csv`,
      ['品項', '存放地點', '數量', '最後異動'],
      rows.value.map((r) => [r.name, locLabel(r.location_id), r.quantity, fmtTime(r.last_updated)]),
    )
    if (path) ui.ok(`已匯出 ${rows.value.length} 筆庫存：\n${path}`)
  } catch (err) {
    ui.error(err)
  }
}
</script>

<template>
  <PageHeader title="庫存清冊" hint="各品項在各存放地點的即時數量">
    <button type="button" class="btn-primary" :disabled="!rows.length" @click="exportCsv">匯出 CSV</button>
  </PageHeader>

  <input v-model="q" class="field mb-4 max-w-md" placeholder="搜尋品項名稱或存放地點" />

  <div class="panel overflow-hidden">
    <div class="max-h-[calc(100vh-17rem)] overflow-auto">
      <table class="table-base">
        <thead>
          <tr>
            <th>品項</th>
            <th>存放地點</th>
            <th class="text-right">數量</th>
            <th>最後異動</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in rows" :key="`${r.product_id}|${r.location_id}`">
            <td class="font-semibold">{{ r.name }}</td>
            <td :class="r.location_id ? '' : 'italic text-mute'">{{ locLabel(r.location_id) }}</td>
            <td class="num text-right text-[15px]">{{ fmtInt(r.quantity) }}</td>
            <td class="num text-[12.5px] text-mute">{{ fmtTime(r.last_updated) }}</td>
          </tr>
          <tr v-if="!rows.length">
            <td colspan="4" class="py-10 text-center text-mute">{{ stock.inventory.length ? '沒有符合條件的庫存' : '目前沒有任何庫存，請先入庫' }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <div class="flex justify-end gap-6 border-t border-line bg-surface/60 px-4 py-2 text-[13px]">
      <span>共 <span class="num">{{ fmtInt(rows.length) }}</span> 筆</span>
      <span>合計 <span class="num">{{ fmtInt(totalQty) }}</span> 件</span>
    </div>
  </div>
</template>
