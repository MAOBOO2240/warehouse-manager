<script setup>
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import ActionBadge from '../components/ActionBadge.vue'
import DateField from '../components/DateField.vue'
import SelectMenu from '../components/SelectMenu.vue'
import { useStockStore } from '../stores/stock'
import { useUiStore } from '../stores/ui'
import { useProjectStore } from '../stores/project'
import { ACTIONS, ACTIVE_ACTIONS, actionLabel, fmtDelta, fmtInt, fmtTime, localDate, locLabel, todayStamp } from '../utils/format'
import { saveCsv } from '../utils/csv'

const stock = useStockStore()
const ui = useUiStore()
const project = useProjectStore()

const DISPLAY_LIMIT = 500
// 從來沒有填過存放地點時，品項底下不顯示地點（全部都是「未指定」）
const hasLoc = computed(() => stock.logs.some((l) => l.location_id))

const type = ref('')
const typeOptions = [{ value: '', label: '全部' }, ...ACTIVE_ACTIONS.map((key) => ({ value: key, label: ACTIONS[key].label }))]
const from = ref('')
const to = ref('')
const q = ref('')

// 篩選：操作類型、起訖日期（本機日期）、品項名稱 / 存放地點 / 原因 / 備註
const filtered = computed(() => {
  const k = q.value.trim().toLowerCase()
  return stock.logs
    .filter((l) => {
      if (type.value && l.action_type !== type.value) return false
      const d = localDate(l.timestamp)
      if (from.value && d < from.value) return false
      if (to.value && d > to.value) return false
      if (k) {
        const hay = [l.product_name, l.location_id, l.note, l.reason]
        if (!hay.some((v) => String(v ?? '').toLowerCase().includes(k))) return false
      }
      return true
    })
    .reverse()
})

function reset() {
  type.value = ''
  from.value = ''
  to.value = ''
  q.value = ''
}

/* ---- 合計：符合條件的紀錄總共進出多少（不受畫面顯示筆數的上限影響） ---- */
const SUMMED = ['INBOUND', 'OUTBOUND', 'DAMAGE']
// 用真正的減號（−），寬度和加號一樣，數字才對得齊
const signed = (n) => fmtDelta(n).replace('-', '−')
/**
 * 移動地點每次記一出一入兩筆，流水號相連（移出在前、移入在後）：用「移出那一筆的流水號」當這一次移動的代號。
 * 篩選之後只剩其中一筆（例如搜尋只對到移出的那個地點）也照樣算一次，兩筆都在只算一次。
 */
function moveKey(l) {
  if (l.delta < 0) return String(l.id)
  const m = /^(.*?)(\d+)$/.exec(String(l.id))
  // 流水號是「LOG-日期-四位數以上的序號」
  return m ? m[1] + String(Number(m[2]) - 1).padStart(4, '0') : String(l.id)
}
const summary = computed(() => {
  const qty = { INBOUND: 0, OUTBOUND: 0, DAMAGE: 0 }
  const count = { INBOUND: 0, OUTBOUND: 0, DAMAGE: 0 }
  const moves = new Set()
  for (const l of filtered.value) {
    if (SUMMED.includes(l.action_type)) {
      qty[l.action_type] += l.delta
      count[l.action_type] += 1
    } else if (l.action_type === 'TRANSFER') {
      moves.add(moveKey(l))
    }
  }
  const figure = (key, tone) => ({ key, label: actionLabel(key), tone, value: signed(qty[key]), unit: '件', count: count[key] })
  const list = [figure('INBOUND', 'go'), figure('OUTBOUND', 'accent'), figure('DAMAGE', 'signal')]
  // 從來沒有用過存放地點的專案不提「移動地點」（一定是 0 次）
  if (hasLoc.value) list.push({ key: 'TRANSFER', label: actionLabel('TRANSFER'), tone: 'violet', value: fmtInt(moves.size), unit: '次', count: null })
  return list
})

/* ---- 依日期分組：每一天開頭一條色帶，寫日期、星期與當天筆數 ---- */
// 「今天／昨天」的標示：程式一直開著、過了午夜也要跟著換，所以每分鐘對一次時間
const now = ref(Date.now())
let clock = 0
onMounted(() => (clock = setInterval(() => (now.value = Date.now()), 60_000)))
onBeforeUnmount(() => clearInterval(clock))
function dayFromNow(shift) {
  const d = new Date(now.value)
  d.setDate(d.getDate() + shift)
  return localDate(d.toISOString())
}
const today = computed(() => dayFromNow(0))
const yesterday = computed(() => dayFromNow(-1))

