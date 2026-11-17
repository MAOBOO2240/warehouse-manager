<script setup>
// 庫存清單列上的快速操作：入庫（新入庫／改變地點）/ 出庫 / 損壞
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import ModalDialog from './ModalDialog.vue'
import ComboField from './ComboField.vue'
import QtyHint from './QtyHint.vue'
import { useProductStore } from '../stores/products'
import { useStockStore } from '../stores/stock'
import { useUiStore } from '../stores/ui'
import { fmtInt, locLabel, parseQty } from '../utils/format'

const props = defineProps({
  mode: { type: String, required: true }, // 'in' | 'out' | 'damage'
  // { product_id, name, location_id, locs }：locs 是放在好幾個地點時的各地點（這時在視窗裡選地點）
  row: { type: Object, required: true },
  // 入庫視窗一打開停在哪一邊：'add' 新入庫、'move' 改變地點（品項沒有庫存時只能是新入庫）
  startTab: { type: String, default: 'add' },
})
const emit = defineEmits(['close'])

const products = useProductStore()
const stock = useStockStore()
const ui = useUiStore()

const MODES = {
  in: { title: '入庫', verb: '入庫', btn: 'btn-go', tone: 'go' },
  out: { title: '出庫', verb: '出貨', btn: 'btn-primary', tone: 'accent' },
  damage: { title: '登記損壞', verb: '登記損壞', btn: 'btn-danger', tone: 'signal' },
}
// 改變地點併在入庫裡：一樣是把東西放進某個地點，只是從別的地點搬過來（總數不變），不是從外面進來
const MOVE = { title: '改變地點', verb: '移動', btn: 'btn-violet', tone: 'violet' }
const DAMAGE_REASONS = ['道具受損', '印刷錯誤', '其他']

const pid = props.row.product_id
// 這一列代表好幾個地點時的各地點；只有一個地點時是 null
const locs = props.row.locs && props.row.locs.length > 1 ? props.row.locs : null
const rowLoc = props.row.location_id ?? ''
const product = computed(() => products.get(pid))
// 這個品項目前有庫存的地點（數量多的在前）與不分地點的總數
const places = computed(() => stock.rowsOfProduct(pid))
const total = computed(() => stock.total(pid))

// 入庫的兩種方式（視窗最上面切換）：add＝新入庫（從外面進來，總數增加）；move＝改變地點（從別的地點搬過來，總數不變）
const canMove = computed(() => total.value > 0)
const tab = ref(props.startTab === 'move' && canMove.value ? 'move' : 'add')
const isMove = computed(() => props.mode === 'in' && tab.value === 'move')
const adding = computed(() => props.mode === 'in' && tab.value === 'add')
const cfg = computed(() => (isMove.value ? MOVE : MODES[props.mode]))
// 會扣掉某個地點庫存的操作：出庫、損壞、改變地點（從來源地點移出）
const deducts = computed(() => !adding.value)

// 扣庫存時可以選的地點；null＝固定是這一列的地點。
// 這一列代表好幾個地點時一定要自己選；改變地點時，萬一這一列的地點沒有庫存（東西放在別的地方），也列出來選
const choices = locs ?? (props.mode === 'in' && places.value.length && stock.qty(pid, rowLoc) <= 0 ? places.value : null)
// 從哪個地點扣（改變地點的來源）；null 表示還沒選
const from = ref(choices ? (choices.length === 1 ? choices[0].location_id : null) : rowLoc)
const available = computed(() => (from.value === null ? 0 : stock.qty(pid, from.value)))

// 新入庫要放的地點：可以改，預設是這一列的地點（好幾個地點時是數量最多的那裡）
const place = ref(locs ? locs.reduce((m, l) => (stock.qty(pid, l.location_id) > stock.qty(pid, m.location_id) ? l : m)).location_id : rowLoc)
const placeLoc = computed(() => place.value.trim())
const placeQty = computed(() => stock.qty(pid, placeLoc.value))

// 改變地點要移到哪裡：輸入或選一個地點，或是明確點「未指定」（不放在任何地點）。
// 欄位留空不算選好了——不然還沒填就按到確認，庫存會默默被移到「未指定」
const target = ref('')
const toNone = ref(false)
const targetLoc = computed(() => (toNone.value ? '' : target.value.trim()))
const targetChosen = computed(() => toNone.value || !!target.value.trim())
const otherLocations = computed(() => stock.knownLocations.filter((l) => l !== from.value))
// 來源就是「未指定」時不能再移到「未指定」
const canPickNone = computed(() => from.value !== '')
watch(target, (v) => {
  if (v.trim()) toNone.value = false
})
watch(from, (v) => {
  if (v === '') toNone.value = false
})

