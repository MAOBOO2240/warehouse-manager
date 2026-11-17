<script setup>
// 近幾天的進出圖：每天一根長條，以 0 為基準線——入庫往上長；出貨與損壞往下疊（出貨貼著基準線，損壞接在下面）。
// 上下共用同一個刻度：一樣的高度就是一樣的件數，所以基準線的位置會跟著資料上下移動，不會有第二條座標軸。
// 滑鼠移到某一天（或用鍵盤移過去、觸控點一下）會顯示那天的數字；同樣的數字切到「表格」也看得到，不必靠滑鼠停留。
import { computed, ref } from 'vue'
import { fmtDelta, fmtInt } from '../../utils/format'

const props = defineProps({
  /** [{ date: 'YYYY-MM-DD', label: 'M/D', inbound, outbound, damage }]，由舊到新，最後一筆是今天 */
  days: { type: Array, required: true },
  /** 圖例後面的說明；沒有用存放地點的專案傳空字串（畫面上不出現「地點」這些字） */
  note: { type: String, default: '不含移動地點' },
})

// 三種長條各有固定的顏色（深淺色主題都挑過，色弱也分得出來）；文字一律用一般的文字顏色，不跟著長條上色
const SERIES = [
  { key: 'inbound', name: '入庫', fill: 'bg-chart-in' },
  { key: 'outbound', name: '出貨', fill: 'bg-chart-out' },
  { key: 'damage', name: '損壞', fill: 'bg-chart-damage' },
]
const WEEKDAYS = ['日', '一', '二', '三', '四', '五', '六']

const VIEWS = [
  { id: 'chart', label: '圖表' },
  { id: 'table', label: '表格' },
]
const view = ref('chart')
const segTone = { '--tone': 'var(--color-accent)', '--tone-fg': 'var(--color-on-accent)' }

const count = (v) => {
  const n = Number(v)
  return Number.isFinite(n) && n > 0 ? n : 0
}

/** 每天整理好的數字 */
const rows = computed(() => {
  const last = props.days.length - 1
  return props.days.map((d, i) => {
    const inbound = count(d.inbound)
    const outbound = count(d.outbound)
    const damage = count(d.damage)
    const [y, m, day] = String(d.date).split('-').map(Number)
    const wd = WEEKDAYS[new Date(y, m - 1, day).getDay()]
    return {
      date: d.date,
      label: d.label,
      full: wd ? `${d.label}（${wd}）` : d.label,
      today: i === last,
      inbound,
      outbound,
      damage,
      out: outbound + damage, // 往下那一疊的總長
      net: inbound - outbound - damage,
    }
  })
})
/** 表格：新的在上面 */
const tableRows = computed(() => [...rows.value].reverse())
const totals = computed(() => {
  const t = { inbound: 0, outbound: 0, damage: 0, net: 0 }
  for (const r of rows.value) {
    t.inbound += r.inbound
    t.outbound += r.outbound
    t.damage += r.damage
    t.net += r.net
  }
  return t
})

/** 格線的間距：從 1、2、5、10、20、50… 裡挑最小的，讓比較長的那一邊最多兩條格線 */
function gridStep(top) {
  for (let pow = 1; ; pow *= 10) {
    for (const m of [1, 2, 5]) {
      if (Math.floor(top / (m * pow)) <= 2) return m * pow
    }
  }
}

/**
 * 刻度：往上的範圍是入庫最多的那一天，往下是「出貨＋損壞」最多的那一天，兩邊用同一個比例。
 * 位置都用百分比（從畫長條的範圍的上緣算起）。全部都是 0 時回傳 null（不畫圖）。
 */
const scale = computed(() => {
  let up = 0
  let down = 0
  for (const r of rows.value) {
    up = Math.max(up, r.inbound)
    down = Math.max(down, r.out)
  }
  const span = up + down
  if (!span) return null
  const pct = (v) => (v / span) * 100
  const top = Math.max(up, down)
  const step = gridStep(top)
  // 刻度的數字：一萬以上寫成「1萬、1.5萬」，左邊才放得下
  const text = top >= 10000 ? (v) => `${+(v / 10000).toFixed(1)}萬` : fmtInt
  const ticks = [{ id: 0, pos: pct(up), text: '0' }]
  for (let v = step; v <= up; v += step) ticks.push({ id: v, pos: pct(up - v), text: text(v) })
  for (let v = step; v <= down; v += step) ticks.push({ id: -v, pos: pct(up + v), text: text(v) })
  return { pct, base: pct(up), ticks }
})