const WEEKDAYS = ['日', '一', '二', '三', '四', '五', '六']
/** 本機日期「2026-11-17」→「2026/11/17（二）」 */
function dayText(day) {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day)
  if (!m) return '日期不明'
  const weekday = WEEKDAYS[new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3])).getDay()]
  return `${m[1]}/${m[2]}/${m[3]}（${weekday}）`
}

/**
 * 畫面上的列（最新的在前，最多 DISPLAY_LIMIT 筆）依本機日期分組。
 * total 是那一天符合條件的筆數；最後一天可能因為顯示上限只列出其中一部分。
 */
const groups = computed(() => {
  const out = []
  let g = null
  let left = DISPLAY_LIMIT
  for (const l of filtered.value) {
    const day = localDate(l.timestamp)
    if (!g || g.day !== day) {
      if (left <= 0) break
      const tag = day === today.value ? '今天' : day === yesterday.value ? '昨天' : ''
      g = { key: l.id, day, label: dayText(day), tag, rows: [], total: 0 }
      out.push(g)
    }
    g.total += 1
    if (left > 0) {
      g.rows.push(l)
      left -= 1
    }
  }
  return out
})

/* ---- 太長的品項名稱、原因、備註先截成兩行；真的被截到的那一列會多一顆「展開全文」，點一下整列展開 ---- */
const scroller = ref(null)
const clamped = shallowRef(new Set()) // 有文字被截短的列（流水號）
const opened = shallowRef(new Set()) // 已展開的列
const CLAMP = 'line-clamp-2'
const clampOf = (id) => (opened.value.has(id) ? '' : CLAMP)

/**
 * 找出哪幾列的文字真的被截短了。被截掉的部分，各版本的瀏覽器核心量法不一樣，
 * 所以另外排一份「不截短、一樣寬」的文字來比高度（兩邊都是未縮放的 CSS px，不受文字大小設定影響）。
 */
function measure() {
  const root = scroller.value
  if (!root) return
  // 字數 × 字寬還排不滿一行的一定沒被截，不用量
  const els = [...root.querySelectorAll('[data-clamp]')].filter(
    (el) => !opened.value.has(el.dataset.clamp) && el.textContent.length * 15 > el.clientWidth,
  )
  const next = new Set()
  if (els.length) {
    const box = document.createElement('div')
    box.style.cssText = 'position:fixed;top:0;left:-99999px;visibility:hidden;pointer-events:none'
    const copies = els.map((el) => {
      const copy = document.createElement('div')
      copy.className = el.className.replace(CLAMP, '')
      // 窄 1px：寧可多出現一顆按鈕，也不要漏掉剛好被截到的
      copy.style.width = `${Math.max(1, el.clientWidth - 1)}px`
      copy.textContent = el.textContent
      box.appendChild(copy)
      return copy
    })
    document.body.appendChild(box)
    els.forEach((el, i) => {
      if (copies[i].offsetHeight > el.clientHeight + 1) next.add(el.dataset.clamp)
    })
    box.remove()
  }
  const prev = clamped.value
  if (next.size !== prev.size || [...next].some((id) => !prev.has(id))) clamped.value = next
}

function toggle(id) {
  const next = new Set(opened.value)
  if (!next.delete(id)) next.add(id)
  opened.value = next
}

// 內容換了、展開或收合之後重新量；視窗或文字大小改變時欄寬會變，也要重新量
watch([groups, opened], measure, { flush: 'post' })
let ro = null
let raf = 0
onMounted(() => {
  measure()
  if (typeof ResizeObserver === 'undefined' || !scroller.value) return
  ro = new ResizeObserver(() => {
    cancelAnimationFrame(raf)
    raf = requestAnimationFrame(measure)
  })
  ro.observe(scroller.value)
})
onBeforeUnmount(() => {
  ro?.disconnect()
  cancelAnimationFrame(raf)
})

