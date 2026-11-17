<script setup>
// 排行長條：一列一個項目——上面是名稱與數量，下面是長條。
// 整組只有一種顏色：長條的長度就是數量，不另外用顏色分大小；標題已經說明這是什麼，所以沒有圖例。
import { computed } from 'vue'
import { fmtInt } from '../../utils/format'

const props = defineProps({
  /** [{ key, label, value, sub?, italic? }]，要顯示的順序（通常由大到小） */
  rows: { type: Array, required: true },
  unit: { type: String, default: '件' },
  /** 每一列都可以點（點了送出 pick，帶著那一列的資料） */
  clickable: { type: Boolean, default: false },
})
const emit = defineEmits(['pick'])

// 超過這個列數就在卡片裡上下捲動（高度寫在下面的 .is-scroll）
const MAX_ROWS = 8

const max = computed(() => props.rows.reduce((m, r) => Math.max(m, Number(r.value) || 0), 0))
/** 長條的寬度＝數量 ÷ 最大的那一列；有數量就至少畫 2px，才看得出來不是 0 */
const barStyle = (r) =>
  max.value > 0 && r.value > 0 ? { width: `${(r.value / max.value) * 100}%`, minWidth: '2px' } : { width: '0' }
</script>

<template>
  <ul class="rank-list -mx-2.5 space-y-0.5" :class="rows.length > MAX_ROWS ? 'is-scroll' : ''">
    <li v-for="r in rows" :key="r.key">
      <component
        :is="clickable ? 'button' : 'div'"
        :type="clickable ? 'button' : undefined"
        class="flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left"
        :class="clickable ? 'transition-colors hover:bg-hover focus-visible:-outline-offset-2' : ''"
        @click="clickable && emit('pick', r)"
      >
        <span class="block min-w-0 flex-1">
          <span class="flex items-baseline gap-2 leading-5">
            <span class="min-w-0 truncate text-[14px] text-ink" :class="r.italic ? 'italic' : ''" :title="r.label">{{ r.label }}</span>
            <span v-if="r.sub" class="shrink-0 text-[12.5px] whitespace-nowrap text-mute">{{ r.sub }}</span>
            <span class="ml-auto shrink-0 pl-1 whitespace-nowrap">
              <span class="num text-[15px] text-ink">{{ fmtInt(r.value) }}</span>
              <span v-if="unit" class="ml-1 text-[12px] text-mute">{{ unit }}</span>
            </span>
          </span>
          <span class="mt-[5px] block h-[10px]">
            <span class="rank-bar block h-full bg-chart-bar transition-[width] duration-300" :style="barStyle(r)" />
          </span>
        </span>
        <svg
          v-if="clickable"
          viewBox="0 0 24 24"
          class="h-4 w-4 shrink-0 text-mute"
          fill="none"
          stroke="currentColor"
          stroke-width="2.4"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path d="M9 6l6 6-6 6" />
        </svg>
      </component>
    </li>
  </ul>
</template>

<style scoped>
/* 超過 8 列：固定成 8 列的高度（每列 47px、列距 2px），其餘在卡片裡上下捲動 */
.rank-list.is-scroll {
  max-height: 390px;
  overflow-y: auto;
}
/* 長條：左邊貼齊起點（直角），右邊是資料的那一端（4px 圓角）；不畫底軌也不加框 */
.rank-bar {
  border-radius: 0 4px 4px 0;
  corner-shape: round;
}
</style>