/** 每天的長條與數字的位置 */
const bars = computed(() => {
  const s = scale.value
  if (!s) return []
  return rows.value.map((r) => ({
    ...r,
    upBar: { bottom: `${100 - s.base}%`, height: `${s.pct(r.inbound)}%` },
    downBar: { top: `${s.base}%`, height: `${s.pct(r.out)}%` },
    upLabel: { bottom: `calc(${100 - s.base + s.pct(r.inbound)}% + 4px)` },
    downLabel: { top: `calc(${s.base + s.pct(r.out)}% + 4px)` },
  }))
})

/* ---- 正在看的那一天：滑鼠移上去的優先，其次是鍵盤（或點一下）選到的 ---- */
const hovered = ref(null)
const focused = ref(null)
const active = computed(() => {
  const i = hovered.value ?? focused.value
  return i !== null && i < rows.value.length ? i : null
})

/** 提示框：放在那一天的旁邊（左半邊的日子放右邊、右半邊的放左邊），不擋住那一天的長條，也不會超出圖的範圍 */
const tip = computed(() => {
  const i = active.value
  if (i === null) return null
  const n = rows.value.length
  const style = i < n / 2 ? { left: `calc(${((i + 1) / n) * 100}% + 6px)` } : { right: `calc(${((n - i) / n) * 100}% + 6px)` }
  return { row: rows.value[i], style }
})

/** 圖很窄的時候，日期每隔一天才標一個（從今天往回數） */
const thin = (i) => (rows.value.length - 1 - i) % 2 === 1
const xClass = (i) => ({
  'is-thin': thin(i),
  'is-active': active.value === i,
  // 正在看一個沒標日期的日子：暫時標出它，兩旁的日期先讓開
  'is-near': active.value !== null && thin(active.value) && Math.abs(active.value - i) === 1,
})

const describe = (r) =>
  `${r.today ? '今天，' : ''}${r.full}：入庫 ${fmtInt(r.inbound)} 件、出貨 ${fmtInt(r.outbound)} 件、損壞 ${fmtInt(r.damage)} 件`

// 鍵盤：左右鍵換一天，Home／End 跳到最舊的一天／今天，Esc 離開
function onKey(e) {
  const el = e.currentTarget
  if (e.key === 'Escape') return el.blur()
  const to = {
    ArrowLeft: el.previousElementSibling,
    ArrowRight: el.nextElementSibling,
    Home: el.parentElement.firstElementChild,
    End: el.parentElement.lastElementChild,
  }[e.key]
  if (to === undefined) return
  e.preventDefault()
  to?.focus()
}
</script>

