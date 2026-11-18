<script setup>
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import ProductPicker from '../components/ProductPicker.vue'
import ComboField from '../components/ComboField.vue'
import QtyHint from '../components/QtyHint.vue'
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
const location = ref('') // 放到哪個地點；可以留空（未指定）
const qtyText = ref('')
const note = ref('')
const busy = ref(false)
const picker = ref(null)
const qtyInput = ref(null)

const qty = computed(() => parseQty(qtyText.value))
const loc = computed(() => location.value.trim())
const productId = computed(() => (selected.value && !selected.value.isNew ? selected.value.id : ''))
// 這個品項目前有庫存的地點（數量多的在前）
const rows = computed(() => (productId.value ? stock.rowsOfProduct(productId.value) : []))
const before = computed(() => (productId.value ? stock.qty(productId.value, loc.value) : 0))
const problem = computed(() => (loc.value.length > 50 ? '存放地點請在 50 個字以內' : ''))
const canSubmit = computed(() => !busy.value && !project.readOnly && !!selected.value && !!qty.value && !problem.value)
const toneStyle = { '--tone': 'var(--color-go)', '--tone-fg': 'var(--color-on-go)' }

// 預設數量：第一次入庫的數量
function autoQty() {
  const p = products.get(productId.value)
  return p?.first_qty > 0 ? String(p.first_qty) : ''
}
const qtyNote = computed(() => (productId.value && products.get(productId.value)?.first_qty > 0 ? '已帶入第一次入庫的數量' : ''))

function onSelect(sel) {
  selected.value = sel
  qtyText.value = autoQty()
}

function onClear() {
  selected.value = null
  qtyText.value = ''
}

// 右邊「目前存放位置」點一下：填進存放地點
function pickPlace(l) {
  location.value = l
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
    const where = loc.value || stock.usesLocations ? `（${locLabel(loc.value)}）` : ''
    ui.ok(`已入庫：${name} ${fmtInt(qty.value)} 件${where}`)
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

// 從別的頁面帶著品項過來：ui.go('inbound', { productId })，直接選好品項
async function applyParams(p) {
  const item = p?.productId ? products.get(p.productId) : null
  if (!item) return
  onSelect({ id: item.id, name: item.name, isNew: false })
  await nextTick()
  qtyInput.value?.focus()
  qtyInput.value?.select()
}
onMounted(() => applyParams(ui.params))
// 已經在入庫頁時又被帶品項過來（同一頁不會重新載入，所以另外接）
watch(
  () => ui.params,
  (p) => {
    if (ui.page === 'inbound') applyParams(p)
  },
)
</script>

<template>
  <PageHeader title="入庫" hint="選擇品項或輸入新品項名稱 → 確認數量 → 按 Enter 或「確認入庫」；要把現有庫存換個地點放，請到「出庫/損壞」選「改變地點」" />

  <div class="grid max-w-5xl grid-cols-5 gap-6">
    <form class="panel col-span-3 space-y-6 p-7" :style="toneStyle" @submit.prevent="submit">
      <ProductPicker ref="picker" :selected="selected" allow-create @select="onSelect" @clear="onClear" />

      <div>
        <label class="field-label" for="in-loc">存放地點 <span class="font-normal text-mute">（可不填）</span></label>
        <ComboField
          id="in-loc"
          v-model="location"
          :suggestions="stock.knownLocations"
          title="已使用過的存放地點"
          maxlength="50"
          placeholder="例如：後倉第二排、A 櫃"
        />
        <p v-if="problem" class="mt-1.5 pl-1 text-[12.5px] text-signal">{{ problem }}</p>
      </div>

      <div class="grid grid-cols-2 gap-4">
        <div>
          <label class="field-label" for="in-qty">數量</label>
          <input
            id="in-qty"
            ref="qtyInput"
            v-model="qtyText"
            class="field num h-12 text-lg"
            placeholder="可輸入算式，如 12*5"
          />
          <QtyHint :text="qtyText" class="mt-1 pl-1">
            <template v-if="qtyNote" #default>
              <p class="text-[12.5px] text-mute">{{ qtyNote }}</p>
            </template>
          </QtyHint>
        </div>
        <div>
          <label class="field-label" for="in-note">備註</label>
          <input id="in-note" v-model="note" class="field h-12" placeholder="可不填" />
        </div>
      </div>

      <button type="submit" class="btn-go w-full py-3.5 text-[15.5px]" :disabled="!canSubmit">確認入庫</button>
    </form>

    <aside class="col-span-2 space-y-4" :style="toneStyle">
      <!-- 這個品項不分地點的總數，入庫時一直看得到；有用存放地點的專案再列出各地點各有多少 -->
      <div v-if="productId" class="panel px-6 py-4">
        <div class="flex items-baseline gap-2">
          <span class="text-[13px] font-semibold text-ink-2">這個品項全部共</span>
          <span class="num text-[28px] leading-none">{{ fmtInt(stock.total(productId)) }}</span>
          <span class="text-[13px] text-mute">件</span>
        </div>
        <div v-if="stock.usesLocations && rows.length" class="mt-3 flex flex-wrap gap-1.5">
          <span v-for="r in rows" :key="r.location_id" class="chip">
            <span class="min-w-0 truncate" :class="r.location_id ? '' : 'italic'">{{ locLabel(r.location_id) }}</span>
            <span class="chip-num">{{ fmtInt(r.quantity) }}</span>
          </span>
        </div>
      </div>

      <!-- 放進去的那個地點會變成多少 -->
      <QtyPreview
        v-if="selected"
        :before="before"
        :after="qty ? before + qty : null"
        :caption="loc || stock.usesLocations ? `${locLabel(loc)} 現有` : '現有'"
        after-label="入庫後"
        unit="件"
        tone="go"
        delta
      />

      <div v-if="productId && stock.usesLocations" class="panel p-5">
        <h3 class="mb-2 text-[13px] font-semibold text-mute">
          此品項目前存放位置<span v-if="rows.length" class="font-normal">（點一下填入存放地點）</span>
        </h3>
        <ul v-if="rows.length" class="space-y-1.5">
          <li v-for="r in rows" :key="r.location_id">
            <button
              type="button"
              class="place-row flex w-full items-center justify-between gap-3 rounded-full px-3 py-1.5 text-left hover:bg-hover"
              :class="loc === r.location_id ? 'is-on font-semibold' : ''"
              @click="pickPlace(r.location_id)"
            >
              <span class="min-w-0 truncate" :class="r.location_id ? '' : 'italic'">{{ locLabel(r.location_id) }}</span>
              <span class="num shrink-0">{{ fmtInt(r.quantity) }}</span>
            </button>
          </li>
        </ul>
        <p v-else class="text-[13px] text-mute">目前沒有庫存</p>
      </div>
    </aside>
  </div>
</template>

<style scoped>
/* 右邊「目前存放位置」：現在要放進去的那個地點淡淡標出來 */
.place-row.is-on {
  background: color-mix(in oklab, var(--tone) 15%, transparent);
}
</style>
