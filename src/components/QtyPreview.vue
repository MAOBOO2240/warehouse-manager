<script setup>
// 「現量 → 異動後」預覽
import { fmtInt } from '../utils/format'

defineProps({
  before: { type: Number, default: 0 },
  after: { type: Number, default: null },
  unit: { type: String, default: '' },
  caption: { type: String, default: '現有' },
  danger: { type: Boolean, default: false },
})
</script>

<template>
  <div class="panel flex items-end gap-4 px-6 py-5">
    <div>
      <div class="text-[12.5px] text-mute">{{ caption }}</div>
      <div class="num mt-1 text-[34px] leading-none">{{ fmtInt(before) }}</div>
    </div>
    <template v-if="after !== null">
      <svg viewBox="0 0 24 24" class="mb-1.5 h-5 w-5 text-mute" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
      <div>
        <div class="text-[12.5px] text-mute">異動後</div>
        <div class="num mt-1 text-[34px] leading-none" :class="danger ? 'text-signal' : 'text-accent'">{{ fmtInt(after) }}</div>
      </div>
    </template>
    <div class="ml-auto pb-1 text-[13px] text-mute">{{ unit }}</div>
  </div>
</template>
