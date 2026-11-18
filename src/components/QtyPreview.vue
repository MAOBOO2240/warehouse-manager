<script setup>
// 「現量 → 異動後」預覽
import { fmtDelta, fmtInt } from '../utils/format'

defineProps({
  before: { type: Number, default: 0 },
  after: { type: Number, default: null },
  unit: { type: String, default: '' },
  caption: { type: String, default: '現有' },
  danger: { type: Boolean, default: false },
  // 以下三個不給就是原本的樣子（出貨、損壞沒有用到）
  /** 右邊那個數字的標題：入庫後、移出後、移入後… */
  afterLabel: { type: String, default: '異動後' },
  /** 右邊那個數字的顏色：go（入庫）、violet（改變地點）…；不給是藍色 */
  tone: { type: String, default: '' },
  /** 在右邊的標題旁寫出增減多少（+40、-40） */
  delta: { type: Boolean, default: false },
})
</script>

<template>
  <div class="panel flex items-end gap-4 px-6 py-5">
    <div>
      <!-- 地點名稱很長時最多兩行，不會把右邊的數字擠出卡片 -->
      <div class="line-clamp-2 text-[12.5px] wrap-anywhere text-mute" :title="caption">{{ caption }}</div>
      <div class="num mt-1 text-[34px] leading-none">{{ fmtInt(before) }}</div>
    </div>
    <template v-if="after !== null">
      <svg viewBox="0 0 24 24" class="mb-1.5 h-5 w-5 shrink-0 text-mute" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
      <div>
        <div class="text-[12.5px] whitespace-nowrap text-mute">
          {{ afterLabel }}<span v-if="delta" class="num ml-1.5 text-[13.5px] font-semibold text-ink-2">{{ fmtDelta(after - before) }}</span>
        </div>
        <div
          class="num mt-1 text-[34px] leading-none"
          :class="danger ? 'text-signal' : tone ? '' : 'text-accent'"
          :style="!danger && tone ? { color: `var(--color-${tone})` } : null"
        >{{ fmtInt(after) }}</div>
      </div>
    </template>
    <div class="ml-auto pb-1 text-[13px] text-mute">{{ unit }}</div>
  </div>
</template>