async function exportCsv() {
  try {
    const range = from.value || to.value ? `_${from.value || '起'}至${to.value || '今'}` : ''
    const path = await saveCsv(
      `${project.current}_歷史紀錄${range}_${todayStamp()}.csv`,
      ['流水號', '時間', '類型', '品項', '存放地點', '異動數量', '異動後結存', '原因', '備註'],
      [...filtered.value].reverse().map((l) => [
        l.id, fmtTime(l.timestamp), actionLabel(l.action_type), l.product_name,
        locLabel(l.location_id), l.delta, l.balance_after, l.reason, l.note,
      ]),
    )
    if (path) ui.ok(`已匯出 ${filtered.value.length} 筆歷史紀錄：\n${path}`)
  } catch (err) {
    ui.error(err)
  }
}
</script>

<template>
  <PageHeader title="歷史紀錄" hint="所有庫存異動的完整紀錄，不可修改或刪除">
    <button type="button" class="btn-primary" :disabled="!filtered.length" @click="exportCsv">匯出 CSV</button>
  </PageHeader>

  <div class="panel mb-5 flex flex-wrap items-end gap-4 p-5">
    <div>
      <label class="field-label" for="lg-type">操作類型</label>
      <SelectMenu id="lg-type" v-model="type" :options="typeOptions" title="操作類型" class="w-44" />
    </div>
    <DateField id="lg-from" v-model="from" label="起始日期" class="w-48" />
    <DateField id="lg-to" v-model="to" label="結束日期" class="w-48" />
    <div class="min-w-52 flex-1">
      <label class="field-label" for="lg-q">{{ hasLoc ? '品項或存放地點' : '品項、原因或備註' }}</label>
      <input id="lg-q" v-model="q" class="field rounded-full" placeholder="輸入關鍵字" />
    </div>
    <button type="button" class="btn-ghost mb-0.5" :disabled="!type && !from && !to && !q" @click="reset">清除條件</button>
  </div>

  <div class="panel overflow-hidden">
    <!-- 合計：符合條件的紀錄總共入庫、出貨、損壞多少件，移動過幾次地點 -->
    <div
      class="flex flex-wrap items-baseline gap-x-6 gap-y-1.5 px-5 py-3 text-[14px]"
      style="box-shadow: inset 0 -1px 0 var(--color-line)"
      role="group"
      aria-label="符合條件的紀錄合計"
    >
      <span class="text-[13px] font-semibold text-mute" aria-hidden="true">合計</span>
      <div v-for="s in summary" :key="s.key" class="flex items-baseline gap-1.5 whitespace-nowrap">
        <span class="mr-0.5 h-2 w-2 shrink-0 self-center rounded-full" :style="{ background: `var(--color-${s.tone})` }" aria-hidden="true" />
        <span class="text-ink-2">{{ s.label }}</span>
        <span class="num text-[16px] font-semibold text-ink">{{ s.value }}</span>
        <span class="text-ink-2">{{ s.unit }}</span>
        <span v-if="s.count !== null" class="text-[13px] text-mute">（<span class="num">{{ fmtInt(s.count) }}</span> 筆）</span>
      </div>
    </div>

    <!-- 高度：預設視窗下整頁剛好放得下（只有表格自己捲動）；視窗很矮時至少留 20rem，改成整頁一起捲 -->
    <div ref="scroller" class="max-h-[max(20rem,calc(100vh-27.5rem))] overflow-x-hidden overflow-y-auto">
      <table class="table-base table-fixed">
        <!-- 時間、類型、異動、結存固定寬度；品項與原因／備註平分剩下的寬度，視窗再窄也不用左右捲動 -->
        <colgroup>
          <col class="w-[148px]" />
          <col class="w-[92px]" />
          <col />
          <col class="w-[76px]" />
          <col class="w-[76px]" />
          <col />
        </colgroup>
        <thead>
          <tr>
            <th class="pr-2">時間</th>
            <th class="px-2">類型</th>
            <th class="px-2">品項</th>
            <th class="px-2 text-right">異動</th>
            <th class="px-2 text-right">結存</th>
            <th class="pl-3">原因／備註</th>
          </tr>
        </thead>
        <template v-for="g in groups" :key="g.key">
          <!-- 每一天開頭的日期色帶（自己一個 tbody，底下的列才會從第一列開始算斑馬底色） -->
          <tbody>
            <tr class="day-row">
              <td colspan="6" class="py-2 text-[13px] leading-normal font-semibold text-ink-2">
                <div class="flex items-baseline gap-2">
                  <span v-if="g.tag" class="font-bold text-ink">{{ g.tag }}</span>
                  <span class="num">{{ g.label }}</span>
                  <span class="ml-auto font-normal whitespace-nowrap">
                    <span class="num">{{ fmtInt(g.total) }}</span> 筆<template v-if="g.rows.length < g.total">（畫面顯示其中 <span class="num">{{ fmtInt(g.rows.length) }}</span> 筆）</template>
                  </span>
                </div>
              </td>
            </tr>
          </tbody>
          <tbody>
            <tr v-for="l in g.rows" :key="l.id">
              <td class="py-2.5 pr-2 whitespace-nowrap">
                <div class="num leading-snug">{{ fmtTime(l.timestamp) }}</div>
                <div class="num mt-0.5 text-[11.5px] leading-snug text-mute">{{ l.id }}</div>
              </td>
              <td class="px-2 py-2.5"><ActionBadge :type="l.action_type" /></td>
              <td class="px-2 py-2.5">
                <div class="text-[14.5px] leading-snug font-semibold break-words" :class="clampOf(l.id)" :data-clamp="l.id" :title="l.product_name">{{ l.product_name }}</div>
                <div v-if="hasLoc" class="mt-1 flex items-start gap-1 text-[12.5px] leading-snug text-mute">
                  <svg viewBox="0 0 24 24" class="mt-[2.5px] h-3 w-3 shrink-0" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <path d="M12 21s-6.5-5.6-6.5-11a6.5 6.5 0 0 1 13 0c0 5.4-6.5 11-6.5 11z" />
                    <circle cx="12" cy="10" r="2.3" />
                  </svg>
                  <span class="min-w-0 break-words" :class="l.location_id ? '' : 'italic'">{{ locLabel(l.location_id) }}</span>
                </div>
              </td>
              <td class="num px-2 py-2.5 text-right text-[15px] whitespace-nowrap" :class="l.delta < 0 ? 'text-signal' : 'text-go'">{{ fmtDelta(l.delta) }}</td>
              <td class="num px-2 py-2.5 text-right whitespace-nowrap">{{ fmtInt(l.balance_after) }}</td>
              <td class="py-2.5 pl-3">
                <div v-if="l.reason" class="text-[13.5px] leading-snug break-words" :class="clampOf(l.id)" :data-clamp="l.id" :title="l.reason">{{ l.reason }}</div>
                <div v-if="l.note" class="text-[12.5px] leading-snug break-words text-mute" :class="[clampOf(l.id), l.reason ? 'mt-0.5' : '']" :data-clamp="l.id" :title="l.note">{{ l.note }}</div>
                <button
                  v-if="clamped.has(l.id) || opened.has(l.id)"
                  type="button"
                  class="mt-1.5 rounded-full bg-raised px-3 py-1 text-[12.5px] leading-snug font-semibold text-ink-2 transition-colors hover:bg-hover hover:text-ink"
                  :aria-expanded="opened.has(l.id)"
                  @click="toggle(l.id)"
                >{{ opened.has(l.id) ? '收合' : '展開全文' }}</button>
              </td>
            </tr>
          </tbody>
        </template>
        <tbody v-if="!filtered.length">
          <tr>
            <td colspan="6" class="py-10 text-center text-mute">{{ stock.logs.length ? '沒有符合條件的紀錄' : '尚無任何異動紀錄' }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <div class="px-6 py-3.5 text-right text-[13px] text-mute" style="box-shadow: inset 0 1px 0 var(--color-line)">
      符合條件 <span class="num">{{ fmtInt(filtered.length) }}</span> 筆
      <span v-if="filtered.length > DISPLAY_LIMIT" class="text-mute">（畫面顯示最新 {{ DISPLAY_LIMIT }} 筆，上方合計與匯出 CSV 包含全部）</span>
    </div>
  </div>
</template>

<style scoped>
/* 每一天開頭的日期列：安靜的一條色帶，不跟著斑馬底色、滑鼠停留變色 */
.day-row td {
  background: color-mix(in oklab, var(--color-ink) 7%, var(--color-panel));
}
</style>
