<script setup>
// 移除存放地點：這個地點的庫存一次改放到別處（或設為未指定），之後這個地點不再出現在選擇清單裡
import { computed, nextTick, ref } from 'vue'
import ModalDialog from './ModalDialog.vue'
import ComboField from './ComboField.vue'
import { useProductStore } from '../stores/products'
import { useStockStore } from '../stores/stock'
import { useRowOrderStore } from '../stores/rowOrder'
import { useUiStore } from '../stores/ui'
import { useProjectStore } from '../stores/project'
import { fmtInt, locLabel } from '../utils/format'

const props = defineProps({
  location: { type: String, required: true },
})
const emit = defineEmits(['close'])

const products = useProductStore()
const stock = useStockStore()
const rowOrder = useRowOrderStore()
const ui = useUiStore()
const project = useProjectStore()

/** 這個地點目前放的品項（順序和庫存清單相同） */
const items = computed(() =>
  rowOrder.sort(
    stock.inventory
      .filter((r) => r.location_id === props.location && r.quantity > 0)
      .map((r) => ({ ...r, name: products.get(r.product_id)?.name ?? r.product_id })),
    (a, b) => a.name.localeCompare(b.name, 'zh-Hant'),
  ),
)
const totalQty = computed(() => items.value.reduce((t, r) => t + r.quantity, 0))
const hasStock = computed(() => items.value.length > 0)

// 庫存改放到哪裡：none＝未指定（不設定地點）；other＝自己輸入或選一個其他地點
const mode = ref('none')
const other = ref('')
const otherEl = ref(null)
const busy = ref(false)

const dest = computed(() => (hasStock.value && mode.value === 'other' ? other.value.trim() : ''))
const otherLocations = computed(() => stock.knownLocations.filter((l) => l !== props.location))
const problem = computed(() => {
  if (!hasStock.value || mode.value !== 'other') return ''
  if (!dest.value) return '請輸入或選擇要改放的地點'
  if (dest.value.length > 50) return '存放地點請在 50 個字以內'
  if (dest.value === props.location) return '不可改放到正要移除的地點'
  return ''
})
const canSubmit = computed(() => !busy.value && !project.readOnly && !problem.value)

async function pick(m) {
  mode.value = m
  if (m === 'other') {
    await nextTick()
    otherEl.value?.focus()
  }
}

async function submit() {
  if (!canSubmit.value) return
  busy.value = true
  // 移除之後這個地點就沒有庫存了，提示要用的數字先記下來
  const name = props.location
  const count = items.value.length
  const qty = totalQty.value
  const to = dest.value
  try {
    await stock.removeLocation(name, to)
    ui.ok(
      count
        ? `已移除存放地點「${name}」：${fmtInt(count)} 項共 ${fmtInt(qty)} 件改放到「${locLabel(to)}」`
        : `已移除存放地點「${name}」`,
    )
    emit('close')
  } catch (err) {
    ui.error(err)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <ModalDialog title="移除存放地點" :subtitle="location" tone="signal" :closable="!busy" @close="emit('close')">
    <template #icon>
      <svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M4 7h16M10 11v6M14 11v6M5.5 7l1 12a2 2 0 0 0 2 1.8h7a2 2 0 0 0 2-1.8l1-12M9 7V4.5h6V7" />
      </svg>
    </template>

    <div class="space-y-5">
      <div class="tone-card">
        <template v-if="hasStock">
          <p class="text-[14.5px] font-semibold">
            這個地點目前有 <span class="num text-[17px]">{{ fmtInt(items.length) }}</span> 項、共
            <span class="num text-[17px]">{{ fmtInt(totalQty) }}</span> 件
          </p>
          <ul class="mt-2.5 max-h-44 space-y-0.5 overflow-y-auto overscroll-contain pr-1">
            <li
              v-for="r in items"
              :key="r.product_id"
              class="flex items-baseline justify-between gap-4 rounded-xl px-2.5 py-1 text-[14px] odd:bg-ink/[0.045]"
            >
              <span class="min-w-0 break-words">{{ r.name }}</span>
              <span class="num shrink-0 text-[15.5px]">{{ fmtInt(r.quantity) }}</span>
            </li>
          </ul>
        </template>
        <p v-else class="text-[14.5px] leading-relaxed">這個地點目前沒有庫存，移除後就不會再出現在選擇清單裡。</p>
      </div>

      <div v-if="hasStock">
        <label class="field-label" for="rl-dest">這些庫存要改放到哪裡？</label>
        <div class="flex flex-wrap gap-2">
          <button
            type="button"
            class="pill"
            :class="mode === 'none' ? 'pill-tone-on' : ''"
            :aria-pressed="mode === 'none'"
            :disabled="busy"
            @click="pick('none')"
          >未指定（不設定地點）</button>
          <button
            type="button"
            class="pill"
            :class="mode === 'other' ? 'pill-tone-on' : ''"
            :aria-pressed="mode === 'other'"
            :disabled="busy"
            @click="pick('other')"
          >其他地點</button>
        </div>
        <div v-if="mode === 'other'" class="mt-3">
          <ComboField
            id="rl-dest"
            ref="otherEl"
            v-model="other"
            :suggestions="otherLocations"
            title="已使用過的存放地點"
            maxlength="50"
            placeholder="輸入或選擇要改放的地點"
          />
          <p v-if="problem" class="mt-1.5 pl-1 text-[12.5px] text-signal">{{ problem }}</p>
        </div>
      </div>

      <p v-if="hasStock" class="pl-1 text-[12.5px] leading-relaxed text-mute">庫存數量不會改變；歷史紀錄會保留，並為每個品項記下一筆移出、一筆移入。註記會跟著搬過去。</p>
      <p v-else class="pl-1 text-[12.5px] leading-relaxed text-mute">歷史紀錄會保留；之後再用同一個名稱入庫，這個地點就會重新出現。</p>
      <p v-if="project.readOnly" class="pl-1 text-[13px] font-semibold text-signal" role="alert">資料檔異常，目前為唯讀模式，無法移除存放地點</p>
    </div>

    <template #footer>
      <button type="button" class="btn-ghost" :disabled="busy" @click="emit('close')">取消</button>
      <button type="button" class="btn-danger" :disabled="!canSubmit" @click="submit">{{ busy ? '移除中…' : '移除地點' }}</button>
    </template>
  </ModalDialog>
</template>
