<script setup>
// 品項選取器：點一下列出目前專案所有品項（可用滾輪捲動），輸入文字即時篩選；
// allowCreate 時，輸入清單中沒有的名稱可直接新增為新品項。
import { computed, nextTick, ref, watch } from 'vue'
import { useProductStore } from '../stores/products'
import { useStockStore } from '../stores/stock'
import { fmtInt } from '../utils/format'

const props = defineProps({
  /** 目前選擇：{ id, name } 既有品項，或 { id: '', name, isNew: true } 新品項；null 為未選 */
  selected: { type: Object, default: null },
  allowCreate: { type: Boolean, default: false },
  label: { type: String, default: '品項' },
})
const emit = defineEmits(['select', 'clear'])

const products = useProductStore()
const stock = useStockStore()

const input = ref(null)
const text = ref('')
const open = ref(false)
const active = ref(-1)
const listEl = ref(null)

const filtered = computed(() => products.search(text.value))
const exact = computed(() => products.findByName(text.value))
const canCreate = computed(() => props.allowCreate && text.value.trim() !== '' && !exact.value)
// 選單項目：新增列（若有）＋符合的品項
const options = computed(() => [
  ...(canCreate.value ? [{ create: true, name: text.value.trim() }] : []),
  ...filtered.value.map((p) => ({ product: p })),
])

watch(text, () => {
  active.value = options.value.length ? 0 : -1
  open.value = true
})

function choose(opt) {
  if (!opt) return
  if (opt.create) emit('select', { id: '', name: opt.name, isNew: true })
  else emit('select', { id: opt.product.id, name: opt.product.name, isNew: false })
  text.value = ''
  open.value = false
}

function onEnter() {
  if (exact.value) return choose({ product: exact.value })
  if (active.value >= 0) return choose(options.value[active.value])
}

async function move(step) {
  const n = options.value.length
  if (!n) return
  open.value = true
  active.value = (active.value + step + n) % n
  await nextTick()
  listEl.value?.querySelector('[data-active="true"]')?.scrollIntoView({ block: 'nearest' })
}

async function clear() {
  emit('clear')
  await nextTick()
  focus()
}

function focus() {
  input.value?.focus()
  open.value = true
}

defineExpose({ focus })
</script>

<template>
  <div>
    <label class="field-label">{{ label }}</label>

    <button
      v-if="selected"
      type="button"
      class="group flex w-full items-center gap-3 rounded-2xl bg-accent-soft px-4 py-3 text-left ring-2 ring-accent/50 transition-shadow hover:ring-accent"
      title="點一下更換品項"
      @click="clear"
    >
      <div class="min-w-0 flex-1">
        <div class="truncate text-[15px] font-semibold">{{ selected.name }}</div>
        <div class="mt-0.5 text-[12.5px] text-mute">
          <span v-if="selected.isNew" class="font-semibold text-go">新品項，確認後自動建立</span>
          <span v-else>全部庫存 <span class="num">{{ fmtInt(stock.total(selected.id)) }}</span> 件</span>
        </div>
      </div>
      <span class="flex shrink-0 items-center gap-1 text-[12.5px] font-semibold text-accent opacity-70 transition-opacity group-hover:opacity-100">
        <svg viewBox="0 0 24 24" class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 8h13M13.5 4.5 17 8l-3.5 3.5M20 16H7M10.5 12.5 7 16l3.5 3.5" /></svg>
        點一下更換
      </span>
    </button>

    <div v-else class="relative">
      <input
        ref="input"
        v-model="text"
        class="field text-[15px]"
        :placeholder="allowCreate ? '點此選擇品項，或輸入名稱新增品項' : '點此選擇品項，或輸入名稱搜尋'"
        autocomplete="off"
        spellcheck="false"
        @focus="open = true"
        @click="open = true"
        @blur="open = false"
        @keydown.enter.prevent="onEnter"
        @keydown.down.prevent="move(1)"
        @keydown.up.prevent="move(-1)"
        @keydown.esc="open = false"
      />
      <ul
        v-if="open"
        ref="listEl"
        class="menu-panel absolute inset-x-0 top-full z-30 mt-2 max-h-80 overflow-y-auto overscroll-contain rounded-2xl p-1.5"
      >
        <li v-for="(opt, i) in options" :key="opt.create ? '__create' : opt.product.id">
          <button
            type="button"
            :data-active="i === active"
            class="flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left"
            :class="i === active ? 'bg-accent-soft' : 'hover:bg-hover'"
            @mousedown.prevent="choose(opt)"
            @mousemove="active = i"
          >
            <template v-if="opt.create">
              <span class="flex h-5 w-5 items-center justify-center rounded-full bg-go text-white" aria-hidden="true">
                <svg viewBox="0 0 24 24" class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="3"><path d="M12 5v14M5 12h14" /></svg>
              </span>
              <span class="flex-1 font-semibold text-go">新增品項「{{ opt.name }}」</span>
            </template>
            <template v-else>
              <span class="min-w-0 flex-1 truncate">{{ opt.product.name }}</span>
              <span class="num text-[13px]" :class="stock.total(opt.product.id) ? 'text-ink' : 'text-mute'">
                {{ stock.total(opt.product.id) ? `${fmtInt(stock.total(opt.product.id))} 件` : '無庫存' }}
              </span>
            </template>
          </button>
        </li>
        <li v-if="!options.length" class="px-3 py-3 text-[13px] text-mute">
          {{ products.items.length ? '沒有符合的品項' : allowCreate ? '還沒有任何品項，輸入名稱即可新增' : '還沒有任何品項，請先入庫' }}
        </li>
      </ul>
    </div>
  </div>
</template>
