<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import ActionBadge from '../components/ActionBadge.vue'
import StatTile from '../components/StatTile.vue'
import FlowChart from '../components/charts/FlowChart.vue'
import RankBars from '../components/charts/RankBars.vue'
import { useProductStore } from '../stores/products'
import { locCompare, useStockStore } from '../stores/stock'
import { useUiStore } from '../stores/ui'
import { fmtDelta, fmtInt, fmtTime, localDate, locLabel } from '../utils/format'

const products = useProductStore()
const stock = useStockStore()
const ui = useUiStore()

// 統計卡的圖示（24×24 的 SVG 路徑）；入庫、出庫和左側選單的圖示相同
const ICONS = {
  layers: 'M12 3 3.5 7.5 12 12l8.5-4.5zM3.5 12 12 16.5l8.5-4.5M3.5 16.5 12 21l8.5-4.5',
  pin: 'M12 21s-6.5-5.6-6.5-11a6.5 6.5 0 0 1 13 0c0 5.4-6.5 11-6.5 11zM12 7.7a2.3 2.3 0 1 0 0 4.6 2.3 2.3 0 0 0 0-4.6z',
  box: 'M12 3 4 7.2v9.6L12 21l8-4.2V7.2zM4 7.2l8 4.3 8-4.3M12 11.5V21',
  inbound: 'M12 3.5v10M7.5 9 12 13.5 16.5 9M4 14.5v3a2.5 2.5 0 0 0 2.5 2.5h11a2.5 2.5 0 0 0 2.5-2.5v-3',
  outbound: 'M12 13.5v-10M7.5 8 12 3.5 16.5 8M4 14.5v3a2.5 2.5 0 0 0 2.5 2.5h11a2.5 2.5 0 0 0 2.5-2.5v-3',
}

const FLOW_DAYS = 14 // 進出圖看最近幾天
const TOP_ITEMS = 8 // 「庫存最多的品項」列幾名
const RECENT = 8 // 「最近異動」列幾筆
const SCROLL_ROWS = 8 // 排行長條超過這個列數會在卡片裡捲動（見 RankBars），標題旁要寫總共有幾個

// 今天的日期（本機）。程式開著過夜時每分鐘看一次有沒有換日，「今日」的數字與圖表才不會停在昨天
const today = ref(localDate(new Date().toISOString()))
let dayTimer = null
onMounted(() => {
  dayTimer = setInterval(() => {
    today.value = localDate(new Date().toISOString())
  }, 60000)
})
onBeforeUnmount(() => clearInterval(dayTimer))

/**
 * 最近 14 天（由舊到新，最後一天是今天）每天的入庫、出貨、損壞：件數與筆數。
 * 只算入庫、出貨、損壞三種紀錄；移動地點只是換位置，不算進出。
 */
const days = computed(() => {
  const [y, m, d] = today.value.split('-').map(Number)
  const list = []
  const byDate = new Map()
  for (let i = FLOW_DAYS - 1; i >= 0; i--) {
    const dt = new Date(y, m - 1, d - i)
    const day = {
      date: localDate(dt.toISOString()),
      label: `${dt.getMonth() + 1}/${dt.getDate()}`,
      inbound: 0,
      outbound: 0,
      damage: 0,
      inCount: 0,
      outCount: 0,
      damageCount: 0,
    }
    list.push(day)
    byDate.set(day.date, day)
  }
  // 紀錄的時間是 UTC：日期比這天還早的一定在 14 天以前，不用逐筆換算成本機日期（紀錄很多時省時間）
  const oldest = new Date(y, m - 1, d - FLOW_DAYS).toISOString().slice(0, 10)
  for (const l of stock.logs) {
    const ts = String(l.timestamp)
    if (ts.slice(0, 10) < oldest) continue
    const day = byDate.get(localDate(ts))
    if (!day) continue
    if (l.action_type === 'INBOUND') {
      day.inbound += l.delta
      day.inCount += 1
    } else if (l.action_type === 'OUTBOUND') {
      day.outbound -= l.delta
      day.outCount += 1
    } else if (l.action_type === 'DAMAGE') {
      day.damage -= l.delta
      day.damageCount += 1
    }
  }
  return list
})

