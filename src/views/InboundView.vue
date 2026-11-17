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
// 入庫方式：add＝新入庫（從外面進來，總數增加）；move＝改變地點（把現有庫存搬到別的地點，總數不變）
const way = ref('add')
const from = ref(null) // 改變地點時從哪個地點移出；null 表示尚未選擇
const location = ref('') // 新入庫＝放到哪個地點；改變地點＝移到哪個地點
// 改變地點時明確選了「未指定」（不放在任何地點）。新入庫的存放地點可以留空；
// 改變地點留空不算選好了——不然還沒填就按到確認，庫存會默默被移到「未指定」
const toNone = ref(false)
const qtyText = ref('')
const qtyTouched = ref(false) // 使用者自己改過數量：換入庫方式、換來源地點時不再自動帶入
const note = ref('')
const busy = ref(false)
const picker = ref(null)
const qtyInput = ref(null)
const locInput = ref(null)

const qty = computed(() => parseQty(qtyText.value))
const loc = computed(() => location.value.trim())
const productId = computed(() => (selected.value && !selected.value.isNew ? selected.value.id : ''))
// 這個品項目前有庫存的地點（數量多的在前）
const rows = computed(() => (productId.value ? stock.rowsOfProduct(productId.value) : []))
// 有庫存的既有品項才能改變地點（新品項、沒有庫存的品項沒有東西可以移）
const canMove = computed(() => rows.value.length > 0)
const moving = computed(() => way.value === 'move' && canMove.value)
const before = computed(() => (productId.value ? stock.qty(productId.value, loc.value) : 0))
const available = computed(() => (moving.value && from.value !== null ? stock.qty(productId.value, from.value) : 0))
// 嚴格防呆：移動的數量不可大於來源地點的現有庫存
const insufficient = computed(() => moving.value && from.value !== null && !!qty.value && qty.value > available.value)
const locTooLong = computed(() => loc.value.length > 50)
// 改變地點：要移到哪裡選好了沒有（輸入／選了一個地點，或點了「未指定」）
const destChosen = computed(() => toNone.value || !!loc.value)
// 來源就是「未指定」時不能再移到「未指定」
const canPickNone = computed(() => from.value !== '')
// 改變地點：新地點和原地點一樣（等於沒有移）
const sameAsFrom = computed(() => moving.value && from.value !== null && destChosen.value && loc.value === from.value)
const problem = computed(() => {
  if (locTooLong.value) return '存放地點請在 50 個字以內'
  if (sameAsFrom.value) return '新地點不可與原地點相同'
  return ''
})
const canSubmit = computed(() => {
  if (busy.value || project.readOnly || !selected.value || !qty.value || problem.value) return false
  if (!moving.value) return true
  return from.value !== null && !insufficient.value && destChosen.value
})
// 改變地點時，建議清單不列出來源地點
const otherLocations = computed(() => stock.knownLocations.filter((l) => l !== from.value))
// 右邊「目前存放位置」裡，改變地點的來源那一列（不能又移回同一個地點）
const isSource = (r) => moving.value && from.value === r.location_id
// 右邊「目前存放位置」裡，現在要放進去（或移入）的那一列
const isDest = (r) => loc.value === r.location_id && (!moving.value || destChosen.value)
watch(location, (v) => {
  if (v.trim()) toNone.value = false
})
watch(from, (v) => {
  if (v === '') toNone.value = false
})
// 整個表單的主題色：入庫是綠色、改變地點是紫色
const toneStyle = computed(() => {
  const t = moving.value ? 'violet' : 'go'
  return { '--tone': `var(--color-${t})`, '--tone-fg': `var(--color-on-${t})` }
})

// 預設數量：新入庫＝第一次入庫的數量；改變地點＝來源地點的全部數量（整批搬最常見，只搬一部分再自己改）
function autoQty() {
  if (moving.value) return available.value > 0 ? String(available.value) : ''
  const p = products.get(productId.value)
  return p?.first_qty > 0 ? String(p.first_qty) : ''
}
watch([moving, from], () => {
  if (!qtyTouched.value) qtyText.value = autoQty()
})
// 數量欄下面的說明：帶入的是什麼數字
const qtyNote = computed(() => {
  if (moving.value) return !qtyTouched.value && qtyText.value ? '已帶入這個地點的全部數量，只移一部分可直接修改' : ''
  return productId.value && products.get(productId.value)?.first_qty > 0 ? '已帶入第一次入庫的數量' : ''
})

function onSelect(sel) {
  selected.value = sel
  // 換了品項：回到新入庫，自動帶入第一次入庫的數量
  way.value = 'add'
  from.value = null
  toNone.value = false
  qtyTouched.value = false
  qtyText.value = autoQty()
}

function onClear() {
  selected.value = null
  way.value = 'add'
  from.value = null
  toNone.value = false
  qtyText.value = ''
  qtyTouched.value = false
}

