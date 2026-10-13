<script setup>
import { computed } from 'vue'
import { ACTIONS, TONE_CLASS } from '../utils/format'

const props = defineProps({ type: { type: String, required: true } })
const info = computed(() => ACTIONS[props.type] ?? { label: props.type, tone: 'mute' })
</script>

<template>
  <span class="inline-block rounded-md px-2 py-0.5 text-[12px] font-semibold whitespace-nowrap" :class="TONE_CLASS[info.tone]">{{ info.label }}</span>
</template>