/** 統計卡的數字與補充說明 */
const kpi = computed(() => {
  let units = 0
  for (const r of stock.inventory) units += r.quantity
  const inStock = products.items.filter((p) => stock.total(p.id) > 0).length
  const namedLocs = stock.locationStats.filter((s) => s.loc).length
  const unassigned = stock.locationStats.find((s) => !s.loc)?.qty ?? 0
  const now = days.value[days.value.length - 1]
  const yesterday = days.value[days.value.length - 2]
  return {
    units,
    inStock,
    items: products.items.length,
    namedLocs,
    locSub: !units ? '目前沒有庫存' : unassigned ? `未指定 ${fmtInt(unassigned)} 件` : '全部都有指定地點',
    today: now,
    inSub: [`${fmtInt(now.inCount)} 筆`, yesterday.inbound ? `昨天 ${fmtInt(yesterday.inbound)} 件` : ''],
    outSub: [`出貨 ${fmtInt(now.outCount)} 筆`, `損壞 ${fmtInt(now.damageCount)} 筆`],
  }
})

/** 各存放地點的庫存，件數多的排前面（件數相同時依名稱，「未指定」排最後） */
const locRows = computed(() =>
  [...stock.locationStats]
    .sort((a, b) => b.qty - a.qty || locCompare(a.loc, b.loc))
    .map((s) => ({ key: `loc:${s.loc}`, loc: s.loc, label: locLabel(s.loc), value: s.qty, sub: `${fmtInt(s.items)} 項`, italic: !s.loc })),
)
const locCountText = computed(() => {
  const named = locRows.value.filter((r) => r.loc).length
  return named === locRows.value.length ? `共 ${fmtInt(named)} 個地點` : `共 ${fmtInt(named)} 個地點＋未指定`
})
// 點某個地點：到庫存清單只看那個地點
function openLocation(row) {
  ui.invLoc = row.loc
  ui.go('inventory')
}

/** 庫存最多的品項（各地點加總），前 8 名；放在好幾個地點的品項註明有幾個地點 */
const topItems = computed(() =>
  products.items
    .map((p) => ({ p, total: stock.total(p.id) }))
    .filter((x) => x.total > 0)
    .sort((a, b) => b.total - a.total || a.p.name.localeCompare(b.p.name, 'zh-Hant'))
    .slice(0, TOP_ITEMS)
    .map(({ p, total }) => {
      const locs = stock.rowsOfProduct(p.id).length
      return { key: p.id, label: p.name, value: total, sub: locs > 1 ? `${locs} 個地點` : '' }
    }),
)

const recent = computed(() => stock.logs.slice(-RECENT).reverse())

/** 全新的專案（沒有紀錄也沒有庫存）：只顯示一張邀請入庫的卡片，不畫空的圖表 */
const isNew = computed(() => !stock.logs.length && !stock.inventory.length)
</script>

