<script setup>
// 日期欄位：年 / 月 / 日三格，輸入 4 位年份自動跳月份、2 位月份自動跳日期；
// 右側按鈕開啟彈窗日曆挑選。v-model 為 'YYYY-MM-DD'，未填完整時為 ''。
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

const props = defineProps({
  modelValue: { type: String, default: '' },
  label: { type: String, default: '' },
  id: { type: String, default: '' },
})
const emit = defineEmits(['update:modelValue'])

const y = ref('')
const m = ref('')
const d = ref('')
const yEl = ref(null)
const mEl = ref(null)
const dEl = ref(null)
const root = ref(null)
const open = ref(false)

const pad = (n) => String(n).padStart(2, '0')
const daysIn = (yy, mm) => new Date(yy, mm, 0).getDate()

function fromModel(v) {
  const hit = /^(\d{4})-(\d{2})-(\d{2})$/.exec(v || '')
  if (hit) [y.value, m.value, d.value] = [hit[1], hit[2], hit[3]]
  else if (!v) [y.value, m.value, d.value] = ['', '', '']
}
fromModel(props.modelValue)
watch(() => props.modelValue, (v) => {
  if (v !== current.value) fromModel(v)
})

/** 三格都合法時的完整日期，否則 '' */
const current = computed(() => {
  if (!/^\d{4}$/.test(y.value) || !/^\d{1,2}$/.test(m.value) || !/^\d{1,2}$/.test(d.value)) return ''
  const yy = Number(y.value)
  const mm = Number(m.value)
  const dd = Number(d.value)
  if (mm < 1 || mm > 12 || dd < 1 || dd > daysIn(yy, mm)) return ''
  return `${yy}-${pad(mm)}-${pad(dd)}`
})
const invalid = computed(() => (y.value || m.value || d.value) && !current.value && d.value.length >= 1 && m.value.length >= 1 && y.value.length === 4)

watch(current, (v) => emit('update:modelValue', v))

function digits(e, max) {
  return e.target.value.replace(/\D/g, '').slice(0, max)
}

function onY(e) {
  y.value = digits(e, 4)
  e.target.value = y.value
  if (y.value.length === 4) mEl.value?.focus()
}
function onM(e) {
  m.value = digits(e, 2)
  e.target.value = m.value
  // 輸入 2 位，或第一位就大於 1（不可能是兩位數月份）時跳到日期
  if (m.value.length === 2 || Number(m.value) > 1) dEl.value?.focus()
}
function onD(e) {
  d.value = digits(e, 2)
  e.target.value = d.value
}
function padOnBlur(which) {
  const field = which === 'm' ? m : d
  if (field.value.length === 1 && field.value !== '0') field.value = pad(field.value)
}
// 空白欄位按 Backspace 回到前一格
function back(e, prevEl) {
  if (e.key === 'Backspace' && !e.target.value) {
    e.preventDefault()
    prevEl?.focus()
  }
}
function clear() {
  y.value = m.value = d.value = ''
  open.value = false
}

/* ---- 彈窗日曆 ---- */
const today = new Date()
const viewY = ref(today.getFullYear())
const viewM = ref(today.getMonth() + 1)
const weekdays = ['日', '一', '二', '三', '四', '五', '六']
const todayStr = `${today.getFullYear()}-${pad(today.getMonth() + 1)}-${pad(today.getDate())}`

const cells = computed(() => {
  const first = new Date(viewY.value, viewM.value - 1, 1).getDay()
  const total = daysIn(viewY.value, viewM.value)
  const list = Array.from({ length: first }, () => null)
  for (let i = 1; i <= total; i++) list.push(`${viewY.value}-${pad(viewM.value)}-${pad(i)}`)
  return list
})

function toggle() {
  if (!open.value) {
    const base = current.value ? new Date(current.value + 'T00:00:00') : today
    viewY.value = base.getFullYear()
    viewM.value = base.getMonth() + 1
  }
  open.value = !open.value
}
function shift(step) {
  let mm = viewM.value + step
  let yy = viewY.value
  if (mm < 1) { mm = 12; yy-- }
  if (mm > 12) { mm = 1; yy++ }
  viewM.value = mm
  viewY.value = yy
}
async function pick(v) {
  ;[y.value, m.value, d.value] = v.split('-')
  open.value = false
  await nextTick()
}

