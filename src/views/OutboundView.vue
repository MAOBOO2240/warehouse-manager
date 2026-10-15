<script setup>
import { computed, nextTick, onMounted, ref } from 'vue'
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

const ACTIONS = [
  { id: 'SHIP', label: '出貨' },
  { id: 'DAMAGE', label: '損壞' },
]
const DAMAGE_REASONS = ['道具受損', '印刷錯誤', '其他']

const selected = ref(null)
const location = ref(null) // 字串（可為空字串＝未指定）；null 表示尚未選擇
const action = ref('SHIP')
const damageReason = ref('')
const otherReason = ref('')
const qtyText = ref('')
const note = ref('')
const busy = ref(false)
const picker = ref(null)
const qtyInput = ref(null)
const otherInput = ref(null)

const productId = computed(() => selected.value?.id || '')
const rows = computed(() => (productId.value ? stock.rowsOfProduct(productId.value) : []))
const qty = computed(() => parseQty(qtyText.value))
const available = computed(() => (productId.value && location.value !== null ? stock.qty(productId.value, location.value) : 0))
// 嚴格防呆：數量不可大於該存放地點的現有庫存
const insufficient = computed(() => !!qty.value && location.value !== null && qty.value > available.value)
const finalReason = computed(() => {
  if (action.value === 'SHIP') return '出貨'
  if (damageReason.value === '其他') return otherReason.value.trim()
  return damageReason.value
})
const reasonMissing = computed(() => action.value === 'DAMAGE' && !finalReason.value)
const canSubmit = computed(
  () =>
    !busy.value && !project.readOnly && productId.value && location.value !== null &&
    qty.value && !insufficient.value && !reasonMissing.value,
)
const verb = computed(() => (action.value === 'SHIP' ? '出貨' : '登記損壞'))

async function onSelect(sel) {
  selected.value = sel
  location.value = null
  const p = products.get(sel.id)
  // 自動帶入第一次入庫的數量
  qtyText.value = p?.first_qty > 0 ? String(p.first_qty) : ''
  const list = stock.rowsOfProduct(sel.id)
  if (list.length) location.value = list[0].location_id
  await nextTick()
  qtyInput.value?.focus()
  qtyInput.value?.select()
}

function onClear() {
  selected.value = null
  location.value = null
  qtyText.value = ''
}

async function pickReason(r) {
  damageReason.value = r
  if (r === '其他') {
    await nextTick()
    otherInput.value?.focus()
  }
}

