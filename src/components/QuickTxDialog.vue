<script setup>
// 庫存清單列上的快速操作：入庫 / 出庫 / 損壞 / 移到新地點
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import ModalDialog from './ModalDialog.vue'
import ComboField from './ComboField.vue'
import QtyHint from './QtyHint.vue'
import { useProductStore } from '../stores/products'
import { useStockStore } from '../stores/stock'
import { useUiStore } from '../stores/ui'
import { fmtInt, locLabel, parseQty } from '../utils/format'

const props = defineProps({
  mode: { type: String, required: true }, // 'in' | 'out' | 'damage' | 'move'
  // { product_id, name, location_id, locs }：locs 是放在好幾個地點時的各地點（這時在視窗裡選地點）
  row: { type: Object, required: true },
})
const emit = defineEmits(['close'])

const products = useProductStore()
const stock = useStockStore()
const ui = useUiStore()

const MODES = {
  in: { title: '入庫', verb: '入庫', btn: 'btn-go', tone: 'go' },
  out: { title: '出庫', verb: '出貨', btn: 'btn-primary', tone: 'accent' },
  damage: { title: '登記損壞', verb: '登記損壞', btn: 'btn-danger', tone: 'signal' },
  move: { title: '移到新地點', verb: '移動', btn: 'btn-violet', tone: 'violet' },
}
const DAMAGE_REASONS = ['道具受損', '印刷錯誤', '其他']

const cfg = computed(() => MODES[props.mode])
const locs = props.row.locs && props.row.locs.length > 1 ? props.row.locs : null
// 存放地點：只有一個地點時就是那裡；好幾個地點時要選（入庫預設數量最多的那裡，出庫、損壞、移動一定要自己選）
const from = ref(locs ? (props.mode === 'in' ? locs[0].location_id : null) : props.row.location_id)
// 移動地點：原本有地點時叫「改變地點」，未指定時叫「新增地點」（與庫存清單按鈕一致）
const dialogTitle = computed(() => (props.mode === 'move' ? (locs || props.row.location_id ? '改變地點' : '新增地點') : cfg.value.title))
const product = computed(() => products.get(props.row.product_id))
const available = computed(() => (from.value === null ? 0 : stock.qty(props.row.product_id, from.value)))

// 預設數量：第一次入庫的數量；出庫、損壞時不超過這個地點現有的數量（不會一打開就是庫存不足）
const firstQty = product.value?.first_qty > 0 ? product.value.first_qty : 0
const autoQty = () => {
  if (props.mode === 'move' || !firstQty) return ''
  if (props.mode === 'in') return String(firstQty)
  if (from.value === null) return ''
  return available.value > 0 ? String(Math.min(firstQty, available.value)) : ''
}
const qtyText = ref(autoQty())
const qtyTouched = ref(false) // 使用者自己改過數量：換地點時不再自動帶入
watch(from, () => {
  if (!qtyTouched.value) qtyText.value = autoQty()
})
const note = ref('')
const reason = ref('')
const other = ref('')
const target = ref('')
const busy = ref(false)
const qtyEl = ref(null)
const targetEl = ref(null)

const qty = computed(() => parseQty(qtyText.value))
const deducts = computed(() => props.mode !== 'in')
const insufficient = computed(() => deducts.value && !!qty.value && qty.value > available.value)
const targetLoc = computed(() => target.value.trim())
const damageReason = computed(() => (reason.value === '其他' ? other.value.trim() : reason.value))
const problem = computed(() => {
  if (props.mode === 'move') {
    if (targetLoc.value.length > 50) return '存放地點請在 50 個字以內'
    if (from.value !== null && targetLoc.value === from.value) return '新地點不可與原地點相同'
  }
  return ''
})
const canSubmit = computed(() => {
  if (busy.value || from.value === null || !qty.value || insufficient.value || problem.value) return false
  if (props.mode === 'damage' && !damageReason.value) return false
  if (props.mode === 'move' && targetLoc.value === from.value) return false
  return true
})
const otherLocations = computed(() => stock.knownLocations.filter((l) => l !== from.value))
// 沒有用存放地點的專案：入庫、出庫、損壞不顯示「存放地點：未指定」（移動地點時照樣顯示原地點）
const showWhere = computed(() => props.mode === 'move' || !!props.row.location_id || stock.usesLocations)

async function submit() {
  if (!canSubmit.value) return
  busy.value = true
  const base = { product_id: props.row.product_id, quantity: qty.value }
  const at = from.value
  try {
    if (props.mode === 'in') {
      await stock.apply({ kind: 'INBOUND', ...base, location_id: at, reason: '入庫', note: note.value })
    } else if (props.mode === 'out') {
      await stock.apply({ kind: 'OUTBOUND', ...base, location_id: at, reason: '出貨', note: note.value })
    } else if (props.mode === 'damage') {
      await stock.apply({ kind: 'DAMAGE', ...base, location_id: at, reason: damageReason.value, note: note.value })
    } else {
      await stock.apply({
        kind: 'TRANSFER',
        product_id: props.row.product_id,
        from_location_id: at,
        to_location_id: targetLoc.value,
        quantity: qty.value,
        note: note.value,
      })
    }
    const where = props.mode === 'move' ? `${locLabel(at)} → ${locLabel(targetLoc.value)}` : showWhere.value ? locLabel(at) : ''
    ui.ok(`已${cfg.value.verb}：${props.row.name} ${fmtInt(qty.value)} 件${where ? `（${where}）` : ''}`)
    emit('close')
  } catch (err) {
    ui.error(err)
  } finally {
    busy.value = false
  }
}

async function pickReason(r) {
  reason.value = r
  if (r === '其他') {
    await nextTick()
    document.getElementById('qt-other')?.focus()
  }
}

