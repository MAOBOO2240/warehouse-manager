<script setup>
import { computed, nextTick, onMounted, ref } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import ProductPicker from '../components/ProductPicker.vue'
import QtyPreview from '../components/QtyPreview.vue'
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

const productId = ref(ui.params.productId || '')
const locationId = ref('')
const qtyText = ref('')
const reason = ref('進貨入庫')
const note = ref('')
const busy = ref(false)
const picker = ref(null)
const qtyInput = ref(null)

const product = computed(() => products.get(productId.value))
const qty = computed(() => parseQty(qtyText.value))
const before = computed(() => (productId.value && locationId.value ? stock.qty(productId.value, locationId.value) : 0))
const canSubmit = computed(() => !busy.value && !project.readOnly && product.value && locationId.value && qty.value)

// 選好商品後：若該商品已有存放庫位，預設帶入最多的那一格，游標跳到數量
async function onPicked(p) {
  const rows = stock.rowsOfProduct(p.id)
  if (rows.length) locationId.value = rows[0].location_id
  await nextTick()
  qtyInput.value?.focus()
}

async function submit() {
  if (!canSubmit.value) return
  busy.value = true
  try {
    await stock.apply({
      kind: 'INBOUND',
      product_id: productId.value,
      location_id: locationId.value,
      quantity: qty.value,
      reason: reason.value,
      note: note.value,
    })
    ui.ok(`已入庫：${product.value.name} ${fmtInt(qty.value)} ${product.value.unit || '件'} → ${locationId.value}`)
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

onMounted(async () => {
  if (productId.value && product.value) await onPicked(product.value)
  else picker.value?.focus()
})
</script>

<template>
  <PageHeader title="入庫作業" hint="掃描商品條碼 → 選擇入庫庫位 → 輸入數量後按 Enter 送出" />

  <div v-if="!locations.items.length" class="panel mb-4 border-safety bg-safety-soft px-5 py-3 text-[14px]">
    尚未建立庫位，請先到
    <button type="button" class="font-semibold underline" @click="ui.go('locations')">庫位主檔</button>
    新增至少一個庫位。
  </div>

  <div class="grid max-w-5xl grid-cols-5 gap-6">
    <form class="panel col-span-3 space-y-4 p-5" @submit.prevent="submit">
      <ProductPicker ref="picker" v-model="productId" @picked="onPicked" />

      <div>
        <label class="field-label" for="in-loc">入庫庫位</label>
        <select id="in-loc" v-model="locationId" class="field num">
          <option value="" disabled>請選擇庫位</option>
          <option v-for="l in locations.sorted" :key="l.id" :value="l.id">{{ locations.label(l.id) }}</option>
        </select>
      </div>

      <div class="grid grid-cols-2 gap-4">
        <div>
          <label class="field-label" for="in-qty">入庫數量</label>
          <input id="in-qty" ref="qtyInput" v-model="qtyText" class="field num text-lg" inputmode="numeric" placeholder="0" />
          <p v-if="qtyText && !qty" class="mt-1 text-[12.5px] text-signal">請輸入大於 0 的整數</p>
        </div>
        <div>
          <label class="field-label" for="in-reason">入庫原因</label>
          <input id="in-reason" v-model="reason" class="field" list="in-reasons" />
          <datalist id="in-reasons">
            <option value="進貨入庫" />
            <option value="門市退回" />
            <option value="其他入庫" />
          </datalist>
        </div>
      </div>

      <div>
        <label class="field-label" for="in-note">採購備註</label>
        <input id="in-note" v-model="note" class="field" placeholder="例如：進貨單號、供應商" />
      </div>

      <button type="submit" class="btn-go w-full py-3 text-[15px]" :disabled="!canSubmit">確認入庫</button>
    </form>

    <aside class="col-span-2 space-y-4">
      <QtyPreview
        v-if="product && locationId"
        :before="before"
        :after="qty ? before + qty : null"
        :unit="product.unit"
      />
      <div v-if="product" class="panel p-4">
        <h3 class="mb-2 text-[13px] font-semibold text-mute">此商品目前存放位置</h3>
        <ul v-if="stock.rowsOfProduct(product.id).length" class="space-y-1.5">
          <li v-for="r in stock.rowsOfProduct(product.id)" :key="r.location_id" class="flex justify-between">
            <span class="num">{{ r.location_id }}</span>
            <span class="num">{{ fmtInt(r.quantity) }}</span>
          </li>
        </ul>
        <p v-else class="text-[13px] text-mute">尚未存放在任何庫位</p>
      </div>
    </aside>
  </div>
</template>