async function submit() {
  if (!canSubmit.value) return
  busy.value = true
  try {
    const base = { product_id: productId.value, location_id: location.value, quantity: qty.value, note: note.value }
    if (action.value === 'SHIP') {
      await stock.apply({ kind: 'OUTBOUND', ...base, reason: '出貨' })
    } else {
      await stock.apply({ kind: 'DAMAGE', ...base, reason: finalReason.value })
    }
    ui.ok(`已${verb.value}：${selected.value.name} ${fmtInt(qty.value)} 件（${locLabel(location.value)}）`)
    selected.value = null
    location.value = null
    qtyText.value = ''
    note.value = ''
    otherReason.value = ''
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
  <PageHeader title="出庫/損壞" hint="數量不可超過該存放地點的現有庫存，系統會在畫面與後端雙重阻擋" />

  <div class="grid max-w-5xl grid-cols-5 gap-6">
    <form class="panel col-span-3 space-y-6 p-7" :style="{ '--tone': action === 'DAMAGE' ? 'var(--color-signal)' : 'var(--color-accent)' }" @submit.prevent="submit">
      <ProductPicker ref="picker" :selected="selected" @select="onSelect" @clear="onClear" />

      <div>
        <label class="field-label">存放地點</label>
        <div v-if="!productId" class="rounded-2xl bg-raised px-4 py-3.5 text-[13px] text-mute">
          先選擇品項，這裡會列出有庫存的存放地點
        </div>
        <div v-else-if="!rows.length" class="rounded-2xl bg-signal-soft px-4 py-3.5 text-[13px] text-signal">
          此品項目前沒有庫存
        </div>
        <div v-else class="grid max-h-56 grid-cols-2 gap-2 overflow-y-auto pr-1">
          <button
            v-for="r in rows"
            :key="r.location_id"
            type="button"
            class="pill justify-between px-4 py-2.5 text-left"
            :class="location === r.location_id ? 'pill-on' : ''"
            @click="location = r.location_id"
          >
            <span class="min-w-0 truncate text-[14px]" :class="r.location_id ? '' : 'italic opacity-80'">{{ locLabel(r.location_id) }}</span>
            <span class="num ml-2 text-lg">{{ fmtInt(r.quantity) }}</span>
          </button>
        </div>
      </div>

      <div>
        <label class="field-label">動作</label>
        <div class="grid grid-cols-2 gap-2">
          <button
            v-for="a in ACTIONS"
            :key="a.id"
            type="button"
            class="pill py-2.5 text-[15px]"
            :class="
              action === a.id ? (a.id === 'DAMAGE' ? 'pill-danger-on' : 'pill-on') : ''
            "
            :aria-pressed="action === a.id"
            @click="action = a.id"
          >{{ a.label }}</button>
        </div>
      </div>

      <div v-if="action === 'DAMAGE'">
        <label class="field-label">損壞原因</label>
        <div class="flex flex-wrap gap-2">
          <button
            v-for="r in DAMAGE_REASONS"
            :key="r"
            type="button"
            class="pill px-4 py-1.5"
            :class="damageReason === r ? 'pill-danger-on' : ''"
            :aria-pressed="damageReason === r"
            @click="pickReason(r)"
          >{{ r === '其他' ? '其他（自行輸入）' : r }}</button>
        </div>
        <input
          v-if="damageReason === '其他'"
          ref="otherInput"
          v-model="otherReason"
          class="field mt-2"
          maxlength="100"
          placeholder="請輸入損壞原因"
        />
        <p v-if="reasonMissing" class="mt-1.5 text-[12.5px] text-signal">請選擇損壞原因{{ damageReason === '其他' ? '並填寫內容' : '' }}</p>
      </div>

      <div class="grid grid-cols-2 gap-4">
        <div>
          <label class="field-label" for="out-qty">數量</label>
          <input
            id="out-qty"
            ref="qtyInput"
            v-model="qtyText"
            class="field num text-lg"
            :class="insufficient ? 'border-signal/70 bg-signal-soft focus:border-signal focus:ring-signal/20' : ''"
            inputmode="numeric"
            placeholder="0"
          />
          <p v-if="qtyText && !qty" class="mt-1 text-[12.5px] text-signal">請輸入大於 0 的整數</p>
        </div>
        <div>
          <label class="field-label" for="out-note">備註</label>
          <input id="out-note" v-model="note" class="field" placeholder="可不填" />
        </div>
      </div>

      <div
        v-if="insufficient"
        class="rounded-2xl bg-signal-soft px-5 py-3.5 text-[15px] font-bold text-signal ring-2 ring-signal/50"
        role="alert"
      >
        庫存不足！此存放地點剩餘 {{ fmtInt(available) }} 件，不可{{ verb }} {{ fmtInt(qty) }} 件
      </div>

      <button
        type="submit"
        class="w-full py-3.5 text-[15.5px]"
        :class="action === 'DAMAGE' ? 'btn-danger' : 'btn-primary'"
        :disabled="!canSubmit"
      >確認{{ verb }}</button>
    </form>

    <aside class="col-span-2">
      <QtyPreview
        v-if="productId && location !== null"
        :before="available"
        :after="qty && !insufficient ? available - qty : null"
        :caption="`${locLabel(location)} 現有`"
        unit="件"
        :danger="insufficient"
      />
    </aside>
  </div>
</template>
