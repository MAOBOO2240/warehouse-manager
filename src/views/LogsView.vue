<script setup>
import { computed, ref } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import ActionBadge from '../components/ActionBadge.vue'
import DateField from '../components/DateField.vue'
import { useStockStore } from '../stores/stock'
import { useUiStore } from '../stores/ui'
import { useProjectStore } from '../stores/project'
import { ACTIONS, ACTIVE_ACTIONS, actionLabel, fmtDelta, fmtInt, fmtTime, localDate, locLabel, todayStamp } from '../utils/format'
import { saveCsv } from '../utils/csv'

const stock = useStockStore()
const ui = useUiStore()
const project = useProjectStore()

const DISPLAY_LIMIT = 500

const type = ref('')
const from = ref('')
const to = ref('')
const q = ref('')

// 篩選：操作類型、起訖日期（本機日期）、品項名稱 / 存放地點 / 原因 / 備註
const filtered = computed(() => {
  const k = q.value.trim().toLowerCase()
  return stock.logs
    .filter((l) => {
      if (type.value && l.action_type !== type.value) return false
      const d = localDate(l.timestamp)
      if (from.value && d < from.value) return false
      if (to.value && d > to.value) return false
      if (k) {
        const hay = [l.product_name, l.location_id, l.note, l.reason]
        if (!hay.some((v) => String(v ?? '').toLowerCase().includes(k))) return false
      }
      return true
    })
    .reverse()
})
const shown = computed(() => filtered.value.slice(0, DISPLAY_LIMIT))

function reset() {
  type.value = ''
  from.value = ''
  to.value = ''
  q.value = ''
}

async function exportCsv() {
  try {
    const range = from.value || to.value ? `_${from.value || '起'}至${to.value || '今'}` : ''
    const path = await saveCsv(
      `${project.current}_異動流水帳${range}_${todayStamp()}.csv`,
      ['流水號', '時間', '類型', '品項', '存放地點', '異動數量', '異動後結存', '原因', '備註'],
      [...filtered.value].reverse().map((l) => [
        l.id, fmtTime(l.timestamp), actionLabel(l.action_type), l.product_name,
        locLabel(l.location_id), l.delta, l.balance_after, l.reason, l.note,
      ]),
    )
    if (path) ui.ok(`已匯出 ${filtered.value.length} 筆流水帳：\n${path}`)
  } catch (err) {
    ui.error(err)
  }
}
</script>

<template>
  <PageHeader title="異動流水帳" hint="所有庫存異動的完整紀錄，不可修改或刪除">
    <button type="button" class="btn-primary" :disabled="!filtered.length" @click="exportCsv">匯出 CSV</button>
  </PageHeader>

  <div class="panel mb-5 flex flex-wrap items-end gap-4 p-5">
    <div>
      <label class="field-label" for="lg-type">操作類型</label>
      <select id="lg-type" v-model="type" class="field w-40 rounded-full">
        <option value="">全部</option>
        <option v-for="key in ACTIVE_ACTIONS" :key="key" :value="key">{{ ACTIONS[key].label }}</option>
      </select>
    </div>
    <DateField id="lg-from" v-model="from" label="起始日期" class="w-56" />
    <DateField id="lg-to" v-model="to" label="結束日期" class="w-56" />
    <div class="min-w-64 flex-1">
      <label class="field-label" for="lg-q">品項或存放地點</label>
      <input id="lg-q" v-model="q" class="field rounded-full" placeholder="輸入關鍵字" />
    </div>
    <button type="button" class="btn-ghost" @click="reset">清除條件</button>
  </div>

  <div class="panel overflow-hidden">
    <div class="max-h-[calc(100vh-22rem)] overflow-auto">
      <table class="table-base">
        <thead>
          <tr>
            <th>時間</th>
            <th>類型</th>
            <th>品項</th>
            <th>存放地點</th>
            <th class="text-right">異動</th>
            <th class="text-right">結存</th>
            <th>原因</th>
            <th>備註</th>
            <th>流水號</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="l in shown" :key="l.id">
            <td class="num whitespace-nowrap">{{ fmtTime(l.timestamp) }}</td>
            <td><ActionBadge :type="l.action_type" /></td>
            <td>{{ l.product_name }}</td>
            <td :class="l.location_id ? '' : 'italic text-mute'">{{ locLabel(l.location_id) }}</td>
            <td class="num text-right text-[15px]" :class="l.delta < 0 ? 'text-signal' : 'text-go'">{{ fmtDelta(l.delta) }}</td>
            <td class="num text-right">{{ fmtInt(l.balance_after) }}</td>
            <td class="max-w-56 truncate" :title="l.reason">{{ l.reason }}</td>
            <td class="max-w-56 truncate text-mute" :title="l.note">{{ l.note }}</td>
            <td class="num text-[12px] text-mute">{{ l.id }}</td>
          </tr>
          <tr v-if="!filtered.length">
            <td colspan="9" class="py-10 text-center text-mute">{{ stock.logs.length ? '沒有符合條件的紀錄' : '尚無任何異動紀錄' }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <div class="px-6 py-3.5 text-right text-[13px] text-mute" style="box-shadow: inset 0 1px 0 var(--color-line)">
      符合條件 <span class="num">{{ fmtInt(filtered.length) }}</span> 筆
      <span v-if="filtered.length > DISPLAY_LIMIT" class="text-mute">（畫面顯示最新 {{ DISPLAY_LIMIT }} 筆，匯出 CSV 包含全部）</span>
    </div>
  </div>
</template>
