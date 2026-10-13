<script setup>
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import ProductPicker from '../components/ProductPicker.vue'
import LocationStockList from '../components/LocationStockList.vue'
import QtyPreview from '../components/QtyPreview.vue'
import { useProductStore } from '../stores/products'
import { useStockStore } from '../stores/stock'
import { useUiStore } from '../stores/ui'
import { useProjectStore } from '../stores/project'
import { fmtInt, parseQty } from '../utils/format'

const products = useProductStore()
const stock = useStockStore()
const ui = useUiStore()
const project = useProjectStore()

const productId = ref('')
const locationId = ref('')
const qtyText = ref('')
const reason = ref('門市零售出貨')
const note = ref('')
const busy = ref(false)
const picker = ref(null)
const qtyInput = ref(null)

const product = computed(() => products.get(productId.value))
const qty = computed(() => parseQty(qtyText.value))
const available = computed(() => (productId.value && locationId.value ? stock.qty(productId.value, locationId.value) : 0))
// 嚴格防呆：出庫數量不可大於該庫位現有庫存
const insufficient = computed(() => !!qty.value && !!locationId.value && qty.value > available.value)
const canSubmit = computed(
  () => !busy.value && !project.readOnly && product.value && locationId.value && qty.value && !insufficient.value,
)

watch(productId, () => {
  locationId.value = ''
})

async function onPicked(p) {
  const rows = stock.rowsOfProduct(p.id)
  await nextTick()
  if (rows.length) locationId.value = rows[0].location_id
  qtyInput.value?.focus()
}

async function submit() {
  if (!canSubmit.value) return
  busy.value = true
  try {
    await stock.apply({
      kind: 'OUTBOUND',
      product_id: productId.value,
      location_id: locationId.value,
      quantity: qty.value,
      reason: reason.value,
      note: note.value,
    })
    ui.ok(`已出庫：${product.value.name} ${fmtInt(qty.value)} ${product.value.unit || '件'}（${locationId.value}）`)
    productId.value = ''
    qtyText.value = ''
    note.value = ''
    await nextTick()
    picker.value?.focus()
  } catch (err) {
    ui.error(err)
  } finally {
    busy.value = false
  }
}

onMounted(() => picker.value?.focus())
</script>

<template>
  <PageHeader title="出庫作業" hint="出庫數量不可超過該庫位現有庫存，系統會在前端與後端雙重阻擋" />

  <div class="grid max-w-5xl grid-cols-5 gap-6">
    <form class="panel col-span-3 space-y-4 p-5" @submit.prevent="submit">
      <ProductPicker ref="picker" v-model="productId" @picked="onPicked" />
      <LocationStockList v-model="locationId" :product-id="productId" :unit="product?.unit" label="出庫庫位（顯示各庫位現存量）" />

      <div class="grid grid-cols-2 gap-4">
        <div>
          <label class="field-label" for="out-qty">出庫數量</label>
          <input
            id="out-qty"
            ref="qtyInput"
            v-model="qtyText"
            class="field num text-lg"
            :class="insufficient ? 'border-signal bg-signal-soft focus:border-signal focus:ring-signal/20' : ''"
            inputmode="numeric"
            placeholder="0"
          />
          <p v-if="qtyText && !qty" class="mt-1 text-[12.5px] text-signal">請輸入大於 0 的整數</p>
        </div>
        <div>
          <label class="field-label" for="out-reason">出庫原因</label>
          <input id="out-reason" v-model="reason" class="field" list="out-reasons" />
          <datalist id="out-reasons">
            <option value="門市零售出貨" />
            <option value="批發出貨" />
            <option value="門市補貨" />
            <option value="內部領用" />
          </datalist>
        </div>
      </div>

      <div
        v-if="insufficient"
        class="rounded-md border-2 border-signal bg-signal-soft px-4 py-3 text-[15px] font-bold text-signal"
        role="alert"
      >
        庫存不足！當前庫位剩餘 {{ fmtInt(available) }} 件，不可出庫 {{ fmtInt(qty) }} 件
      </div>

      <div>
        <label class="field-label" for="out-note">備註</label>
        <input id="out-note" v-model="note" class="field" placeholder="例如：收銀機單號 #8821" />
      </div>

      <button type="submit" class="btn-primary w-full py-3 text-[15px]" :disabled="!canSubmit">確認出庫</button>
    </form>

    <aside class="col-span-2">
      <QtyPreview
        v-if="product && locationId"
        :before="available"
        :after="qty && !insufficient ? available - qty : null"
        :unit="product.unit"
        :danger="insufficient"
      />
    </aside>
  </div>
</template>