onMounted(async () => {
  await nextTick()
  if (from.value === null) return // 先選地點
  const el = props.mode === 'move' ? targetEl.value : qtyEl.value
  el?.focus()
  el?.select?.()
})
</script>

<template>
  <ModalDialog :title="dialogTitle" :subtitle="row.name" :tone="cfg.tone" @close="emit('close')">
    <template #icon>
      <svg v-if="mode === 'in'" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 4v10M7.5 9.5 12 14l4.5-4.5M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" /></svg>
      <svg v-else-if="mode === 'out'" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 14V4M7.5 8.5 12 4l4.5 4.5M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" /></svg>
      <svg v-else-if="mode === 'damage'" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 9v4.5M12 17v.01M10.3 3.9 2.4 17.6A2 2 0 0 0 4.1 20.6h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z" /></svg>
      <svg v-else viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 8h13M13.5 4.5 17 8l-3.5 3.5M20 16H7M10.5 12.5 7 16l3.5 3.5" /></svg>
    </template>
    <form class="space-y-5" @submit.prevent="submit">
      <div v-if="locs">
        <label class="field-label">{{ mode === 'move' ? '從哪個地點移出' : mode === 'in' ? '放到哪個地點' : '從哪個地點' + cfg.verb }}</label>
        <div class="grid max-h-48 grid-cols-2 gap-2 overflow-y-auto pr-1">
          <button
            v-for="l in locs"
            :key="l.location_id"
            type="button"
            class="pill justify-between px-4 py-2.5 text-left"
            :class="from === l.location_id ? (mode === 'damage' ? 'pill-danger-on' : 'pill-on') : ''"
            :style="from === l.location_id && mode !== 'damage' ? { background: 'var(--tone)' } : null"
            :aria-pressed="from === l.location_id"
            @click="from = l.location_id"
          >
            <span class="min-w-0 truncate text-[14px]" :class="l.location_id ? '' : 'italic opacity-80'">{{ locLabel(l.location_id) }}</span>
            <span class="num ml-2 text-lg">{{ fmtInt(stock.qty(row.product_id, l.location_id)) }}</span>
          </button>
        </div>
        <p v-if="from === null" class="mt-1.5 pl-1 text-[12.5px] text-mute">這個品項放在好幾個地點，請點選一個</p>
      </div>
      <div v-else class="tone-card flex items-center justify-between">
        <div v-if="showWhere">
          <div class="text-[12.5px] font-semibold text-ink-2">{{ mode === 'move' ? '原存放地點' : '存放地點' }}</div>
          <div class="mt-0.5 text-[16px] font-bold" :class="row.location_id ? '' : 'italic text-mute'">{{ locLabel(row.location_id) }}</div>
        </div>
        <div v-else class="text-[15px] font-semibold text-ink-2">現有庫存</div>
        <div class="text-right">
          <div v-if="showWhere" class="text-[12.5px] font-semibold text-ink-2">現有庫存</div>
          <div class="num text-[26px] leading-tight" :style="{ color: 'var(--tone)' }">{{ fmtInt(available) }}</div>
        </div>
      </div>

      <div v-if="mode === 'move'">
        <label class="field-label" for="qt-target">新存放地點</label>
        <ComboField
          id="qt-target"
          ref="targetEl"
          v-model="target"
          :suggestions="otherLocations"
          title="已使用過的存放地點"
          maxlength="50"
          placeholder="輸入新地點，或選擇既有地點；留空代表「未指定」"
        />
        <p v-if="problem" class="mt-1.5 pl-1 text-[12.5px] text-signal">{{ problem }}</p>
      </div>

      <div v-if="mode === 'damage'">
        <label class="field-label">損壞原因</label>
        <div class="flex flex-wrap gap-2">
          <button
            v-for="r in DAMAGE_REASONS"
            :key="r"
            type="button"
            class="pill px-4 py-1.5"
            :class="reason === r ? 'pill-danger-on' : ''"
            :aria-pressed="reason === r"
            @click="pickReason(r)"
          >{{ r === '其他' ? '其他（自行輸入）' : r }}</button>
        </div>
        <input v-if="reason === '其他'" id="qt-other" v-model="other" class="field mt-2" maxlength="100" placeholder="請輸入損壞原因" />
      </div>

      <div class="grid grid-cols-2 gap-4">
        <div>
          <label class="field-label" for="qt-qty">數量</label>
          <input
            id="qt-qty"
            ref="qtyEl"
            v-model="qtyText"
            class="field num h-12 text-lg"
            :class="insufficient ? 'border-signal/70 bg-signal-soft' : ''"
            placeholder="可輸入算式，如 12*5"
            @input="qtyTouched = true"
          />
        </div>
        <div>
          <label class="field-label" for="qt-note">備註</label>
          <input id="qt-note" v-model="note" class="field h-12" placeholder="可不填" />
        </div>
      </div>
      <QtyHint :text="qtyText" class="-mt-2 pl-1" />
      <div v-if="insufficient" class="rounded-2xl bg-signal-soft px-5 py-3 text-[14.5px] font-bold text-signal ring-2 ring-signal/50" role="alert">
        庫存不足！{{ showWhere ? '此存放地點剩餘' : '目前只剩' }} {{ fmtInt(available) }} 件，不可{{ cfg.verb }} {{ fmtInt(qty) }} 件
      </div>
      <button type="submit" class="hidden" />
    </form>
    <template #footer>
      <button type="button" class="btn-ghost" @click="emit('close')">取消</button>
      <button type="button" :class="cfg.btn" :disabled="!canSubmit" @click="submit">確認{{ cfg.verb }}</button>
    </template>
  </ModalDialog>
</template>
