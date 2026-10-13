<script setup>
// 列出某商品「有庫存」的庫位供點選（出庫、調撥來源、損耗用）
import { computed } from 'vue'
import { useStockStore } from '../stores/stock'
import { useLocationStore } from '../stores/locations'
import { fmtInt } from '../utils/format'

const props = defineProps({
  productId: { type: String, default: '' },
  modelValue: { type: String, default: '' },
  label: { type: String, default: '庫位' },
  unit: { type: String, default: '' },
})
const emit = defineEmits(['update:modelValue'])

const stock = useStockStore()
const locations = useLocationStore()
const rows = computed(() => (props.productId ? stock.rowsOfProduct(props.productId) : []))
</script>

<template>
  <div>
    <label class="field-label">{{ label }}</label>
    <div v-if="!productId" class="rounded-md border border-dashed border-line px-3 py-3 text-[13px] text-mute">
      先選擇商品，這裡會列出有庫存的庫位
    </div>
    <div v-else-if="!rows.length" class="rounded-md border border-signal/30 bg-signal-soft px-3 py-3 text-[13px] text-signal">
      此商品目前在所有庫位都沒有庫存
    </div>
    <div v-else class="grid max-h-56 grid-cols-2 gap-2 overflow-auto pr-1">
      <button
        v-for="r in rows"
        :key="r.location_id"
        type="button"
        class="flex items-center justify-between rounded-md border-2 px-3 py-2 text-left transition-colors"
        :class="modelValue === r.location_id ? 'border-ink bg-ink text-white' : 'border-line bg-white hover:border-ink-3'"
        @click="emit('update:modelValue', r.location_id)"
      >
        <span class="min-w-0">
          <span class="num block truncate text-[14px]">{{ r.location_id }}</span>
          <span class="block truncate text-[12px] opacity-70">{{ locations.get(r.location_id)?.description || locations.get(r.location_id)?.zone || '' }}</span>
        </span>
        <span class="num ml-2 text-lg">{{ fmtInt(r.quantity) }}<span class="ml-0.5 font-sans text-[11px] opacity-70">{{ unit }}</span></span>
      </button>
    </div>
  </div>
</template>
