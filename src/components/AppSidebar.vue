<script setup>
import { useUiStore } from '../stores/ui'
import { useStockStore } from '../stores/stock'

const ui = useUiStore()
const stock = useStockStore()

const groups = [
  { title: '總覽', items: [{ id: 'dashboard', label: '儀表板' }] },
  {
    title: '日常作業',
    items: [
      { id: 'inbound', label: '入庫' },
      { id: 'outbound', label: '出庫' },
      { id: 'transfer', label: '庫位調撥' },
    ],
  },
  {
    title: '庫內管理',
    items: [
      { id: 'stocktake', label: '盤點平倉' },
      { id: 'loss', label: '損壞與報廢' },
    ],
  },
  {
    title: '主檔資料',
    items: [
      { id: 'products', label: '商品' },
      { id: 'locations', label: '庫位' },
    ],
  },
  {
    title: '查詢報表',
    items: [
      { id: 'inventory', label: '庫存清冊' },
      { id: 'logs', label: '異動流水帳' },
    ],
  },
]
</script>

<template>
  <nav class="flex w-52 shrink-0 flex-col bg-ink text-white/85">
    <div class="px-5 pt-5 pb-4">
      <div class="text-[15px] font-bold tracking-wider text-white">倉儲管理</div>
      <div class="mt-0.5 text-[11.5px] text-white/45">單機離線版</div>
    </div>
    <div class="flex-1 overflow-auto pb-4">
      <section v-for="g in groups" :key="g.title" class="mt-3">
        <h3 class="px-5 pb-1 text-[11.5px] text-white/40">{{ g.title }}</h3>
        <button
          v-for="item in g.items"
          :key="item.id"
          type="button"
          class="relative flex w-full items-center justify-between px-5 py-2 text-left text-[14px] transition-colors"
          :class="ui.page === item.id ? 'bg-ink-2 font-semibold text-white' : 'hover:bg-ink-2/60'"
          @click="ui.go(item.id)"
        >
          <span v-if="ui.page === item.id" class="absolute inset-y-1 left-0 w-1 rounded-r bg-safety" />
          {{ item.label }}
          <span
            v-if="item.id === 'dashboard' && stock.lowStock.length"
            class="num rounded bg-signal px-1.5 text-[12px] text-white"
            :title="`${stock.lowStock.length} 項商品低於安全庫存`"
          >{{ stock.lowStock.length }}</span>
        </button>
      </section>
    </div>
  </nav>
</template>
