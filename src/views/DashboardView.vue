<script setup>
import { computed } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import ActionBadge from '../components/ActionBadge.vue'
import { useProductStore } from '../stores/products'
import { useStockStore } from '../stores/stock'
import { useUiStore } from '../stores/ui'
import { fmtDelta, fmtInt, fmtTime, localDate, locLabel } from '../utils/format'

const products = useProductStore()
const stock = useStockStore()
const ui = useUiStore()

const stats = computed(() => {
  const today = localDate(new Date().toISOString())
  let units = 0
  for (const r of stock.inventory) units += r.quantity
  let inToday = 0
  let shipToday = 0
  let damageToday = 0
  for (const l of stock.logs) {
    if (localDate(l.timestamp) !== today) continue
    if (l.action_type === 'INBOUND') inToday += l.delta
    else if (l.action_type === 'OUTBOUND') shipToday -= l.delta
    else if (l.action_type === 'DAMAGE') damageToday -= l.delta
  }
  const inStock = products.items.filter((p) => stock.total(p.id) > 0).length
  return [
    { label: '有庫存的品項', value: fmtInt(inStock), unit: `／ 共 ${fmtInt(products.items.length)} 項` },
    { label: '在庫總件數', value: fmtInt(units), unit: '件' },
    { label: '今日入庫', value: fmtInt(inToday), unit: '件' },
    { label: '今日出貨／損壞', value: `${fmtInt(shipToday)}／${fmtInt(damageToday)}`, unit: '件' },
  ]
})

const recent = computed(() => stock.logs.slice(-10).reverse())
</script>

<template>
  <PageHeader title="儀表板" hint="目前專案的庫存概況">
    <button type="button" class="btn-go" @click="ui.go('inbound')">入庫</button>
    <button type="button" class="btn-primary" @click="ui.go('outbound')">出庫/損壞</button>
  </PageHeader>

  <section class="grid grid-cols-4 gap-5">
    <div v-for="s in stats" :key="s.label" class="panel px-6 py-5">
      <div class="text-[13px] text-mute">{{ s.label }}</div>
      <div class="mt-2 flex items-baseline gap-1.5">
        <span class="num text-[34px] leading-none">{{ s.value }}</span>
        <span class="text-[13px] text-mute">{{ s.unit }}</span>
      </div>
    </div>
  </section>

  <section class="mt-8">
    <h2 class="mb-3 pl-1 text-[17px] font-bold">最近異動</h2>
    <div class="panel overflow-hidden">
      <div v-if="!recent.length" class="px-5 py-10 text-center text-mute">
        還沒有任何異動紀錄。
        <button type="button" class="ml-1 font-semibold text-accent underline" @click="ui.go('inbound')">從第一筆入庫開始</button>
      </div>
      <ul v-else class="p-2">
        <li v-for="l in recent" :key="l.id" class="flex items-center gap-4 rounded-2xl px-4 py-3.5 odd:bg-ink/[0.035]">
          <ActionBadge :type="l.action_type" />
          <div class="min-w-0 flex-1">
            <div class="truncate text-[14px]">{{ l.product_name }}</div>
            <div class="text-[12px] text-mute">
              <span class="num">{{ fmtTime(l.timestamp) }}</span>・{{ locLabel(l.location_id) }}
              <span v-if="l.action_type === 'DAMAGE'">・{{ l.reason }}</span>
            </div>
          </div>
          <div class="num text-[16px]" :class="l.delta < 0 ? 'text-signal' : 'text-go'">{{ fmtDelta(l.delta) }}</div>
        </li>
      </ul>
      <button
        v-if="recent.length"
        type="button"
        class="w-full py-3 text-[13.5px] font-semibold text-accent hover:bg-hover"
        @click="ui.go('logs')"
      >查看全部異動流水帳</button>
    </div>
  </section>
</template>
