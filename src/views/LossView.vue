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

const MODES = {
  DAMAGE: {
    label: '物品損壞',
    desc: '商品在倉內破損、壓壞、受潮等，自庫位扣除',
    reasons: ['搬運摔破', '外箱壓損', '受潮', '包裝破損'],
    verb: '登記損壞',
  },
  SCRAP: {
    label: '報廢損耗',
    desc: '商品過期、遺失或無法再販售，自庫位沖銷',
    reasons: ['過期', '遺失', '品質不良', '召回下架'],
    verb: '報廢',
  },
}

const mode = ref('DAMAGE')
const productId = ref('')
const locationId = ref('')
const qtyText = ref('')
const reason = ref('')
const note = ref('')
const busy = ref(false)
const picker = ref(null)
const qtyInput = ref(null)

const m = computed(() => MODES[mode.value])
const product = computed(() => products.get(productId.value))
const qty = computed(() => parseQty(qtyText.value))
const available = computed(() => (productId.value && locationId.value ? stock.qty(productId.value, locationId.value) : 0))
const insufficient = computed(() => !!qty.value && !!locationId.value && qty.value > available.value)
const canSubmit = computed(
  () =>
    !busy.value && !project.readOnly && product.value && locationId.value && qty.value &&
    !insufficient.value && reason.value.trim() && note.value.trim(),
)

watch(productId, () => {
  locationId.value = ''
})

async function onPicked(p) {
  await nextTick()
  const rows = stock.rowsOfProduct(p.id)
  if (rows.length === 1) locationId.value = rows[0].location_id
  qtyInput.value?.focus()
}

async function submit() {
  if (!canSubmit.value) return
  busy.value = true
  try {
    await stock.apply({
      kind: mode.value,
      product_id: productId.value,
      location_id: locationId.value,
      quantity: qty.value,
      reason: reason.value,
      note: note.value,
    })
    ui.ok(`已${m.value.verb}：${product.value.name} ${fmtInt(qty.value)} ${product.value.unit || '件'}（${locationId.value}）`)
    productId.value = ''
    qtyText.value = ''
    reason.value = ''
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
  <PageHeader title="損壞與報廢" hint="自指定庫位扣除，原因與責任備註為必填" />

  <div class="mb-4 inline-flex rounded-md border border-line bg-white p-1" role="tablist">
    <button
      v-for="(info, key) in MODES"
      :key="key"
      type="button"
      role="tab"
      :aria-selected="mode === key"
      class="rounded px-5 py-1.5 text-[14px] font-semibold transition-colors"
      :class="mode === key ? 'bg-signal text-white' : 'text-ink-3 hover:bg-surface'"
      @click="mode = key; reason = ''"
    >{{ info.label }}</button>
  </div>
  <p class="mb-4 text-[13px] text-mute">{{ m.desc }}</p>

  <div class="grid max-w-5xl grid-cols-5 gap-6">
    <form class="panel col-span-3 space-y-4 p-5" @submit.prevent="submit">
      <ProductPicker ref="picker" v-model="productId" @picked="onPicked" />
      <LocationStockList v-model="locationId" :product-id="productId" :unit="product?.unit" label="扣除庫位" />

      <div>
        <label class="field-label" for="loss-qty">數量</label>
        <input
          id="loss-qty"
          ref="qtyInput"
          v-model="qtyText"
          class="field num w-48 text-lg"
          :class="insufficient ? 'border-signal bg-signal-soft' : ''"
          inputmode="numeric"
          placeholder="0"
        />
      </div>
      <div v-if="insufficient" class="rounded-md border-2 border-signal bg-signal-soft px-4 py-3 font-bold text-signal" role="alert">
        庫存不足！當前庫位剩餘 {{ fmtInt(available) }} 件，不可{{ m.verb }} {{ fmtInt(qty) }} 件
      </div>

      <div>
        <label class="field-label" for="loss-reason">{{ mode === 'DAMAGE' ? '損壞原因' : '報廢原因' }} *</label>
        <div class="mb-2 flex flex-wrap gap-2">
          <button
            v-for="r in m.reasons"
            :key="r"
            type="button"
            class="rounded-full border px-3 py-1 text-[13px]"
            :class="reason === r ? 'border-ink bg-ink text-white' : 'border-line bg-white hover:border-ink-3'"
            @click="reason = r"
          >{{ r }}</button>
        </div>
        <input id="loss-reason" v-model="reason" class="field" placeholder="點選常用原因或自行輸入" />
      </div>
      <div>
        <label class="field-label" for="loss-note">責任備註 *</label>
        <input id="loss-note" v-model="note" class="field" placeholder="例如：發現人、責任歸屬、處理方式" />
      </div>

      <button type="submit" class="btn-danger w-full py-3 text-[15px]" :disabled="!canSubmit">確認{{ m.verb }}</button>
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
