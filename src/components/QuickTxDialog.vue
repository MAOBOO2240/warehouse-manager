<script setup>
// 庫存清單列上的快速操作：入庫 / 出庫 / 損壞 / 移到新地點
import { computed, nextTick, onMounted, ref } from 'vue'
import ModalDialog from './ModalDialog.vue'
import { useProductStore } from '../stores/products'
import { useStockStore } from '../stores/stock'
import { useUiStore } from '../stores/ui'
import { fmtInt, locLabel, parseQty } from '../utils/format'

const props = defineProps({
  mode: { type: String, required: true }, // 'in' | 'out' | 'damage' | 'move'
  row: { type: Object, required: true }, // { product_id, name, location_id }
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
const product = computed(() => products.get(props.row.product_id))
const available = computed(() => stock.qty(props.row.product_id, props.row.location_id))

const firstQty = product.value?.first_qty > 0 ? product.value.first_qty : 0
const qtyText = ref(props.mode === 'move' ? '' : firstQty ? String(firstQty) : '')
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
    if (targetLoc.value === props.row.location_id) return '新地點不可與原地點相同'
  }
  return ''
})
const canSubmit = computed(() => {
  if (busy.value || !qty.value || insufficient.value || problem.value) return false
  if (props.mode === 'damage' && !damageReason.value) return false
  if (props.mode === 'move' && targetLoc.value === props.row.location_id) return false
  return true
})
const otherLocations = computed(() => stock.knownLocations.filter((l) => l !== props.row.location_id))

async function submit() {
  if (!canSubmit.value) return
  busy.value = true
  const base = { product_id: props.row.product_id, quantity: qty.value }
  try {
    if (props.mode === 'in') {
      await stock.apply({ kind: 'INBOUND', ...base, location_id: props.row.location_id, reason: '入庫', note: note.value })
    } else if (props.mode === 'out') {
      await stock.apply({ kind: 'OUTBOUND', ...base, location_id: props.row.location_id, reason: '出貨', note: note.value })
    } else if (props.mode === 'damage') {
      await stock.apply({ kind: 'DAMAGE', ...base, location_id: props.row.location_id, reason: damageReason.value, note: note.value })
    } else {
      await stock.apply({
        kind: 'TRANSFER',
        product_id: props.row.product_id,
        from_location_id: props.row.location_id,
        to_location_id: targetLoc.value,
        quantity: qty.value,
        note: note.value,
      })
    }
    const where = props.mode === 'move' ? `${locLabel(props.row.location_id)} → ${locLabel(targetLoc.value)}` : locLabel(props.row.location_id)
    ui.ok(`已${cfg.value.verb}：${props.row.name} ${fmtInt(qty.value)} 件（${where}）`)
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
  const el = props.mode === 'move' ? targetEl.value : qtyEl.value
  el?.focus()
  el?.select?.()
})
</script>

<template>
  <ModalDialog :title="cfg.title" :subtitle="row.name" :tone="cfg.tone" @close="emit('close')">
    <template #icon>
      <svg v-if="mode === 'in'" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 4v10M7.5 9.5 12 14l4.5-4.5M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" /></svg>
      <svg v-else-if="mode === 'out'" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 14V4M7.5 8.5 12 4l4.5 4.5M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" /></svg>
      <svg v-else-if="mode === 'damage'" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 9v4.5M12 17v.01M10.3 3.9 2.4 17.6A2 2 0 0 0 4.1 20.6h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z" /></svg>
      <svg v-else viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 8h13M13.5 4.5 17 8l-3.5 3.5M20 16H7M10.5 12.5 7 16l3.5 3.5" /></svg>
    </template>
    <form class="space-y-5" @submit.prevent="submit">
      <div class="tone-card flex items-center justify-between">
        <div>
          <div class="text-[12.5px] font-semibold text-ink-2">{{ mode === 'move' ? '原存放地點' : '存放地點' }}</div>
          <div class="mt-0.5 text-[16px] font-bold" :class="row.location_id ? '' : 'italic text-mute'">{{ locLabel(row.location_id) }}</div>
        </div>
        <div class="text-right">
          <div class="text-[12.5px] font-semibold text-ink-2">現有庫存</div>
          <div class="num text-[26px] leading-tight" :style="{ color: 'var(--tone)' }">{{ fmtInt(available) }}</div>
        </div>
      </div>

      <div v-if="mode === 'move'">
        <label class="field-label" for="qt-target">新存放地點</label>
        <input
          id="qt-target"
          ref="targetEl"
          v-model="target"
          class="field"
          list="qt-locs"
          maxlength="50"
          placeholder="輸入新地點，或選擇既有地點；留空代表「未指定」"
          autocomplete="off"
        />
        <datalist id="qt-locs">
          <option v-for="l in otherLocations" :key="l" :value="l" />
        </datalist>
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
            class="field num text-lg"
            :class="insufficient ? 'border-signal/70 bg-signal-soft' : ''"
            inputmode="numeric"
            placeholder="0"
          />
        </div>
        <div>
          <label class="field-label" for="qt-note">備註</label>
          <input id="qt-note" v-model="note" class="field" placeholder="可不填" />
        </div>
      </div>
      <p v-if="qtyText && !qty" class="-mt-2 pl-1 text-[12.5px] text-signal">請輸入大於 0 的整數</p>
      <div v-if="insufficient" class="rounded-2xl bg-signal-soft px-5 py-3 text-[14.5px] font-bold text-signal ring-2 ring-signal/50" role="alert">
        庫存不足！此存放地點剩餘 {{ fmtInt(available) }} 件，不可{{ cfg.verb }} {{ fmtInt(qty) }} 件
      </div>
      <button type="submit" class="hidden" />
    </form>
    <template #footer>
      <button type="button" class="btn-ghost" @click="emit('close')">取消</button>
      <button type="button" :class="cfg.btn" :disabled="!canSubmit" @click="submit">確認{{ cfg.verb }}</button>
    </template>
  </ModalDialog>
</template>