<template>
  <div class="flex min-h-0 flex-col">
    <div class="flex items-center gap-3">
      <h2 class="card-title min-w-0 truncate">近 {{ days.length }} 天進出</h2>
      <div v-if="scale" class="seg ml-auto shrink-0" role="group" aria-label="顯示方式" :style="segTone">
        <button
          v-for="v in VIEWS"
          :key="v.id"
          type="button"
          class="seg-item px-3 py-1 text-[13px]"
          :class="view === v.id ? 'is-on' : ''"
          :aria-pressed="view === v.id"
          @click="view = v.id"
        >{{ v.label }}</button>
      </div>
    </div>

    <p v-if="!scale" class="flex min-h-[220px] flex-1 items-center justify-center text-center text-[13.5px] text-mute">
      這 {{ days.length }} 天沒有入庫、出貨或損壞
    </p>

    <template v-else>
      <!-- 圖例（表格的欄位名稱已經寫明，只留說明） -->
      <div v-if="view === 'chart' || note" class="mt-2.5 flex flex-wrap items-center gap-x-4 gap-y-1 text-[12.5px] text-ink-2">
        <template v-if="view === 'chart'">
          <span v-for="s in SERIES" :key="s.key" class="inline-flex items-center gap-1.5">
            <span class="h-2.5 w-2.5 rounded-[3px]" :class="s.fill" aria-hidden="true" />{{ s.name }}
          </span>
        </template>
        <span v-if="note" class="text-mute">{{ note }}</span>
      </div>

      <!-- 圖表：高度約 220px（含最下面的日期），卡片被拉高時跟著長高 -->
      <div
        v-if="view === 'chart'"
        class="relative mt-2 flex min-h-[220px] flex-1"
        role="group"
        :aria-label="`近 ${days.length} 天每天的入庫、出貨、損壞件數`"
      >
        <!-- 左邊的刻度 -->
        <div class="relative w-[34px] shrink-0" aria-hidden="true">
          <span class="absolute top-0 right-2 text-[11px] leading-3 text-mute">件</span>
          <div class="fc-scale">
            <span
              v-for="t in scale.ticks"
              :key="t.id"
              class="num absolute right-2 h-3 text-[11px] leading-3 whitespace-nowrap text-mute"
              :style="{ top: `calc(${t.pos}% - 6px)` }"
            >{{ t.text }}</span>
          </div>
        </div>

        <div class="fc-area relative min-w-0 flex-1">
          <!-- 每天一欄：整欄（不只長條）都可以停留、點選或用鍵盤選到 -->
          <div class="absolute inset-0 flex">
            <div
              v-for="(r, i) in bars"
              :key="r.date"
              class="relative min-w-0 flex-1 rounded-sm focus-visible:-outline-offset-2"
              :class="active === i ? 'bg-hover/80' : ''"
              tabindex="0"
              role="img"
              :aria-label="describe(r)"
              @pointerenter="hovered = i"
              @pointerleave="hovered = null"
              @focus="focused = i"
              @blur="focused = null"
              @keydown="onKey"
            >
              <div class="fc-scale">
                <!-- 入庫：從基準線往上；基準線那一端是直角，頂端 4px 圓角 -->
                <div
                  v-if="r.inbound"
                  class="fc-bar absolute inset-x-0 z-[1] mx-auto min-h-[2px] w-4 max-w-[70%] rounded-t-[4px] bg-chart-in"
                  :style="r.upBar"
                />
                <!-- 出貨＋損壞：從基準線往下疊，兩段之間留 2px 空隙（不是畫框線） -->
                <div
                  v-if="r.out"
                  class="absolute inset-x-0 z-[1] mx-auto flex min-h-[2px] w-4 max-w-[70%] flex-col gap-[2px]"
                  :style="r.downBar"
                >
                  <div
                    v-if="r.outbound"
                    class="fc-bar min-h-[2px] basis-0 bg-chart-out"
                    :class="r.damage ? '' : 'rounded-b-[4px]'"
                    :style="{ flexGrow: r.outbound }"
                  />
                  <div v-if="r.damage" class="fc-bar min-h-[2px] basis-0 rounded-b-[4px] bg-chart-damage" :style="{ flexGrow: r.damage }" />
                </div>
                <!-- 長條頂端的數字：平常只標今天；正在看某一天時只標那一天（兩天相鄰時數字才不會疊在一起）。
                     往下那一疊標的是出貨＋損壞的合計 -->
                <template v-if="active === i || (r.today && active === null)">
                  <div v-if="r.inbound" class="absolute inset-x-0 z-[2] flex justify-center" :style="r.upLabel">
                    <span class="num shrink-0 text-[11.5px] leading-none font-semibold whitespace-nowrap text-ink">{{ fmtInt(r.inbound) }}</span>
                  </div>
                  <div v-if="r.out" class="absolute inset-x-0 z-[2] flex justify-center" :style="r.downLabel">
                    <span class="num shrink-0 text-[11.5px] leading-none font-semibold whitespace-nowrap text-ink">{{ fmtInt(r.out) }}</span>
                  </div>
                </template>
              </div>
              <div class="fc-x absolute inset-x-0 bottom-0 flex h-6 items-center justify-center" :class="xClass(i)">
                <span
                  class="num shrink-0 text-[11px] leading-none whitespace-nowrap"
                  :class="r.today ? 'font-bold text-ink' : active === i ? 'text-ink' : 'text-mute'"
                >{{ r.today ? '今天' : r.label }}</span>
              </div>
            </div>
          </div>

          <!-- 格線（最多上下各兩條，實線）：畫在欄位底色之上、長條之下 -->
          <div class="fc-scale pointer-events-none" aria-hidden="true">
            <template v-for="t in scale.ticks" :key="t.id">
              <div v-if="t.id" class="absolute inset-x-0 border-t border-line/70" :style="{ top: `${t.pos}%` }" />
            </template>
          </div>
          <!-- 基準線（0）：比格線明顯一階，壓在長條上面，往上與往下的長條之間看得到這條線 -->
          <div class="fc-scale pointer-events-none z-[2]" aria-hidden="true">
            <div class="absolute inset-x-0 border-t border-line" :style="{ top: `${scale.base}%` }" />
          </div>

          <!-- 提示框：數字在前、名稱在後，前面一小段色條對應長條的顏色 -->
          <div
            v-if="tip"
            class="menu-panel pointer-events-none absolute top-1 z-[3] min-w-[132px] rounded-lg px-3.5 py-2.5"
            :style="tip.style"
            aria-hidden="true"
          >
            <div class="text-[12px] whitespace-nowrap text-mute">{{ tip.row.today ? '今天・' : '' }}{{ tip.row.full }}</div>
            <div class="mt-1.5 grid grid-cols-[auto_auto_1fr] items-center gap-x-2 gap-y-1">
              <template v-for="s in SERIES" :key="s.key">
                <span class="h-[3px] w-3 rounded-full" :class="s.fill" />
                <span class="num text-right text-[14px] leading-5 font-semibold text-ink">{{ fmtInt(tip.row[s.key]) }}</span>
                <span class="text-[12.5px] leading-5 whitespace-nowrap text-ink-2">{{ s.name }}</span>
              </template>
            </div>
          </div>
        </div>
      </div>

      <!-- 表格：和圖表同一份數字，新的在上面 -->
      <table v-else class="table-base fc-table mt-2 text-[13.5px]">
        <caption class="sr-only">近 {{ days.length }} 天每天的入庫、出貨、損壞件數與淨變化</caption>
        <thead>
          <tr>
            <th>日期</th>
            <th v-for="s in SERIES" :key="s.key" class="text-right">{{ s.name }}</th>
            <th class="text-right">淨變化</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in tableRows" :key="r.date">
            <td>
              <span class="num whitespace-nowrap">{{ r.full }}</span>
              <span v-if="r.today" class="chip ml-2 px-2 py-0 text-[12px]">今天</span>
            </td>
            <td v-for="s in SERIES" :key="s.key" class="num text-right" :class="r[s.key] ? '' : 'text-mute'">{{ fmtInt(r[s.key]) }}</td>
            <td class="num text-right" :class="r.net ? '' : 'text-mute'">{{ fmtDelta(r.net) }}</td>
          </tr>
        </tbody>
        <tfoot>
          <tr>
            <td>合計</td>
            <td v-for="s in SERIES" :key="s.key" class="num text-right">{{ fmtInt(totals[s.key]) }}</td>
            <td class="num text-right">{{ fmtDelta(totals.net) }}</td>
          </tr>
        </tfoot>
      </table>
    </template>
  </div>