<template>
  <PageHeader title="主頁面" hint="目前專案的庫存概況">
    <button type="button" class="btn-go" @click="ui.go('inbound')">入庫</button>
    <button type="button" class="btn-primary" @click="ui.go('outbound')">出庫/損壞</button>
  </PageHeader>

  <!-- 統計卡：一眼看到現在有多少庫存、今天進出多少 -->
  <section class="grid grid-cols-4 gap-5">
    <StatTile
      label="在庫總件數"
      tone="slate"
      :icon="ICONS.layers"
      :value="fmtInt(kpi.units)"
      unit="件"
      :sub="[`${fmtInt(kpi.inStock)} 項有庫存`, `共 ${fmtInt(kpi.items)} 項`]"
    />
    <!-- 沒有用存放地點的專案不顯示地點，改成有庫存的品項數 -->
    <StatTile v-if="stock.usesLocations" label="存放地點" tone="violet" :icon="ICONS.pin" :value="fmtInt(kpi.namedLocs)" unit="個" :sub="kpi.locSub" />
    <StatTile v-else label="有庫存的品項" tone="violet" :icon="ICONS.box" :value="fmtInt(kpi.inStock)" unit="項" :sub="`共 ${fmtInt(kpi.items)} 項`" />
    <StatTile label="今日入庫" tone="go" :icon="ICONS.inbound" :value="fmtInt(kpi.today.inbound)" unit="件" :sub="kpi.inSub" />
    <StatTile label="今日出貨／損壞" tone="accent" :icon="ICONS.outbound" unit="件" :sub="kpi.outSub">
      <template #value>{{ fmtInt(kpi.today.outbound) }}<span class="mx-0.5 align-middle text-[0.6em] text-mute">／</span>{{ fmtInt(kpi.today.damage) }}</template>
    </StatTile>
  </section>

  <!-- 全新的專案：邀請從第一筆入庫開始 -->
  <section v-if="isNew" class="panel mt-5 flex flex-col items-center px-8 py-14 text-center">
    <span class="icon-badge h-14 w-14 text-go" style="--tone: var(--color-go)" aria-hidden="true">
      <svg viewBox="0 0 24 24" class="h-7 w-7" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round">
        <path :d="ICONS.inbound" />
      </svg>
    </span>
    <h2 class="mt-4 text-[17px] font-bold">還沒有任何異動紀錄</h2>
    <p class="mt-1.5 text-[13.5px] leading-relaxed text-mute">入庫第一批貨之後，這裡會顯示庫存概況、近 {{ FLOW_DAYS }} 天的進出和最近異動。</p>
    <button type="button" class="btn-go mt-5" @click="ui.go('inbound')">從第一筆入庫開始</button>
  </section>

  <!--
    圖表與清單：五欄的格線，卡片依序排下去——
    有用存放地點：第一列「近 14 天進出」（3 欄）＋「各存放地點的庫存」（2 欄），第二列「庫存最多的品項」（2 欄）＋「最近異動」（3 欄）；
    沒有用存放地點：第一列「近 14 天進出」＋「庫存最多的品項」，第二列整列是「最近異動」。
  -->
  <div v-else class="mt-5 grid grid-cols-5 gap-5">
    <section class="panel col-span-3 flex flex-col p-6">
      <FlowChart class="flex-1" :days="days" :note="stock.usesLocations ? '不含移動地點' : ''" />
    </section>

    <section v-if="stock.usesLocations" class="panel col-span-2 p-6">
      <div class="flex flex-wrap items-baseline justify-between gap-x-3">
        <h2 class="card-title">各存放地點的庫存</h2>
        <span v-if="locRows.length > SCROLL_ROWS" class="text-[12.5px] text-mute">{{ locCountText }}</span>
      </div>
      <template v-if="locRows.length">
        <p class="mt-0.5 text-[12.5px] text-mute">點一下地點，看那裡放了哪些品項</p>
        <RankBars class="mt-3" :rows="locRows" clickable @pick="openLocation" />
      </template>
      <p v-else class="py-10 text-center text-[13.5px] text-mute">目前沒有庫存</p>
    </section>

    <section class="panel col-span-2 p-6">
      <div class="flex flex-wrap items-baseline justify-between gap-x-3">
        <h2 class="card-title">庫存最多的品項</h2>
        <span v-if="kpi.inStock > topItems.length" class="text-[12.5px] text-mute">前 {{ topItems.length }} 名・共 {{ fmtInt(kpi.inStock) }} 項</span>
      </div>
      <template v-if="topItems.length">
        <p v-if="stock.usesLocations" class="mt-0.5 text-[12.5px] text-mute">不分存放地點的總數</p>
        <RankBars class="mt-3" :rows="topItems" />
      </template>
      <p v-else class="py-10 text-center text-[13.5px] text-mute">目前沒有庫存</p>
    </section>

    <section class="panel flex flex-col overflow-hidden" :class="stock.usesLocations ? 'col-span-3' : 'col-span-5'">
      <h2 class="card-title px-6 pt-5 pb-1">最近異動</h2>
      <div v-if="!recent.length" class="flex-1 px-5 py-10 text-center text-mute">還沒有任何異動紀錄</div>
      <template v-else>
        <ul class="flex-1 p-2">
          <li v-for="l in recent" :key="l.id" class="flex items-center gap-4 rounded-2xl px-4 py-3 odd:bg-ink/[0.035]">
            <ActionBadge :type="l.action_type" />
            <div class="min-w-0 flex-1">
              <div class="truncate text-[14px]">{{ l.product_name }}</div>
              <div class="text-[12px] text-mute">
                <span class="num">{{ fmtTime(l.timestamp) }}</span><template v-if="l.location_id">・{{ locLabel(l.location_id) }}</template>
                <span v-if="l.action_type === 'DAMAGE'">・{{ l.reason }}</span>
              </div>
            </div>
            <div class="num text-[16px]" :class="l.delta < 0 ? 'text-signal' : 'text-go'">{{ fmtDelta(l.delta) }}</div>
          </li>
        </ul>
        <button
          type="button"
          class="w-full py-3 text-[13.5px] font-semibold text-accent hover:bg-hover focus-visible:-outline-offset-2"
          @click="ui.go('logs')"
        >查看全部歷史紀錄</button>
      </template>
    </section>
  </div>
</template>
