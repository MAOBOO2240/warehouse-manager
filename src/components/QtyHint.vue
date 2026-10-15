<script setup>
// 數量欄位下方的提示：算式即時顯示結果，不合法時顯示原因
import { computed } from 'vue'
import { analyzeQty } from '../utils/calc'
import { fmtInt } from '../utils/format'

const props = defineProps({ text: { type: String, default: '' } })
const info = computed(() => analyzeQty(props.text))
</script>

<template>
  <div v-if="info.error || (info.isExpr && info.qty) || $slots.default">
    <p v-if="info.error" class="text-[12.5px] text-signal">{{ info.error }}</p>
    <p v-else-if="info.isExpr && info.qty" class="text-[13px] font-semibold" :style="{ color: 'var(--tone, var(--color-accent))' }">
      = <span class="num text-[15px]">{{ fmtInt(info.qty) }}</span> 件
    </p>
    <slot v-else />
  </div>
</template>