function onDocDown(e) {
  if (open.value && root.value && !root.value.contains(e.target)) open.value = false
}
function onKey(e) {
  if (e.key === 'Escape') open.value = false
}
onMounted(() => {
  document.addEventListener('mousedown', onDocDown)
  window.addEventListener('keydown', onKey)
})
onBeforeUnmount(() => {
  document.removeEventListener('mousedown', onDocDown)
  window.removeEventListener('keydown', onKey)
})
</script>

<template>
  <div ref="root" class="relative">
    <label v-if="label" class="field-label" :for="id || undefined">{{ label }}</label>
    <div
      class="date-box flex items-center gap-0.5 rounded-full border-[1.5px] border-field-line bg-field py-1 pr-1 pl-4 transition-[border-color,box-shadow] hover:border-field-line-hover"
      :class="invalid ? '!border-signal' : ''"
    >
      <input
        :id="id || undefined"
        ref="yEl"
        :value="y"
        class="num w-[3.2em] bg-transparent py-1.5 text-center text-[15px] outline-none placeholder:text-mute/75"
        inputmode="numeric"
        placeholder="年"
        aria-label="年"
        @input="onY"
      />
      <span class="text-mute">/</span>
      <input
        ref="mEl"
        :value="m"
        class="num w-[2.2em] bg-transparent py-1.5 text-center text-[15px] outline-none placeholder:text-mute/75"
        inputmode="numeric"
        placeholder="月"
        aria-label="月"
        @input="onM"
        @blur="padOnBlur('m')"
        @keydown="back($event, yEl)"
      />
      <span class="text-mute">/</span>
      <input
        ref="dEl"
        :value="d"
        class="num w-[2.2em] bg-transparent py-1.5 text-center text-[15px] outline-none placeholder:text-mute/75"
        inputmode="numeric"
        placeholder="日"
        aria-label="日"
        @input="onD"
        @blur="padOnBlur('d')"
        @keydown="back($event, mEl)"
      />
      <button
        v-if="y || m || d"
        type="button"
        class="ml-auto flex h-7 w-7 items-center justify-center rounded-full text-mute hover:bg-hover hover:text-ink"
        aria-label="清除日期"
        @click="clear"
      >
        <svg viewBox="0 0 24 24" class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
      </button>
      <button
        type="button"
        class="flex h-8 w-8 items-center justify-center rounded-full text-accent hover:bg-accent-soft"
        :class="y || m || d ? '' : 'ml-auto'"
        aria-label="開啟日曆"
        :aria-expanded="open"
        @click="toggle"
      >
        <svg viewBox="0 0 24 24" class="h-[18px] w-[18px]" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><rect x="3.5" y="5" width="17" height="15.5" rx="4" /><path d="M8 3v4M16 3v4M3.5 10h17" /></svg>
      </button>
    </div>

    <div v-if="open" class="menu-panel absolute top-full left-0 z-40 mt-2 w-[296px] rounded-[28px] p-4" role="dialog" aria-label="選擇日期">
      <div class="mb-3 flex items-center justify-between">
        <button type="button" class="flex h-9 w-9 items-center justify-center rounded-full bg-raised hover:bg-hover" aria-label="上個月" @click="shift(-1)">
          <svg viewBox="0 0 24 24" class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M15 6l-6 6 6 6" /></svg>
        </button>
        <div class="num text-[16px] font-semibold">{{ viewY }} 年 {{ viewM }} 月</div>
        <button type="button" class="flex h-9 w-9 items-center justify-center rounded-full bg-raised hover:bg-hover" aria-label="下個月" @click="shift(1)">
          <svg viewBox="0 0 24 24" class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M9 6l6 6-6 6" /></svg>
        </button>
      </div>
      <div class="grid grid-cols-7 gap-1 text-center">
        <div v-for="w in weekdays" :key="w" class="pb-1 text-[12px] text-mute">{{ w }}</div>
        <template v-for="(c, i) in cells" :key="i">
          <div v-if="!c" />
          <button
            v-else
            type="button"
            class="num flex h-9 items-center justify-center rounded-full text-[14px] transition-colors"
            :class="
              c === current
                ? 'bg-accent font-semibold text-on-accent'
                : c === todayStr
                  ? 'font-semibold text-accent ring-1 ring-accent/50 hover:bg-hover'
                  : 'hover:bg-hover'
            "
            @click="pick(c)"
          >{{ Number(c.slice(8)) }}</button>
        </template>
      </div>
      <div class="mt-3 flex justify-between">
        <button type="button" class="btn-ghost btn-sm" @click="clear">清除</button>
        <button type="button" class="btn-primary btn-sm" @click="pick(todayStr)">今天</button>
      </div>
    </div>
  </div>
</template>
