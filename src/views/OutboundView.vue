<script setup>
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import QtyHint from '../components/QtyHint.vue'
import ProductPicker from '../components/ProductPicker.vue'
import QtyPreview from '../components/QtyPreview.vue'
import ComboField from '../components/ComboField.vue'
import { useProductStore } from '../stores/products'
import { useStockStore } from '../stores/stock'
import { useUiStore } from '../stores/ui'
import { useProjectStore } from '../stores/project'
import { fmtInt, locLabel, parseQty } from '../utils/format'

const products = useProductStore()
const stock = useStockStore()
const ui = useUiStore()
const project = useProjectStore()

// 改變地點也放在這裡：一樣是從某個地點把東西拿出來，只是放到別的地點（總數不變），不是離開倉庫
const ACTIONS = [
  { id: 'SHIP', label: '出貨', tone: 'accent' },
  { id: 'DAMAGE', label: '損壞', tone: 'signal' },
  { id: 'MOVE', label: '改變地點', tone: 'violet' },
]
const DAMAGE_REASONS = ['道具受損', '印刷錯誤', '其他']

const selected = ref(null)
const location = ref(null) // 從哪個地點扣（改變地點＝從哪裡移出）。字串（可為空字串＝未指定）；null 表示尚未選擇
const action = ref('SHIP')
const damageReason = ref('')
const otherReason = ref('')
const qtyText = ref('')
const note = ref('')
const busy = ref(false)
const picker = ref(null)
const qtyInput = ref(null)
const otherInput = ref(null)
const targetInput = ref(null)

// 改變地點要移到哪裡：輸入或選一個地點，或是明確點「未指定」（不放在任何地點）。
// 欄位留空不算選好了——不然還沒填就按到確認，庫存會默默被移到「未指定」
const target = ref('')
const toNone = ref(false)

const productId = computed(() => selected.value?.id || '')
const rows = computed(() => (productId.value ? stock.rowsOfProduct(productId.value) : []))
const qty = computed(() => parseQty(qtyText.value))
const moving = computed(() => action.value === 'MOVE')
// 改變地點時一定列出地點讓人選（沒有用存放地點的專案也是：從「未指定」搬到新的地點）
const showLocations = computed(() => stock.usesLocations || moving.value)
const available = computed(() => (productId.value && location.value !== null ? stock.qty(productId.value, location.value) : 0))
// 嚴格防呆：數量不可大於該存放地點的現有庫存
const insufficient = computed(() => !!qty.value && location.value !== null && qty.value > available.value)
const finalReason = computed(() => {
  if (action.value === 'SHIP') return '出貨'
  if (damageReason.value === '其他') return otherReason.value.trim()
  return damageReason.value
})
const reasonMissing = computed(() => action.value === 'DAMAGE' && !finalReason.value)

const targetLoc = computed(() => (toNone.value ? '' : target.value.trim()))
const targetChosen = computed(() => toNone.value || !!target.value.trim())
// 來源就是「未指定」時不能再移到「未指定」
const canPickNone = computed(() => location.value !== '')
const otherLocations = computed(() => stock.knownLocations.filter((l) => l !== location.value))
const targetBefore = computed(() => (productId.value ? stock.qty(productId.value, targetLoc.value) : 0))
const moveProblem = computed(() => {
  if (!moving.value) return ''
  if (targetLoc.value.length > 50) return '存放地點請在 50 個字以內'
  if (location.value !== null && targetChosen.value && targetLoc.value === location.value) return '新地點不可與原地點相同'
  return ''
})
watch(target, (v) => {
  if (v.trim()) toNone.value = false
})
watch(location, (v) => {
  if (v === '') toNone.value = false
})

