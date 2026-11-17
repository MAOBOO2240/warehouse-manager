<script setup>
// 主頁面的統計卡：圖示圓章（顏色跟著 tone）＋標題，下面是大數字與單位，最後一行補充說明
import { computed } from 'vue'

const props = defineProps({
  label: { type: String, required: true },
  /** 大數字（已經格式化好的文字）；需要自己排版時改用 #value 插槽 */
  value: { type: [String, Number], default: '' },
  unit: { type: String, default: '' },
  /** 補充說明：一段文字，或好幾段（中間用「・」隔開；卡片太窄時一段一段換行，不會從字中間斷開） */
  sub: { type: [String, Array], default: '' },
  /** 主題色：slate / violet / go / accent / signal / amber / safety */
  tone: { type: String, default: 'slate' },
  /** 圖示：24×24 的 SVG 路徑 */
  icon: { type: String, default: '' },
})

const toneStyle = computed(() => ({ '--tone': `var(--color-${props.tone})`, '--tone-fg': `var(--color-on-${props.tone})` }))
const subParts = computed(() => (Array.isArray(props.sub) ? props.sub : [props.sub]).filter(Boolean))
</script>

<template>
  <div class="panel stat-tile px-5 py-[18px]" :style="toneStyle">
    <div class="flex items-center gap-3">
      <span v-if="icon" class="icon-badge" aria-hidden="true">
        <svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round">
          <path :d="icon" />
        </svg>
      </span>
      <div class="min-w-0 text-[13px] leading-snug text-mute">{{ label }}</div>
    </div>
    <div class="mt-3 flex flex-wrap items-baseline gap-x-1.5">
      <span class="num stat-value leading-none text-ink" style="font-variant-numeric: normal"><slot name="value">{{ value }}</slot></span>
      <span v-if="unit" class="text-[13px] text-mute">{{ unit }}</span>
    </div>
    <div v-if="subParts.length" class="mt-2 text-[12.5px] leading-snug text-mute">
      <span v-for="(s, i) in subParts" :key="i" class="inline-block">{{ s }}<template v-if="i < subParts.length - 1">・</template></span>
    </div>
  </div>
</template>

<style scoped>
.stat-tile {
  /* 讓大數字可以跟著卡片的寬度縮放（下面的 cqi）；圖示的顏色跟著這張卡的 --tone（見 style.css 的 .icon-badge） */
  container-type: inline-size;
}
/* 大數字：卡片夠寬時是 32px；視窗縮到最小或文字放大時跟著縮小，長數字才不會超出卡片 */
.stat-value {
  font-size: clamp(22px, 17cqi, 32px);
}
</style>
