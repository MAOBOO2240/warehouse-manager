<script setup>
import { computed, ref } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import ProductPicker from '../components/ProductPicker.vue'
import QtyPreview from '../components/QtyPreview.vue'
import { useProductStore } from '../stores/products'
import { useStockStore } from '../stores/stock'
import { useUiStore } from '../stores/ui'
import { useProjectStore } from '../stores/project'
import { fmtInt, locLabel, parseQty } from '../utils/format'

const products = useProductStore()
const stock = useStockStore()
const ui = useUiStore()
const project = useProjectStore()

const selected = ref(null)
const location = ref('')
const qtyText = ref('')
const note = ref('')
const busy = ref(false)
const picker = ref(null)
const qtyInput = ref(null)

const qty = computed(() => parseQty(qtyText.value))
const loc = computed(() => location.value.trim())
const productId = computed(() => (selected.value && !selected.value.isNew ? selected.value.id : ''))
const before = computed(() => (productId.value ? stock.qty(productId.value, loc.value) : 0))
const locTooLong = computed(() => loc.value.length > 50)
const canSubmit = computed(() => !busy.value && !project.readOnly && selected.value && qty.value && !locTooLong.value)

async function onSelect(sel) {
  selected.value = sel
  if (!sel.isNew) {
    const p = products.get(sel.id)
    // 自動帶入第一次入庫的數量
    qtyText.value = p?.first_qty > 0 ? String(p.first_qty) : ''
  } else {
    qtyText.value = ''
  }
}

function onClear() {
  selected.value = null
  qtyText.value = ''
}

async function submit() {
  if (!canSubmit.value) return
  busy.value = true
  try {
    let id = selected.value.id
    let name = selected.value.name
    if (selected.value.isNew) {
      const created = await products.create(name)
      id = created.id
      name = created.name
    }
    await stock.apply({
      kind: 'INBOUND',
      product_id: id,
      location_id: loc.value,
      quantity: qty.value,
      reason: '入庫',
      note: note.value,
    })
    ui.ok(`已入庫：${name} ${fmtInt(qty.value)} 件（${locLabel(loc.value)}）`)
    selected.value = null
    location.value = ''
    qtyText.value = ''
    note.value = ''
  } catch (err) {
    ui.error(err)
  } finally {
    busy.value = false
  }
}

</script>

<template>
  <PageHeader title="入庫" hint="選擇品項或輸入新品項名稱 → 確認數量 → 按 Enter 或「確認入庫」" />

  <div class="grid max-w-5xl grid-cols-5 gap-6">
    <form class="panel col-span-3 space-y-6 p-7" style="--tone: var(--color-go)" @submit.prevent="submit">
      <ProductPicker ref="picker" :selected="selected" allow-create @select="onSelect" @clear="onClear" />

      <div>
        <label class="field-label" for="in-loc">存放地點 <span class="font-normal text-mute">（可不填）</span></label>
        <input
          id="in-loc"
          v-model="location"
          class="field"
          list="in-locs"
          maxlength="50"
          placeholder="例如：後倉第二排、A 櫃"
          autocomplete="off"
        />
        <datalist id="in-locs">
          <option v-for="l in stock.knownLocations" :key="l" :value="l" />
        </datalist>
      </div>

      <div class="grid grid-cols-2 gap-4">
        <div>
          <label class="field-label" for="in-qty">數量</label>
          <input id="in-qty" ref="qtyInput" v-model="qtyText" class="field num text-lg" inputmode="numeric" placeholder="0" />
          <p v-if="qtyText && !qty" class="mt-1 text-[12.5px] text-signal">請輸入大於 0 的整數</p>
          <p v-else-if="productId && products.get(productId)?.first_qty > 0" class="mt-1 text-[12.5px] text-mute">
            已帶入第一次入庫的數量
          </p>
        </div>
        <div>
          <label class="field-label" for="in-note">備註</label>
          <input id="in-note" v-model="note" class="field" placeholder="可不填" />
        </div>
      </div>

      <button type="submit" class="btn-go w-full py-3.5 text-[15.5px]" :disabled="!canSubmit">確認入庫</button>
    </form>

    <aside class="col-span-2 space-y-4">
      <QtyPreview
        v-if="selected"
        :before="before"
        :after="qty ? before + qty : null"
        :caption="`${locLabel(loc)} 現有`"
        unit="件"
      />
      <div v-if="productId" class="panel p-5">
        <h3 class="mb-2 text-[13px] font-semibold text-mute">此品項目前存放位置</h3>
        <ul v-if="stock.rowsOfProduct(productId).length" class="space-y-1.5">
          <li v-for="r in stock.rowsOfProduct(productId)" :key="r.location_id">
            <button
              type="button"
              class="flex w-full justify-between rounded-full px-3 py-1.5 text-left hover:bg-hover"
              @click="location = r.location_id"
            >
              <span>{{ locLabel(r.location_id) }}</span>
              <span class="num">{{ fmtInt(r.quantity) }}</span>
            </button>
          </li>
        </ul>
        <p v-else class="text-[13px] text-mute">目前沒有庫存</p>
      </div>
    </aside>
  </div>
</template>