// 切到改變地點；src 是指定的來源地點（從別的頁面帶過來時）
function startMove(src = null) {
  way.value = 'move'
  const has = (l) => rows.value.some((r) => r.location_id === l)
  // 只放在一個地點（大部分沒有填地點的品項都是「未指定」一個）：來源直接選好，不用再點
  if (src !== null && has(src)) from.value = src
  else if (!has(from.value)) from.value = rows.value.length === 1 ? rows.value[0].location_id : null
  // 剛才填的存放地點如果就是來源，清掉重填（不然一切過來就是「新地點不可與原地點相同」）
  if (from.value !== null && loc.value === from.value) location.value = ''
}

// 把游標放到下一個要填的欄位：改變地點先填要移到哪裡，其餘是數量；還沒選來源時不動
async function focusNext() {
  await nextTick()
  if (moving.value && from.value === null) return
  const el = moving.value && (!destChosen.value || problem.value) ? locInput.value : qtyInput.value
  el?.focus()
  el?.select()
}

function chooseWay(w) {
  if (w === way.value) return
  if (w === 'move') startMove()
  else {
    way.value = 'add'
    toNone.value = false
  }
  focusNext()
}

function pickFrom(l) {
  from.value = l
  focusNext()
}

// 改變地點：明確選「未指定」（再點一次取消）。選了就清掉欄位裡打到一半的字
function pickNone() {
  toNone.value = !toNone.value
  if (toNone.value) location.value = ''
  focusNext()
}

// 右邊「目前存放位置」點一下：填進存放地點；改變地點時點「未指定」那一列等於選了「未指定」
function pickPlace(l) {
  location.value = l
  toNone.value = moving.value && l === ''
}

async function submit() {
  if (!canSubmit.value) return
  busy.value = true
  try {
    let id = selected.value.id
    let name = selected.value.name
    if (moving.value) {
      const src = from.value
      await stock.apply({
        kind: 'TRANSFER',
        product_id: id,
        from_location_id: src,
        to_location_id: loc.value,
        quantity: qty.value,
        note: note.value,
      })
      ui.ok(`已移動：${name} ${fmtInt(qty.value)} 件（${locLabel(src)} → ${locLabel(loc.value)}）`)
    } else {
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
    }
    selected.value = null
    way.value = 'add'
    from.value = null
    location.value = ''
    toNone.value = false
    qtyText.value = ''
    qtyTouched.value = false
    note.value = ''
  } catch (err) {
    ui.error(err)
  } finally {
    busy.value = false
  }
}

