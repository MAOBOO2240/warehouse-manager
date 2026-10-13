<script setup>
import { computed, ref } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import { useProductStore } from '../stores/products'
import { useLocationStore } from '../stores/locations'
import { useStockStore } from '../stores/stock'
import { useUiStore } from '../stores/ui'
import { useProjectStore } from '../stores/project'
import { fmtInt, fmtMoney, fmtTime, todayStamp } from '../utils/format'
import { saveCsv } from '../utils/csv'

const products = useProductStore()
const locations = useLocationStore()
const stock = useStockStore()
const ui = useUiStore()
const project = useProjectStore()

const q = ref('')
const zone = ref('')
const zones = computed(() => [...new Set(locations.items.map((l) => l.zone).filter(Boolean))].sort())

const rows = computed(() => {
  const k = q.value.trim().toLowerCase()
  return stock.inventory
    .filter((r) => r.quantity > 0)
    .map((r) => ({ ...r, product: products.get(r.product_id) ?? { name: r.product_id }, location: locations.get(r.location_id) }))
    .filter((r) => !zone.value || r.location?.zone === zone.value)
    .filter(
      (r) =>
        !k ||
        [r.product.name, r.product.sku, r.product.barcode, r.product_id, r.location_id].some((v) =>
          String(v ?? '').toLowerCase().includes(k),
        ),
    )
    .sort((a, b) => a.location_id.localeCompare(b.location_id, 'zh-Hant', { numeric: true }) || a.product.name.localeCompare(b.product.name, 'zh-Hant'))
})

const totals = computed(() =>
  rows.value.reduce(
    (t, r) => ({
      qty: t.qty + r.quantity,
      cost: t.cost + r.quantity * (Number(r.product.cost_price) || 0),
    }),
    { qty: 0, cost: 0 },
  ),
)

async function exportCsv() {
  try {
    const path = await saveCsv(
      `${project.current}_即時庫存清冊_${todayStamp()}.csv`,
      ['商品編號', 'SKU', '條碼', '商品名稱', '庫位代碼', '當前數量', '單位', '成本價', '售價'],
      rows.value.map((r) => [
        r.product_id, r.product.sku, r.product.barcode, r.product.name, r.location_id,
        r.quantity, r.product.unit, r.product.cost_price, r.product.sale_price,
      ]),
    )
    if (path) ui.ok(`已匯出 ${rows.value.length} 筆庫存：\n${path}`)
  } catch (err) {
    ui.error(err)
  }
}
</script>

<template>
  <PageHeader title="庫存清冊" hint="各商品在各庫位的即時數量">
    <button type="button" class="btn-primary" :disabled="!rows.length" @click="exportCsv">匯出 CSV</button>
  </PageHeader>

  <div class="mb-4 flex gap-3">
    <input v-model="q" class="scan-field max-w-md" placeholder="搜尋商品名稱、條碼、SKU、庫位" />
    <select v-model="zone" class="field w-44">
      <option value="">全部倉區</option>
      <option v-for="z in zones" :key="z" :value="z">{{ z }}</option>
    </select>
  </div>

  <div class="panel overflow-hidden">
    <div class="max-h-[calc(100vh-17rem)] overflow-auto">
      <table class="table-base">
        <thead>
          <tr>
            <th>庫位</th>
            <th>商品名稱</th>
            <th>SKU</th>
            <th>條碼</th>
            <th class="text-right">數量</th>
            <th class="text-right">成本價</th>
            <th class="text-right">售價</th>
            <th>最後異動</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in rows" :key="`${r.product_id}|${r.location_id}`">
            <td class="num font-semibold">{{ r.location_id }}</td>
            <td>{{ r.product.name }}</td>
            <td class="num text-mute">{{ r.product.sku }}</td>
            <td class="num text-mute">{{ r.product.barcode }}</td>
            <td class="num text-right text-[15px]">{{ fmtInt(r.quantity) }} <span class="font-sans text-[12px] text-mute">{{ r.product.unit }}</span></td>
            <td class="num text-right">{{ fmtMoney(r.product.cost_price) }}</td>
            <td class="num text-right">{{ fmtMoney(r.product.sale_price) }}</td>
            <td class="num text-[12.5px] text-mute">{{ fmtTime(r.last_updated) }}</td>
          </tr>
          <tr v-if="!rows.length">
            <td colspan="8" class="py-10 text-center text-mute">{{ stock.inventory.length ? '沒有符合條件的庫存' : '目前沒有任何庫存，請先進行入庫' }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <div class="flex justify-end gap-6 border-t border-line bg-surface/60 px-4 py-2 text-[13px]">
      <span>共 <span class="num">{{ fmtInt(rows.length) }}</span> 筆</span>
      <span>合計 <span class="num">{{ fmtInt(totals.qty) }}</span> 件</span>
      <span>成本合計 <span class="num">{{ fmtMoney(totals.cost) }}</span> 元</span>
    </div>
  </div>
</template>
