<script setup>
// 「現量 → 異動後」預覽，像貨架標籤一樣大字顯示
import { fmtInt } from '../utils/format'

defineProps({
  before: { type: Number, default: 0 },
  after: { type: Number, default: null },
  unit: { type: String, default: '' },
  caption: { type: String, default: '此庫位現量' },
  danger: { type: Boolean, default: false },
})
</script>

<template>
  <div class="flex items-end gap-3 rounded-md border border-line bg-surface/70 px-4 py-3">
    <div>
      <div class="text-[12px] text-mute">{{ caption }}</div>
      <div class="num text-3xl leading-none">{{ fmtInt(before) }}</div>
    </div>
    <template v-if="after !== null">
      <svg viewBox="0 0 24 24" class="mb-1 h-5 w-5 text-mute" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
      <div>
        <div class="text-[12px] text-mute">異動後</div>
        <div class="num text-3xl leading-none" :class="danger ? 'text-signal' : ''">{{ fmtInt(after) }}</div>
      </div>
    </template>
    <div class="ml-auto pb-0.5 text-[13px] text-mute">{{ unit }}</div>
  </div>
</template>