const canSubmit = computed(() => {
  if (busy.value || project.readOnly || !productId.value || location.value === null) return false
  if (!qty.value || insufficient.value || reasonMissing.value) return false
  return !moving.value || (targetChosen.value && !moveProblem.value)
})
const verb = computed(() => ({ SHIP: '出貨', DAMAGE: '登記損壞', MOVE: '移動' })[action.value])
const tone = computed(() => ACTIONS.find((a) => a.id === action.value).tone)
const toneStyle = computed(() => ({ '--tone': `var(--color-${tone.value})`, '--tone-fg': `var(--color-on-${tone.value})` }))
const btnClass = computed(() => ({ SHIP: 'btn-primary', DAMAGE: 'btn-danger', MOVE: 'btn-violet' })[action.value])

// 預設數量：
//   出貨、損壞＝第一次入庫的數量，但不超過選好的存放地點現有的數量（不會一選好就是庫存不足）
//   改變地點＝來源地點的全部數量（整批搬最常見，只搬一部分再自己改）
const qtyTouched = ref(false)
function autoQty() {
  if (moving.value) return location.value !== null && available.value > 0 ? String(available.value) : ''
  const p = products.get(productId.value)
  const first = p?.first_qty > 0 ? p.first_qty : 0
  if (!first) return ''
  if (location.value === null) return String(first)
  return available.value > 0 ? String(Math.min(first, available.value)) : ''
}
watch([location, moving], () => {
  if (!qtyTouched.value) qtyText.value = autoQty()
})
const qtyNote = computed(() =>
  moving.value && !qtyTouched.value && qtyText.value ? '已帶入這個地點的全部數量，只移一部分可直接修改' : '',
)

// 把游標放到下一個要填的欄位：改變地點先填要移到哪裡，其餘是數量；還沒選地點時不動
async function focusNext() {
  await nextTick()
  if (location.value === null) return
  const el = moving.value && (!targetChosen.value || moveProblem.value) ? targetInput.value : qtyInput.value
  el?.focus()
  el?.select?.()
}

function onSelect(sel) {
  selected.value = sel
  qtyTouched.value = false
  // 只放在一個地點（大部分沒有填地點的品項都是「未指定」一個）：直接選好，不用再點
  const list = stock.rowsOfProduct(sel.id)
  location.value = list.length === 1 ? list[0].location_id : null
  qtyText.value = autoQty()
  focusNext()
}

function onClear() {
  selected.value = null
  location.value = null
  qtyText.value = ''
  qtyTouched.value = false
}

function pickLocation(l) {
  location.value = l
  // 剛才填的目的地如果就是來源，清掉重填
  if (targetLoc.value === l && !toNone.value) target.value = ''
  focusNext()
}

function chooseAction(id) {
  if (id === action.value) return
  action.value = id
  if (id === 'MOVE' && location.value !== null && target.value.trim() === location.value) target.value = ''
  focusNext()
}

// 改變地點：明確選「未指定」（再點一次取消）。選了就清掉欄位裡打到一半的字
function pickNone() {
  toNone.value = !toNone.value
  if (toNone.value) target.value = ''
  focusNext()
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
    const name = selected.value.name
    const src = location.value
    if (moving.value) {
      const to = targetLoc.value
      await stock.apply({
        kind: 'TRANSFER',
        product_id: productId.value,
        from_location_id: src,
        to_location_id: to,
        quantity: qty.value,
        note: note.value,
      })
      ui.ok(`已移動：${name} ${fmtInt(qty.value)} 件（${locLabel(src)} → ${locLabel(to)}）`)
    } else {
      const base = { product_id: productId.value, location_id: src, quantity: qty.value, note: note.value }
      if (action.value === 'SHIP') {
        await stock.apply({ kind: 'OUTBOUND', ...base, reason: '出貨' })
      } else {
        await stock.apply({ kind: 'DAMAGE', ...base, reason: finalReason.value })
      }
      const where = stock.usesLocations ? `（${locLabel(src)}）` : ''
      ui.ok(`已${verb.value}：${name} ${fmtInt(qty.value)} 件${where}`)
    }
    selected.value = null
    location.value = null
    qtyText.value = ''
    qtyTouched.value = false
    note.value = ''
    otherReason.value = ''
    target.value = ''
    toNone.value = false
  } catch (err) {
    ui.error(err)
  } finally {
    busy.value = false
  }
}