// 從別的頁面帶著品項過來：ui.go('inbound', { productId, mode: 'move', from })
// 直接選好品項；mode 是 'move' 就切到改變地點，有給 from 的話連來源地點也選好
function applyParams(p) {
  const item = p?.productId ? products.get(p.productId) : null
  if (!item) return
  onSelect({ id: item.id, name: item.name, isNew: false })
  if (p.mode === 'move' && canMove.value) startMove(typeof p.from === 'string' ? p.from : null)
  focusNext()
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
  <PageHeader title="入庫" hint="選擇品項或輸入新品項名稱 → 確認數量 → 按 Enter 或「確認入庫」；已有庫存的品項也可以在這裡改變存放地點" />

  <div class="grid max-w-5xl grid-cols-5 gap-6">
    <form class="panel col-span-3 space-y-6 p-7" :style="toneStyle" @submit.prevent="submit">
      <ProductPicker ref="picker" :selected="selected" allow-create @select="onSelect" @clear="onClear" />

      <!-- 入庫方式：東西是從外面進來（新入庫），還是把現有庫存換個地點放（改變地點）。有庫存的既有品項才看得到 -->
      <div v-if="canMove">
        <label class="field-label">入庫方式</label>
        <div class="seg flex w-fit max-w-full" role="group" aria-label="入庫方式">
          <button type="button" class="seg-item" :class="moving ? '' : 'is-on'" :aria-pressed="!moving" @click="chooseWay('add')">新入庫</button>
          <button type="button" class="seg-item" :class="moving ? 'is-on' : ''" :aria-pressed="moving" @click="chooseWay('move')">改變地點（移動現有庫存）</button>
        </div>
        <p class="mt-2 pl-1 text-[12.5px] text-mute">{{ moving ? '把現有的庫存搬到別的地點，總數不變' : '新進的貨放進倉庫，總數會增加' }}</p>
      </div>

      <div v-if="moving">
        <label class="field-label">從哪個地點移出</label>
        <div class="grid max-h-56 grid-cols-2 gap-2 overflow-y-auto pr-1">
          <button
            v-for="r in rows"
            :key="r.location_id"
            type="button"
            class="pill justify-between px-4 py-2.5 text-left"
            :class="from === r.location_id ? 'pill-tone-on' : ''"
            :aria-pressed="from === r.location_id"
            @click="pickFrom(r.location_id)"
          >
            <span class="min-w-0 truncate text-[14px]" :class="r.location_id ? '' : 'italic opacity-80'">{{ locLabel(r.location_id) }}</span>
            <span class="num ml-2 text-lg">{{ fmtInt(r.quantity) }}</span>
          </button>
        </div>
        <p v-if="from === null" class="mt-1.5 pl-1 text-[12.5px] text-mute">這個品項放在好幾個地點，請點選要從哪裡移出</p>
      </div>

      <div>
        <label v-if="moving" class="field-label" for="in-loc">移到哪個地點</label>
        <label v-else class="field-label" for="in-loc">存放地點 <span class="font-normal text-mute">（可不填）</span></label>
        <ComboField
          id="in-loc"
          ref="locInput"
          v-model="location"
          :suggestions="moving ? otherLocations : stock.knownLocations"
          title="已使用過的存放地點"
          maxlength="50"
          :placeholder="!moving ? '例如：後倉第二排、A 櫃' : toNone ? '已選「未指定」；要放到別的地點請直接輸入' : '輸入新地點，或選擇既有地點'"
        />
        <!-- 改變地點而不想放在任何地點：要自己點「未指定」，欄位留空不會被當成未指定 -->
        <button
          v-if="moving && canPickNone"
          type="button"
          class="pill mt-2 px-3.5 py-1.5 text-[13.5px]"
          :class="toNone ? 'pill-tone-on' : ''"
          :aria-pressed="toNone"
          @click="pickNone"
        >未指定（不設定地點）</button>
        <p v-if="problem" class="mt-1.5 pl-1 text-[12.5px] text-signal">{{ problem }}</p>
        <template v-else-if="moving">
          <p v-if="toNone" class="mt-1.5 pl-1 text-[12.5px] text-mute">會移到「未指定」，不放在任何地點</p>
          <p v-else-if="!destChosen" class="mt-1.5 pl-1 text-[12.5px] text-mute">
            {{ canPickNone ? '請輸入或選擇要移到的地點；不想指定地點就點「未指定」' : '原地點是「未指定」，請輸入要移到哪個地點' }}
          </p>
        </template>
      </div>

      <div class="grid grid-cols-2 gap-4">
        <div>
          <label class="field-label" for="in-qty">數量</label>
          <input
            id="in-qty"
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
          <label class="field-label" for="in-note">備註</label>
          <input id="in-note" v-model="note" class="field h-12" placeholder="可不填" />
        </div>
      </div>

      <div
        v-if="insufficient"
        class="rounded-2xl bg-signal-soft px-5 py-3.5 text-[15px] font-bold text-signal ring-2 ring-signal/50"
        role="alert"
      >
        庫存不足！此存放地點剩餘 {{ fmtInt(available) }} 件，不可移動 {{ fmtInt(qty) }} 件
      </div>

      <button type="submit" class="w-full py-3.5 text-[15.5px]" :class="moving ? 'btn-violet' : 'btn-go'" :disabled="!canSubmit">
        {{ moving ? '確認移動' : '確認入庫' }}
      </button>
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

      <!-- 改變地點：來源少多少、目的地多多少，各一張；新入庫：放進去的那個地點會變成多少 -->
      <template v-if="moving">
        <QtyPreview
          v-if="from !== null"
          :before="available"
          :after="qty && !insufficient ? available - qty : null"
          :caption="`${locLabel(from)} 現有`"
          after-label="移出後"
          unit="件"
          tone="violet"
          delta
          :danger="insufficient"
        />
        <QtyPreview
          v-if="from !== null && destChosen && !problem"
          :before="before"
          :after="qty && !insufficient ? before + qty : null"
          :caption="`${locLabel(loc)} 現有`"
          after-label="移入後"
          unit="件"
          tone="violet"
          delta
        />
      </template>
      <QtyPreview
        v-else-if="selected"
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
          此品項目前存放位置<span v-if="rows.length" class="font-normal">（{{ moving ? '點一下設成要移到的地點' : '點一下填入存放地點' }}）</span>
        </h3>
        <ul v-if="rows.length" class="space-y-1.5">
          <li v-for="r in rows" :key="r.location_id">
            <button
              type="button"
              class="place-row flex w-full items-center justify-between gap-3 rounded-full px-3 py-1.5 text-left hover:bg-hover disabled:cursor-default disabled:hover:bg-transparent"
              :class="isSource(r) ? '' : isDest(r) ? 'is-on font-semibold' : ''"
              :disabled="isSource(r)"
              @click="pickPlace(r.location_id)"
            >
              <span class="min-w-0 truncate" :class="r.location_id ? '' : 'italic'">{{ locLabel(r.location_id) }}</span>
              <span class="flex shrink-0 items-baseline gap-2">
                <span v-if="isSource(r)" class="text-[12px] font-semibold" :style="{ color: 'var(--tone)' }">從這裡移出</span>
                <span class="num">{{ fmtInt(r.quantity) }}</span>
              </span>
            </button>
          </li>
        </ul>
        <p v-else class="text-[13px] text-mute">目前沒有庫存</p>
      </div>
    </aside>
  </div>
</template>

<style scoped>
/* 右邊「目前存放位置」：現在要放進去（或移入）的那個地點淡淡標出來 */
.place-row.is-on {
  background: color-mix(in oklab, var(--tone) 15%, transparent);
}
</style>
