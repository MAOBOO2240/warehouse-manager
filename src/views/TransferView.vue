<script setup>
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import ProductPicker from '../components/ProductPicker.vue'
import LocationStockList from '../components/LocationStockList.vue'
import { useProductStore } from '../stores/products'
import { useLocationStore } from '../stores/locations'
import { useStockStore } from '../stores/stock'
import { useUiStore } from '../stores/ui'
import { useProjectStore } from '../stores/project'
import { fmtInt, parseQty } from '../utils/format'

const products = useProductStore()
const locations = useLocationStore()
const stock = useStockStore()
const ui = useUiStore()
const project = useProjectStore()

const productId = ref('')
const fromId = ref('')
const toId = ref('')
const qtyText = ref('')
const note = ref('')
const busy = ref(false)
const picker = ref(null)
const qtyInput = ref(null)

const product = computed(() => products.get(productId.value))
const qty = computed(() => parseQty(qtyText.value))
const fromQty = computed(() => (productId.value && fromId.value ? stock.qty(productId.value, fromId.value) : 0))
const toQty = computed(() => (productId.value && toId.value ? stock.qty(productId.value, toId.value) : 0))
const insufficient = computed(() => !!qty.value && !!fromId.value && qty.value > fromQty.value)
const sameLoc = computed(() => fromId.value && fromId.value === toId.value)
const targets = computed(() => locations.sorted.filter((l) => l.id !== fromId.value))
const canSubmit = computed(
  () =>
    !busy.value && !project.readOnly && product.value && fromId.value && toId.value &&
    !sameLoc.value && qty.value && !insufficient.value,
)

watch(productId, () => {
  fromId.value = ''
})
watch(fromId, (v) => {
  if (v && v === toId.value) toId.value = ''
})

async function onPicked(p) {
  await nextTick()
  const rows = stock.rowsOfProduct(p.id)
  if (rows.length === 1) fromId.value = rows[0].location_id
}

async function submit() {
  if (!canSubmit.value) return
  busy.value = true
  try {
    await stock.apply({
      kind: 'TRANSFER',
      product_id: productId.value,
      from_location_id: fromId.value,
      to_location_id: toId.value,
      quantity: qty.value,
      note: note.value,
    })
    ui.ok(`已調撥：${product.value.name} ${fmtInt(qty.value)} ${product.value.unit || '件'}，${fromId.value} → ${toId.value}`)
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
  <PageHeader title="庫位調撥" hint="把商品從一個庫位移到另一個庫位，兩端庫存同時更新" />

  <form class="panel max-w-4xl space-y-4 p-5" @submit.prevent="submit">
    <ProductPicker ref="picker" v-model="productId" @picked="onPicked" />

    <div class="grid grid-cols-2 gap-5">
      <LocationStockList v-model="fromId" :product-id="productId" :unit="product?.unit" label="來源庫位" />
      <div>
        <label class="field-label" for="tr-to">目的庫位</label>
        <select id="tr-to" v-model="toId" class="field num">
          <option value="" disabled>請選擇目的庫位</option>
          <option v-for="l in targets" :key="l.id" :value="l.id">{{ locations.label(l.id) }}</option>
        </select>
        <p v-if="toId && productId" class="mt-1.5 text-[13px] text-mute">目的庫位現有 <span class="num">{{ fmtInt(toQty) }}</span> {{ product?.unit }}</p>
      </div>
    </div>

    <div class="grid grid-cols-2 gap-5">
      <div>
        <label class="field-label" for="tr-qty">調撥數量</label>
        <input
          id="tr-qty"
          ref="qtyInput"
          v-model="qtyText"
          class="field num text-lg"
          :class="insufficient ? 'border-signal bg-signal-soft' : ''"
          inputmode="numeric"
          placeholder="0"
        />
      </div>
      <div>
        <label class="field-label" for="tr-note">備註</label>
        <input id="tr-note" v-model="note" class="field" placeholder="例如：整理貨架" />
      </div>
    </div>

    <div v-if="insufficient" class="rounded-md border-2 border-signal bg-signal-soft px-4 py-3 font-bold text-signal" role="alert">
      庫存不足！來源庫位剩餘 {{ fmtInt(fromQty) }} 件，不可調撥 {{ fmtInt(qty) }} 件
    </div>

    <div v-if="product && fromId && toId && qty && !insufficient" class="grid grid-cols-2 gap-4 text-[14px]">
      <div class="rounded-md bg-surface px-4 py-3">
        <span class="num">{{ fromId }}</span>：<span class="num">{{ fmtInt(fromQty) }} → {{ fmtInt(fromQty - qty) }}</span>
      </div>
      <div class="rounded-md bg-surface px-4 py-3">
        <span class="num">{{ toId }}</span>：<span class="num">{{ fmtInt(toQty) }} → {{ fmtInt(toQty + qty) }}</span>
      </div>
    </div>

    <button type="submit" class="btn-primary w-full py-3 text-[15px]" :disabled="!canSubmit">確認調撥</button>
  </form>
</template>