// 預設數量：
//   新入庫＝第一次入庫的數量；出庫、損壞也是，但不超過這個地點現有的數量（不會一打開就是庫存不足）
//   改變地點＝來源地點的全部數量（整批搬最常見，只搬一部分再自己改）
const firstQty = product.value?.first_qty > 0 ? product.value.first_qty : 0
const autoQty = () => {
  if (isMove.value) return available.value > 0 ? String(available.value) : ''
  if (!firstQty) return ''
  if (adding.value) return String(firstQty)
  if (from.value === null) return ''
  return available.value > 0 ? String(Math.min(firstQty, available.value)) : ''
}
const qtyText = ref(autoQty())
const qtyTouched = ref(false) // 使用者自己改過數量：換地點、換入庫方式時不再自動帶入
watch([from, tab], () => {
  if (!qtyTouched.value) qtyText.value = autoQty()
})
const note = ref('')
const reason = ref('')
const other = ref('')
const busy = ref(false)
const qtyEl = ref(null)
const targetEl = ref(null)

const qty = computed(() => parseQty(qtyText.value))
const insufficient = computed(() => deducts.value && from.value !== null && !!qty.value && qty.value > available.value)
const damageReason = computed(() => (reason.value === '其他' ? other.value.trim() : reason.value))
// 沒有用存放地點的專案：入庫、出庫、損壞不顯示「存放地點：未指定」（改變地點時照樣顯示原地點）
const showWhere = computed(() => isMove.value || !!props.row.location_id || stock.usesLocations)
// 改變地點：新地點和原地點一樣（等於沒有移）
const sameAsFrom = computed(() => isMove.value && from.value !== null && targetChosen.value && targetLoc.value === from.value)
const problem = computed(() => {
  if (adding.value) return placeLoc.value.length > 50 ? '存放地點請在 50 個字以內' : ''
  if (!isMove.value) return ''
  if (targetLoc.value.length > 50) return '存放地點請在 50 個字以內'
  if (sameAsFrom.value) return '新地點不可與原地點相同'
  return ''
})
const canSubmit = computed(() => {
  if (busy.value || !qty.value || problem.value) return false
  if (adding.value) return true
  if (from.value === null || insufficient.value) return false
  if (props.mode === 'damage' && !damageReason.value) return false
  // 改變地點：一定要選好要移到哪裡
  return !isMove.value || targetChosen.value
})
// 數量是自動帶入的時候，在欄位下面說明帶的是什麼
const qtyNote = computed(() => {
  if (qtyTouched.value || !qtyText.value) return ''
  if (isMove.value) return '已帶入這個地點的全部數量，只移一部分可直接修改'
  return adding.value ? '已帶入第一次入庫的數量' : ''
})
// 改變地點：可以送出時，先算好兩邊移動前後各是多少
const moved = computed(() => {
  if (!isMove.value || from.value === null || !targetChosen.value || !qty.value || insufficient.value || problem.value) return null
  const toBefore = stock.qty(pid, targetLoc.value)
  return {
    from: `${fmtInt(available.value)} → ${fmtInt(available.value - qty.value)}`,
    to: `${fmtInt(toBefore)} → ${fmtInt(toBefore + qty.value)}`,
  }
})

async function submit() {
  if (!canSubmit.value) return
  const n = qty.value
  const at = from.value
  const base = { product_id: pid, quantity: n, note: note.value }
  let tx
  let where
  if (isMove.value) {
    const to = targetLoc.value
    tx = { kind: 'TRANSFER', ...base, from_location_id: at, to_location_id: to }
    where = `${locLabel(at)} → ${locLabel(to)}`
  } else if (props.mode === 'in') {
    const to = placeLoc.value
    tx = { kind: 'INBOUND', ...base, location_id: to, reason: '入庫' }
    where = showWhere.value ? locLabel(to) : ''
  } else {
    tx = props.mode === 'out' ? { kind: 'OUTBOUND', ...base, location_id: at, reason: '出貨' } : { kind: 'DAMAGE', ...base, location_id: at, reason: damageReason.value }
    where = showWhere.value ? locLabel(at) : ''
  }
  const done = `已${cfg.value.verb}：${props.row.name} ${fmtInt(n)} 件${where ? `（${where}）` : ''}`
  busy.value = true
  try {
    await stock.apply(tx)
    ui.ok(done)
    emit('close')
  } catch (err) {
    ui.error(err)
  } finally {
    busy.value = false
  }
}

// 把游標放到第一個還要填的欄位：改變地點先填要移到哪裡，其餘是數量；要先選地點時不動
async function focusNext() {
  await nextTick()
  if (deducts.value && from.value === null) return
  const el = isMove.value && (!targetChosen.value || problem.value) ? targetEl.value : qtyEl.value
  el?.focus()
  el?.select?.()
}

// 改變地點：明確選「未指定」（再點一次取消）。選了就清掉欄位裡打到一半的字
function pickNone() {
  toNone.value = !toNone.value
  if (toNone.value) target.value = ''
  focusNext()
}

