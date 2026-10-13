<script setup>
import { computed } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import ActionBadge from '../components/ActionBadge.vue'
import { useProductStore } from '../stores/products'
import { useStockStore } from '../stores/stock'
import { useUiStore } from '../stores/ui'
import { fmtDelta, fmtInt, fmtMoney, fmtTime } from '../utils/format'

const products = useProductStore()
const stock = useStockStore()
const ui = useUiStore()

const stats = computed(() => {
  let units = 0
  let cost = 0
  let retail = 0
  for (const r of stock.inventory) {
    const p = products.get(r.product_id)
    units += r.quantity
    if (p) {
      cost += r.quantity * (Number(p.cost_price) || 0)
      retail += r.quantity * (Number(p.sale_price) || 0)
    }
  }
  return [
    { label: '商品品項數 (SKU)', value: fmtInt(products.items.length), unit: '項' },
    { label: '在庫總件數', value: fmtInt(units), unit: '件' },
    { label: '庫存總成本', value: fmtMoney(cost), unit: '元', note: 'Σ 數量 × 成本價' },
    { label: '預期零售總值', value: fmtMoney(retail), unit: '元', note: 'Σ 數量 × 售價' },
  ]
})

const recent = computed(() => stock.logs.slice(-8).reverse())
</script>

<template>
  <PageHeader title="儀表板" hint="即時庫存概況與低庫存預警" />

  <section class="grid grid-cols-4 gap-4">
    <div v-for="s in stats" :key="s.label" class="panel px-5 py-4">
      <div class="text-[13px] text-mute">{{ s.label }}</div>
      <div class="mt-2 flex items-baseline gap-1.5">
        <span class="num text-[30px] leading-none">{{ s.value }}</span>
        <span class="text-[13px] text-mute">{{ s.unit }}</span>
      </div>
      <div v-if="s.note" class="mt-1.5 text-[12px] text-mute/80">{{ s.note }}</div>
    </div>
  </section>

  <section class="mt-6 grid grid-cols-5 gap-5">
    <div class="col-span-3">
      <h2 class="mb-3 flex items-center gap-2 text-[16px] font-bold">
        低庫存預警
        <span v-if="stock.lowStock.length" class="num rounded bg-signal px-2 text-[13px] text-white">{{ stock.lowStock.length }}</span>
      </h2>
      <div v-if="!products.items.length" class="panel px-5 py-8 text-center text-mute">
        還沒有任何商品。
        <button type="button" class="ml-1 font-semibold text-steel underline" @click="ui.go('products')">前往建立商品</button>
      </div>
      <div v-else-if="!stock.lowStock.length" class="panel px-5 py-8 text-center text-mute">
        所有設定了最低安全庫存的商品都高於安全水位。
      </div>
      <div v-else class="grid grid-cols-2 gap-3">
        <article
          v-for="{ product: p, total } in stock.lowStock"
          :key="p.id"
          class="rounded-lg border border-signal/35 border-l-4 border-l-signal bg-signal-soft/50 px-4 py-3"
        >
          <div class="flex items-start justify-between gap-2">
            <div class="min-w-0">
              <div class="truncate font-semibold">{{ p.name }}</div>
              <div class="num mt-0.5 truncate text-[12px] text-mute">{{ p.barcode || p.sku || p.id }}</div>
            </div>
            <div class="text-right">
              <div class="num text-2xl leading-none text-signal">{{ fmtInt(total) }}</div>
              <div class="mt-1 text-[11.5px] text-mute">安全量 {{ fmtInt(p.min_stock) }} {{ p.unit }}</div>
            </div>
          </div>
          <button type="button" class="btn-primary btn-sm mt-3 w-full" @click="ui.go('inbound', { productId: p.id })">
            直接入庫
          </button>
        </article>
      </div>
    </div>

    <div class="col-span-2">
      <h2 class="mb-3 text-[16px] font-bold">最近異動</h2>
      <div class="panel overflow-hidden">
        <div v-if="!recent.length" class="px-5 py-8 text-center text-mute">尚無任何異動紀錄</div>
        <ul v-else class="divide-y divide-line">
          <li v-for="l in recent" :key="l.id" class="flex items-center gap-3 px-4 py-2.5">
            <ActionBadge :type="l.action_type" />
            <div class="min-w-0 flex-1">
              <div class="truncate text-[13.5px]">{{ l.product_name }}</div>
              <div class="num text-[12px] text-mute">{{ fmtTime(l.timestamp) }}・{{ l.location_id }}</div>
            </div>
            <div class="num text-[15px]" :class="l.delta < 0 ? 'text-signal' : 'text-go'">{{ fmtDelta(l.delta) }}</div>
          </li>
        </ul>
        <button type="button" class="w-full border-t border-line py-2 text-[13px] text-steel hover:bg-surface" @click="ui.go('logs')">
          查看全部流水帳
        </button>
      </div>
    </div>
  </section>
</template>
