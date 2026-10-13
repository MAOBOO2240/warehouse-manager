<script setup>
import { computed, nextTick, ref } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import ProductPicker from '../components/ProductPicker.vue'
import ModalDialog from '../components/ModalDialog.vue'
import { useProductStore } from '../stores/products'
import { useLocationStore } from '../stores/locations'
import { useStockStore } from '../stores/stock'
import { useUiStore } from '../stores/ui'
import { useProjectStore } from '../stores/project'
import { fmtDelta, fmtInt, parseCount } from '../utils/format'

const products = useProductStore()
const locations = useLocationStore()
const stock = useStockStore()
const ui = useUiStore()
const project = useProjectStore()

const locationId = ref('')
const rows = ref([]) // { product_id, actualText }
const scanId = ref('')
const note = ref('')
const picker = ref(null)
const inputs = {}
const confirming = ref(false)
const busy = ref(false)

function loadSheet() {
  rows.value = stock
    .rowsOfLocation(locationId.value)
    .map((r) => ({ product_id: r.product_id, actualText: '' }))
    .sort((a, b) => (products.get(a.product_id)?.name || '').localeCompare(products.get(b.product_id)?.name || '', 'zh-Hant'))
}

const sheet = computed(() =>
  rows.value.map((r) => {
    const book = stock.qty(r.product_id, locationId.value)
    const actual = parseCount(r.actualText)
    const invalid = r.actualText.trim() !== '' && actual === null
    return { ...r, product: products.get(r.product_id), book, actual, invalid, diff: actual === null ? null : actual - book }
  }),
)
const counted = computed(() => sheet.value.filter((r) => r.actual !== null))
const changes = computed(() => counted.value.filter((r) => r.diff !== 0))
const profit = computed(() => changes.value.filter((r) => r.diff > 0))
const loss = computed(() => changes.value.filter((r) => r.diff < 0))
const hasInvalid = computed(() => sheet.value.some((r) => r.invalid))

async function focusRow(pid) {
  await nextTick()
  inputs[pid]?.focus()
  inputs[pid]?.select()
}

// 掃描商品：已在清冊就跳到該列；不在就加入（帳面量 0，盤到的話就是盤盈）
function onScan(p) {
  scanId.value = ''
  if (!rows.value.some((r) => r.product_id === p.id)) {
    rows.value.push({ product_id: p.id, actualText: '' })
  }
  focusRow(p.id)
}

async function nextScan() {
  await nextTick()
  picker.value?.focus()
}

function fillBook() {
  for (const r of rows.value) {
    if (r.actualText.trim() === '') r.actualText = String(stock.qty(r.product_id, locationId.value))
  }
}

