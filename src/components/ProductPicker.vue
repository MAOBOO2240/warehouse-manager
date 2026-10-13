<script setup>
// 商品選取器：掃描槍掃入條碼後按 Enter 即精準比對；也可輸入名稱關鍵字後點選。
import { computed, nextTick, ref, watch } from 'vue'
import { useProductStore } from '../stores/products'
import { useStockStore } from '../stores/stock'
import { fmtInt } from '../utils/format'

const props = defineProps({
  modelValue: { type: String, default: '' },
  placeholder: { type: String, default: '掃描條碼，或輸入商品名稱 / SKU 後按 Enter' },
  label: { type: String, default: '商品' },
  showStock: { type: Boolean, default: true },
})
const emit = defineEmits(['update:modelValue', 'picked'])

const products = useProductStore()
const stock = useStockStore()

const input = ref(null)
const text = ref('')
const active = ref(-1)
const message = ref('')

const selected = computed(() => (props.modelValue ? products.get(props.modelValue) : null))
const suggestions = computed(() => (text.value.trim() ? products.search(text.value).slice(0, 8) : []))

watch(text, () => {
  active.value = -1
  message.value = ''
})

function pick(p) {
  text.value = ''
  message.value = ''
  emit('update:modelValue', p.id)
  emit('picked', p)
}

function onEnter() {
  const code = text.value.trim()
  if (!code) return
  const exact = products.findExact(code)
  if (exact) return pick(exact)
  if (active.value >= 0 && suggestions.value[active.value]) return pick(suggestions.value[active.value])
  if (suggestions.value.length === 1) return pick(suggestions.value[0])
  message.value = suggestions.value.length
    ? `找到 ${suggestions.value.length} 筆相符商品，請從下方點選`
    : `找不到「${code}」對應的商品，請確認條碼或先到商品主檔建立`
  input.value?.select()
}

function move(step) {
  const n = suggestions.value.length
  if (!n) return
  active.value = (active.value + step + n) % n
}

async function clear() {
  emit('update:modelValue', '')
  await nextTick()
  input.value?.focus()
}

function focus() {
  input.value?.focus()
}

defineExpose({ focus, clear })
</script>

<template>
  <div>
    <label class="field-label">{{ label }}</label>

    <div v-if="selected" class="flex items-center gap-3 rounded-md border-2 border-ink/80 bg-white px-3 py-2.5">
      <div class="min-w-0 flex-1">
        <div class="truncate text-[15px] font-semibold">{{ selected.name }}</div>
        <div class="num mt-0.5 flex flex-wrap gap-x-3 text-[12.5px] text-mute">
          <span>{{ selected.id }}</span>
          <span v-if="selected.sku">SKU {{ selected.sku }}</span>
          <span v-if="selected.barcode">條碼 {{ selected.barcode }}</span>
          <span v-if="selected.spec" class="font-sans">{{ selected.spec }}</span>
        </div>
      </div>
      <div v-if="showStock" class="text-right">
        <div class="text-[11.5px] text-mute">全庫存量</div>
        <div class="num text-lg leading-tight">{{ fmtInt(stock.total(selected.id)) }} <span class="font-sans text-xs text-mute">{{ selected.unit }}</span></div>
      </div>
      <button type="button" class="btn-ghost btn-sm" @click="clear">更換</button>
    </div>

    <div v-else class="relative">
      <input
        ref="input"
        v-model="text"
        class="scan-field"
        :placeholder="placeholder"
        autocomplete="off"
        spellcheck="false"
        @keydown.enter.prevent="onEnter"
        @keydown.down.prevent="move(1)"
        @keydown.up.prevent="move(-1)"
        @keydown.esc="text = ''"
      />
      <ul
        v-if="suggestions.length && !products.findExact(text)"
        class="absolute inset-x-0 top-full z-30 mt-1 max-h-72 overflow-auto rounded-md border border-line bg-white py-1 shadow-lg"
      >
        <li v-for="(p, i) in suggestions" :key="p.id">
          <button
            type="button"
            class="flex w-full items-center gap-3 px-3 py-2 text-left"
            :class="i === active ? 'bg-steel-soft' : 'hover:bg-surface'"
            @mousedown.prevent="pick(p)"
          >
            <span class="min-w-0 flex-1 truncate">{{ p.name }}</span>
            <span class="num text-xs text-mute">{{ p.barcode || p.sku || p.id }}</span>
            <span class="num w-14 text-right text-xs">{{ fmtInt(stock.total(p.id)) }}</span>
          </button>
        </li>
      </ul>
      <p v-if="message" class="mt-1.5 text-[13px] text-signal">{{ message }}</p>
    </div>
  </div>
</template>