function setTab(t) {
  if (t === tab.value) return
  tab.value = t
  focusNext()
}

function pickFrom(loc) {
  from.value = loc
  // 改變地點：選好來源，接著填要移到哪裡
  if (isMove.value) focusNext()
}

// 新入庫：點一下品項目前放的地點，直接填進存放地點
function pickPlace(loc) {
  place.value = loc
  focusNext()
}

async function pickReason(r) {
  reason.value = r
  if (r === '其他') {
    await nextTick()
    document.getElementById('qt-other')?.focus()
  }
}

onMounted(focusNext)
</script>

<template>
  <ModalDialog :title="cfg.title" :subtitle="row.name" :tone="cfg.tone" @close="emit('close')">
    <template #icon>
      <svg v-if="isMove" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 8h13M13.5 4.5 17 8l-3.5 3.5M20 16H7M10.5 12.5 7 16l3.5 3.5" /></svg>
      <svg v-else-if="mode === 'in'" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 4v10M7.5 9.5 12 14l4.5-4.5M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" /></svg>
      <svg v-else-if="mode === 'out'" viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 14V4M7.5 8.5 12 4l4.5 4.5M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" /></svg>
      <svg v-else viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 9v4.5M12 17v.01M10.3 3.9 2.4 17.6A2 2 0 0 0 4.1 20.6h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z" /></svg>
    </template>
    <form class="space-y-5" @submit.prevent="submit">
      <!-- 入庫方式：東西是從外面進來（新入庫），還是從別的地點搬過來（改變地點） -->
      <div v-if="mode === 'in'">
        <div class="seg grid w-full grid-cols-2" role="group" aria-label="入庫方式">
          <button type="button" class="seg-item" :class="tab === 'add' ? 'is-on' : ''" :aria-pressed="tab === 'add'" @click="setTab('add')">
            <svg viewBox="0 0 24 24" class="h-4 w-4 shrink-0" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 4v10M7.5 9.5 12 14l4.5-4.5M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" /></svg>
            新入庫
          </button>
          <button type="button" class="seg-item" :class="tab === 'move' ? 'is-on' : ''" :aria-pressed="tab === 'move'" :disabled="!canMove" @click="setTab('move')">
            <svg viewBox="0 0 24 24" class="h-4 w-4 shrink-0" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 8h13M13.5 4.5 17 8l-3.5 3.5M20 16H7M10.5 12.5 7 16l3.5 3.5" /></svg>
            改變地點
          </button>
        </div>
        <p class="mt-2 pl-1 text-[12.5px] text-mute">
          {{ !canMove ? '這個品項目前沒有庫存，沒有東西可以移動' : isMove ? '把現有的庫存搬到別的地點，總數不變' : '新進的貨放進倉庫，總數會增加' }}
        </p>
      </div>

      <!-- 新入庫：要放到哪個地點（可以改） -->
      <div v-if="adding && showWhere">
        <label class="field-label" for="qt-place">存放地點</label>
        <!-- 放在好幾個地點的品項：列出來，點一下就填進欄位 -->
        <div v-if="places.length > 1" class="mb-2.5 flex max-h-24 flex-wrap gap-2 overflow-y-auto pr-1">
          <button
            v-for="r in places"
            :key="r.location_id"
            type="button"
            class="pill max-w-full px-3.5 py-1.5 text-[13.5px]"
            :class="placeLoc === r.location_id ? 'pill-tone-on' : ''"
            :aria-pressed="placeLoc === r.location_id"
            @click="pickPlace(r.location_id)"
          >
            <span class="min-w-0 truncate" :class="r.location_id ? '' : 'italic opacity-80'">{{ locLabel(r.location_id) }}</span>
            <span class="num ml-2 text-[15px]">{{ fmtInt(r.quantity) }}</span>
          </button>
        </div>
        <ComboField
          id="qt-place"
          v-model="place"
          :suggestions="stock.knownLocations"
          title="已使用過的存放地點"
          maxlength="50"
          placeholder="留空代表「未指定」"
        />
        <p v-if="problem" class="mt-1.5 pl-1 text-[12.5px] text-signal">{{ problem }}</p>
        <p v-else class="mt-1.5 pl-1 text-[12.5px] text-mute">
          {{ placeLoc ? '這個地點' : '「未指定」' }}現有 <span class="num text-[14px] font-semibold text-ink-2">{{ fmtInt(placeQty) }}</span> 件<template v-if="total !== placeQty">・這個品項全部共 <span class="num text-[14px] font-semibold text-ink-2">{{ fmtInt(total) }}</span> 件</template>
        </p>
      </div>
      <!-- 出庫、損壞、改變地點：放在好幾個地點時，選要從哪裡扣 -->
      <div v-else-if="deducts && choices">
        <label class="field-label">{{ isMove ? '從哪個地點移出' : '從哪個地點' + cfg.verb }}</label>
        <div class="grid max-h-48 grid-cols-2 gap-2 overflow-y-auto pr-1">
          <button
            v-for="l in choices"
            :key="l.location_id"
            type="button"
            class="pill justify-between px-4 py-2.5 text-left"
            :class="from === l.location_id ? 'pill-tone-on' : ''"
            :aria-pressed="from === l.location_id"
            @click="pickFrom(l.location_id)"
          >
            <span class="min-w-0 truncate text-[14px]" :class="l.location_id ? '' : 'italic opacity-80'">{{ locLabel(l.location_id) }}</span>
            <span class="num ml-2 text-lg">{{ fmtInt(stock.qty(pid, l.location_id)) }}</span>
          </button>
        </div>
        <p v-if="from === null" class="mt-1.5 pl-1 text-[12.5px] text-mute">這個品項放在好幾個地點，請點選一個</p>
      </div>
      <!-- 只有一個地點：直接顯示那裡的現有庫存（沒有用存放地點的專案不寫地點） -->
      <div v-else class="tone-card flex items-center justify-between gap-4">
        <div v-if="showWhere" class="min-w-0">
          <div class="text-[12.5px] font-semibold text-ink-2">{{ isMove ? '原存放地點' : '存放地點' }}</div>
          <div class="mt-0.5 text-[16px] font-bold wrap-anywhere" :class="from ? '' : 'italic text-mute'">{{ locLabel(from) }}</div>
        </div>
        <div v-else class="text-[15px] font-semibold text-ink-2">現有庫存</div>
        <div class="shrink-0 text-right">
          <div v-if="showWhere" class="text-[12.5px] font-semibold text-ink-2">現有庫存</div>
          <div class="num text-[26px] leading-tight" :style="{ color: 'var(--tone)' }">{{ fmtInt(adding ? placeQty : available) }}</div>
        </div>
      </div>

      <div v-if="isMove">
        <label class="field-label" for="qt-target">移到哪個地點</label>
        <ComboField
          id="qt-target"
          ref="targetEl"
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
        <p v-if="problem" class="mt-1.5 pl-1 text-[12.5px] text-signal">{{ problem }}</p>
        <p v-else-if="toNone" class="mt-1.5 pl-1 text-[12.5px] text-mute">會移到「未指定」，不放在任何地點</p>
        <p v-else-if="!targetChosen" class="mt-1.5 pl-1 text-[12.5px] text-mute">
          {{ canPickNone ? '請輸入或選擇要移到的地點；不想指定地點就點「未指定」' : '原地點是「未指定」，請輸入要移到哪個地點' }}
        </p>
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
      <QtyHint :text="qtyText" class="-mt-2 pl-1">
        <template v-if="qtyNote" #default>
          <p class="text-[12.5px] text-mute">{{ qtyNote }}</p>
        </template>
      </QtyHint>
      <div v-if="insufficient" class="rounded-2xl bg-signal-soft px-5 py-3 text-[14.5px] font-bold text-signal ring-2 ring-signal/50" role="alert">
        庫存不足！{{ showWhere ? '此存放地點剩餘' : '目前只剩' }} {{ fmtInt(available) }} 件，不可{{ cfg.verb }} {{ fmtInt(qty) }} 件
      </div>
      <!-- 改變地點：移動之後兩邊各是多少（地點名稱太長時截短，完整名稱在上面的欄位） -->
      <div
        v-if="moved"
        class="tone-card flex items-baseline justify-center gap-2 px-4 py-2.5 text-[14.5px] whitespace-nowrap"
        :title="`${locLabel(from)} ${moved.from} ・ ${locLabel(targetLoc)} ${moved.to}`"
      >
        <span class="flex min-w-0 items-baseline gap-1.5">
          <span class="min-w-0 truncate font-semibold" :class="from ? '' : 'italic'">{{ locLabel(from) }}</span>
          <span class="num shrink-0">{{ moved.from }}</span>
        </span>
        <span class="shrink-0 text-mute" aria-hidden="true">・</span>
        <span class="flex min-w-0 items-baseline gap-1.5">
          <span class="min-w-0 truncate font-semibold" :class="targetLoc ? '' : 'italic'">{{ locLabel(targetLoc) }}</span>
          <span class="num shrink-0">{{ moved.to }}</span>
        </span>
      </div>
      <button type="submit" class="hidden" />
    </form>
    <template #footer>
      <button type="button" class="btn-ghost" @click="emit('close')">取消</button>
      <button type="button" :class="cfg.btn" :disabled="!canSubmit" @click="submit">確認{{ cfg.verb }}</button>
    </template>
  </ModalDialog>
</template>