async function commit() {
  busy.value = true
  try {
    const logs = await stock.apply({
      kind: 'STOCKTAKE',
      location_id: locationId.value,
      items: changes.value.map((r) => ({ product_id: r.product_id, actual: r.actual })),
      note: note.value,
    })
    ui.ok(`盤點平倉完成：${locationId.value} 共調整 ${logs.length} 項`)
    confirming.value = false
    note.value = ''
    loadSheet()
  } catch (err) {
    ui.error(err)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <PageHeader title="盤點平倉" hint="選擇庫位 → 輸入實盤數量 → 系統試算盈虧 → 確認後帳面校正為實盤數" />

  <div class="mb-4 flex items-end gap-4">
    <div class="w-80">
      <label class="field-label" for="st-loc">盤點庫位</label>
      <select id="st-loc" v-model="locationId" class="field num" @change="loadSheet">
        <option value="" disabled>請選擇庫位</option>
        <option v-for="l in locations.sorted" :key="l.id" :value="l.id">{{ locations.label(l.id) }}</option>
      </select>
    </div>
    <div v-if="locationId" class="w-[28rem]">
      <ProductPicker
        ref="picker"
        v-model="scanId"
        label="掃描商品以定位或加入清冊"
        :show-stock="false"
        placeholder="掃描條碼 → 輸入實盤數 → Enter → 下一件"
        @picked="onScan"
      />
    </div>
  </div>

  <div v-if="!locationId" class="panel px-5 py-10 text-center text-mute">請先選擇要盤點的庫位</div>

  <template v-else>
    <div class="panel overflow-hidden">
      <div class="max-h-[calc(100vh-22rem)] overflow-auto">
        <table class="table-base">
          <thead>
            <tr>
              <th>商品</th>
              <th>條碼</th>
              <th class="text-right">帳面數量</th>
              <th class="w-40">實盤數量</th>
              <th class="text-right">差異</th>
              <th>結果</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(r, i) in sheet" :key="r.product_id">
              <td class="font-semibold">{{ r.product?.name ?? r.product_id }}</td>
              <td class="num text-mute">{{ r.product?.barcode }}</td>
              <td class="num text-right text-[15px]">{{ fmtInt(r.book) }}</td>
              <td>
                <input
                  :ref="(el) => (inputs[r.product_id] = el)"
                  v-model="rows[i].actualText"
                  class="field num py-1.5"
                  :class="r.invalid ? 'border-signal' : ''"
                  inputmode="numeric"
                  placeholder="未盤"
                  @keydown.enter.prevent="nextScan"
                />
              </td>
              <td class="num text-right text-[15px]" :class="r.diff > 0 ? 'text-go' : r.diff < 0 ? 'text-signal' : 'text-mute'">
                {{ r.diff === null ? '' : fmtDelta(r.diff) }}
              </td>
              <td class="text-[13px]">
                <span v-if="r.invalid" class="text-signal">請輸入 0 以上整數</span>
                <span v-else-if="r.diff > 0" class="font-semibold text-go">盤盈</span>
                <span v-else-if="r.diff < 0" class="font-semibold text-signal">盤虧</span>
                <span v-else-if="r.diff === 0" class="text-mute">相符</span>
              </td>
            </tr>
            <tr v-if="!sheet.length">
              <td colspan="6" class="py-10 text-center text-mute">此庫位帳面上沒有任何商品；若實際有貨，請掃描商品加入清冊</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <div class="mt-4 flex items-center gap-4">
      <div class="text-[14px]">
        已盤 <span class="num">{{ counted.length }}</span> / <span class="num">{{ sheet.length }}</span> 項・
        盤盈 <span class="num text-go">{{ profit.length }}</span> 項・
        盤虧 <span class="num text-signal">{{ loss.length }}</span> 項
      </div>
      <button type="button" class="btn-ghost btn-sm" :disabled="!sheet.length" @click="fillBook">未盤項目以帳面數填入</button>
      <input v-model="note" class="field ml-auto w-72" placeholder="盤點備註（盤點人、時間等）" />
      <button
        type="button"
        class="btn-primary"
        :disabled="!changes.length || hasInvalid || project.readOnly"
        @click="confirming = true"
      >確認平倉調整</button>
    </div>
    <p class="mt-2 text-[12.5px] text-mute">未輸入實盤數量的品項不會調整。</p>
  </template>

  <ModalDialog v-if="confirming" title="確認平倉調整" width="max-w-xl" @close="confirming = false">
    <p class="mb-3">以下 <strong>{{ changes.length }}</strong> 項的帳面數量將強制校正為實盤數，並寫入流水帳：</p>
    <ul class="divide-y divide-line rounded-md border border-line">
      <li v-for="r in changes" :key="r.product_id" class="flex items-center justify-between px-3 py-2">
        <span>{{ r.product?.name }}</span>
        <span class="num">
          {{ fmtInt(r.book) }} → {{ fmtInt(r.actual) }}
          <span class="ml-2" :class="r.diff > 0 ? 'text-go' : 'text-signal'">{{ fmtDelta(r.diff) }}</span>
        </span>
      </li>
    </ul>
    <template #footer>
      <button type="button" class="btn-ghost" @click="confirming = false">取消</button>
      <button type="button" class="btn-primary" :disabled="busy" @click="commit">確認平倉</button>
    </template>
  </ModalDialog>
</template>