// 從別的頁面帶著品項過來：ui.go('outbound', { productId, mode: 'move', from })
// 直接選好品項；mode 是 'move' 就切到改變地點，有給 from 的話連來源地點也選好
function applyParams(p) {
  const item = p?.productId ? products.get(p.productId) : null
  if (!item) return
  onSelect({ id: item.id, name: item.name, isNew: false })
  if (p.mode === 'move') action.value = 'MOVE'
  if (typeof p.from === 'string' && rows.value.some((r) => r.location_id === p.from)) location.value = p.from
  focusNext()
}
onMounted(() => applyParams(ui.params))
// 已經在出庫頁時又被帶品項過來（同一頁不會重新載入，所以另外接）
watch(
  () => ui.params,
  (p) => {
    if (ui.page === 'outbound') applyParams(p)
  },
)
</script>

<template>
  <PageHeader
    title="出庫/損壞"
    :hint="stock.usesLocations
      ? '選擇品項 → 選存放地點（只有一個地點時自動選好）→ 出貨、損壞或改變地點 → 確認數量，按 Enter 或「確認」'
      : '選擇品項 → 出貨、損壞或改變地點 → 確認數量，按 Enter 或「確認」'"
  />

  <div class="grid max-w-5xl grid-cols-5 gap-6">
    <form class="panel col-span-3 space-y-6 p-7" :style="toneStyle" @submit.prevent="submit">
      <div>
        <ProductPicker ref="picker" :selected="selected" @select="onSelect" @clear="onClear" />
        <!-- 有用存放地點的專案：選好品項，先說總共有多少、分放在幾個地點 -->
        <p v-if="productId && stock.usesLocations && rows.length" class="mt-2 pl-1 text-[12.5px] text-mute">
          全部共 <span class="num">{{ fmtInt(stock.total(productId)) }}</span> 件，放在 <span class="num">{{ fmtInt(rows.length) }}</span> 個地點
        </p>
      </div>

      <div>
        <label class="field-label">動作</label>
        <div class="grid grid-cols-3 gap-2">
          <button
            v-for="a in ACTIONS"
            :key="a.id"
            type="button"
            class="pill py-2.5 text-[15px]"
            :class="action === a.id ? 'pill-tone-on' : ''"
            :aria-pressed="action === a.id"
            @click="chooseAction(a.id)"
          >{{ a.label }}</button>
        </div>
        <p v-if="moving" class="mt-2 pl-1 text-[12.5px] text-mute">把現有的庫存搬到別的地點，總數不變</p>
      </div>

      <!-- 沒有用存放地點的專案：不用選地點（都是「未指定」那一個），只在沒有庫存時提醒 -->
      <div v-if="!showLocations && productId && !rows.length" class="rounded-2xl bg-signal-soft px-4 py-3.5 text-[13px] text-signal">
        此品項目前沒有庫存
      </div>
      <div v-if="showLocations">
        <label class="field-label">{{ moving ? '從哪個地點移出' : '存放地點' }}</label>
        <div v-if="!productId" class="rounded-2xl bg-raised px-4 py-3.5 text-[13px] text-mute">
          先選擇品項，這裡會列出有庫存的存放地點
        </div>
        <div v-else-if="!rows.length" class="rounded-2xl bg-signal-soft px-4 py-3.5 text-[13px] text-signal">
          此品項目前沒有庫存
        </div>
        <p v-else-if="location === null" class="-mt-1 mb-2 pl-1 text-[12.5px] text-mute">
          這個品項放在好幾個地點，請點選要從哪裡{{ moving ? '移出' : '出庫' }}
        </p>
        <div v-if="productId && rows.length" class="grid max-h-56 grid-cols-2 gap-2 overflow-y-auto pr-1">
          <button
            v-for="r in rows"
            :key="r.location_id"
            type="button"
            class="pill justify-between px-4 py-2.5 text-left"
            :class="location === r.location_id ? 'pill-tone-on' : ''"
            :aria-pressed="location === r.location_id"
            @click="pickLocation(r.location_id)"
          >
            <span class="min-w-0 truncate text-[14px]" :class="r.location_id ? '' : 'italic opacity-80'">{{ locLabel(r.location_id) }}</span>
            <span class="num ml-2 text-lg">{{ fmtInt(r.quantity) }}</span>
          </button>
        </div>
      </div>

      <div v-if="moving">
        <label class="field-label" for="out-target">移到哪個地點</label>
        <ComboField
          id="out-target"
          ref="targetInput"
          v-model="target"
          :suggestions="otherLocations"
          title="已使用過的存放地點"
          maxlength="50"
          :placeholder="toNone ? '已選「未指定」；要放到別的地點請直接輸入' : '輸入新地點，或選擇既有地點'"
        />
        <!-- 不想放在任何地點：要自己點「未指定」，欄位留空不會被當成未指定 -->
        <button
          v-if="canPickNone"
          type="button"
          class="pill mt-2 px-3.5 py-1.5 text-[13.5px]"
          :class="toNone ? 'pill-tone-on' : ''"
          :aria-pressed="toNone"
          @click="pickNone"
        >未指定（不設定地點）</button>
        <p v-if="moveProblem" class="mt-1.5 pl-1 text-[12.5px] text-signal">{{ moveProblem }}</p>
        <p v-else-if="toNone" class="mt-1.5 pl-1 text-[12.5px] text-mute">會移到「未指定」，不放在任何地點</p>
        <p v-else-if="!targetChosen" class="mt-1.5 pl-1 text-[12.5px] text-mute">
          {{ canPickNone ? '請輸入或選擇要移到的地點；不想指定地點就點「未指定」' : '原地點是「未指定」，請輸入要移到哪個地點' }}
        </p>
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
            class="field num h-12 text-lg"
            :class="insufficient ? 'border-signal/70 bg-signal-soft focus:border-signal focus:ring-signal/20' : ''"
            placeholder="可輸入算式，如 12*5"
            @input="qtyTouched = true"
          />
          <QtyHint :text="qtyText" class="mt-1 pl-1">
            <template v-if="qtyNote" #default>
              <p class="text-[12.5px] text-mute">{{ qtyNote }}</p>
            </template>
          </QtyHint>
        </div>
        <div>
          <label class="field-label" for="out-note">備註</label>
          <input id="out-note" v-model="note" class="field h-12" placeholder="可不填" />
        </div>
      </div>

      <div
        v-if="insufficient"
        class="rounded-2xl bg-signal-soft px-5 py-3.5 text-[15px] font-bold text-signal ring-2 ring-signal/50"
        role="alert"
      >
        庫存不足！{{ showLocations ? '此存放地點剩餘' : '目前只剩' }} {{ fmtInt(available) }} 件，不可{{ verb }} {{ fmtInt(qty) }} 件
      </div>

      <button type="submit" class="w-full py-3.5 text-[15.5px]" :class="btnClass" :disabled="!canSubmit">確認{{ verb }}</button>
    </form>

    <aside class="col-span-2 space-y-4">
      <QtyPreview
        v-if="productId && location !== null"
        :before="available"
        :after="qty && !insufficient ? available - qty : null"
        :caption="showLocations ? `${locLabel(location)} 現有` : '現有'"
        :after-label="moving ? '移出後' : undefined"
        :tone="moving ? 'violet' : undefined"
        :delta="moving"
        unit="件"
        :danger="insufficient"
      />
      <!-- 改變地點：目的地會變成多少 -->
      <QtyPreview
        v-if="moving && productId && location !== null && targetChosen && !moveProblem"
        :before="targetBefore"
        :after="qty && !insufficient ? targetBefore + qty : null"
        :caption="`${locLabel(targetLoc)} 現有`"
        after-label="移入後"
        unit="件"
        tone="violet"
        delta
      />
    </aside>
  </div>
</template>
