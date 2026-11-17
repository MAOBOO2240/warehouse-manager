<script setup>
// 每一頁最上面的標題列：左邊是該頁的圖示（顏色與左側選單一致）、標題與說明，右邊是該頁的按鈕
import { computed } from 'vue'
import { useUiStore } from '../stores/ui'
import { NAV_ICONS, NAV_TONES } from '../utils/nav'

defineProps({
  title: { type: String, required: true },
  hint: { type: String, default: '' },
})

const ui = useUiStore()
const icon = computed(() => NAV_ICONS[ui.page] ?? '')
const tone = computed(() => NAV_TONES[ui.page] ?? 'accent')
</script>

<template>
  <header class="mb-6 flex items-end justify-between gap-4">
    <div class="flex min-w-0 items-center gap-3.5">
      <span v-if="icon" class="page-icon" :style="{ '--tone': `var(--color-${tone})` }" aria-hidden="true">
        <svg viewBox="0 0 24 24" class="h-[22px] w-[22px]" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round">
          <path :d="icon" />
        </svg>
      </span>
      <div class="min-w-0">
        <h1 class="text-[26px] leading-tight font-bold tracking-wide">{{ title }}</h1>
        <p v-if="hint" class="mt-1 text-[13.5px] text-mute">{{ hint }}</p>
      </div>
    </div>
    <div class="flex shrink-0 gap-2.5">
      <slot />
    </div>
  </header>
</template>