</template>

<style scoped>
/* 畫長條的範圍：上下各留 18px 放長條頂端的數字，最下面 24px 是日期 */
.fc-scale {
  position: absolute;
  inset: 18px 0 42px;
}
/* 以圖的寬度為準（給下面的容器查詢判斷用） */
.fc-area {
  container-type: inline-size;
}
/* 長條資料端的圓角用正圓弧（全站預設是 squircle，4px 的小圓角看起來會像直角） */
.fc-bar {
  corner-shape: round;
}
/*
  圖太窄（視窗縮到最小、或文字放大）時，日期每隔一天標一個才不會擠在一起；
  移到沒標日期的那一天時暫時標出它，兩旁的先讓開。
*/
@container (max-width: 475px) {
  .fc-x.is-thin:not(.is-active),
  .fc-x.is-near {
    visibility: hidden;
  }
}
/* 表格：比一般的資料表緊湊（卡片不會因為切到表格就拉得很長），不用捲動就看得完；最下面一列是合計 */
.fc-table :is(th, td) {
  padding: 3px 8px;
  line-height: 1.5;
}
.fc-table th {
  position: static;
  padding-block: 6px;
}
.fc-table tfoot td {
  font-weight: 600;
  box-shadow: inset 0 1px 0 var(--color-line);
}
</style>
