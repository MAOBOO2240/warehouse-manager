<script setup>
import { computed, nextTick, ref, watch } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import ModalDialog from '../components/ModalDialog.vue'
import { useLocationStore } from '../stores/locations'
import { useStockStore } from '../stores/stock'
import { useUiStore } from '../stores/ui'
import { useProjectStore } from '../stores/project'
import { fmtInt } from '../utils/format'

const locations = useLocationStore()
const stock = useStockStore()
const ui = useUiStore()
const project = useProjectStore()

const q = ref('')
const list = computed(() => {
  const k = q.value.trim().toLowerCase()
  if (!k) return locations.sorted
  return locations.sorted.filter((l) =>
    [l.id, l.zone, l.shelf, l.layer, l.description].some((v) => String(v ?? '').toLowerCase().includes(k)),
  )
})

const editing = ref(null)
const isNew = ref(false)
const idTouched = ref(false)
const formError = ref('')
const saving = ref(false)
const firstField = ref(null)
const confirmDelete = ref(null)

// 新增時依倉區 / 貨架 / 層號自動帶出代碼（手動改過代碼後就不再覆寫）
watch(
  () => editing.value && [editing.value.zone, editing.value.shelf, editing.value.layer],
  () => {
    if (editing.value && isNew.value && !idTouched.value) {
      editing.value.id = locations.suggestId(editing.value.zone, editing.value.shelf, editing.value.layer)
    }
  },
)

async function openNew() {
  editing.value = { id: '', zone: '', shelf: '', layer: '', description: '' }
  isNew.value = true
  idTouched.value = false
  formError.value = ''
  await nextTick()
  firstField.value?.focus()
}

async function openEdit(l) {
  editing.value = { ...l }
  isNew.value = false
  formError.value = ''
  await nextTick()
  firstField.value?.focus()
}

async function save() {
  saving.value = true
  formError.value = ''
  try {
    const saved = await locations.save(editing.value, isNew.value)
    ui.ok(isNew.value ? `已新增庫位 ${saved.id}` : `已更新庫位 ${saved.id}`)
    editing.value = null
  } catch (err) {
    formError.value = err.message
  } finally {
    saving.value = false
  }
}

async function remove() {
  const l = confirmDelete.value
  try {
    await locations.remove(l.id)
    ui.ok(`已刪除庫位 ${l.id}`)
  } catch (err) {
    ui.error(err)
  } finally {
    confirmDelete.value = null
  }
}

function itemCount(id) {
  return stock.rowsOfLocation(id).length
}
</script>

<template>
  <PageHeader title="庫位主檔" :hint="`共 ${locations.items.length} 個庫位`">
    <button type="button" class="btn-primary" :disabled="project.readOnly" @click="openNew">新增庫位</button>
  </PageHeader>

  <input v-model="q" class="field mb-4 max-w-md" placeholder="搜尋庫位代碼、倉區、描述" />

  <div class="panel overflow-hidden">
    <div class="max-h-[calc(100vh-15rem)] overflow-auto">
      <table class="table-base">
        <thead>
          <tr>
            <th>庫位代碼</th>
            <th>倉區</th>
            <th>貨架</th>
            <th>層號</th>
            <th>描述</th>
            <th class="text-right">品項數</th>
            <th class="text-right">在庫件數</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="l in list" :key="l.id">
            <td class="num font-semibold">{{ l.id }}</td>
            <td>{{ l.zone }}</td>
            <td class="num">{{ l.shelf }}</td>
            <td class="num">{{ l.layer }}</td>
            <td class="text-mute">{{ l.description }}</td>
            <td class="num text-right">{{ fmtInt(itemCount(l.id)) }}</td>
            <td class="num text-right text-[15px]">{{ fmtInt(stock.totalByLocation.get(l.id) || 0) }}</td>
            <td class="text-right whitespace-nowrap">
              <button type="button" class="btn-ghost btn-sm" :disabled="project.readOnly" @click="openEdit(l)">編輯</button>
              <button type="button" class="btn-ghost btn-sm ml-1 text-signal" :disabled="project.readOnly" @click="confirmDelete = l">刪除</button>
            </td>
          </tr>
          <tr v-if="!list.length">
            <td colspan="8" class="py-10 text-center text-mute">
              {{ locations.items.length ? '沒有符合條件的庫位' : '尚未建立任何庫位，按右上角「新增庫位」開始' }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>

  <ModalDialog v-if="editing" :title="isNew ? '新增庫位' : `編輯庫位 ${editing.id}`" @close="editing = null">
    <form class="grid grid-cols-3 gap-x-4 gap-y-3" @submit.prevent="save">
      <div>
        <label class="field-label" for="l-zone">倉區</label>
        <input id="l-zone" ref="firstField" v-model="editing.zone" class="field" placeholder="A區" />
      </div>
      <div>
        <label class="field-label" for="l-shelf">貨架號</label>
        <input id="l-shelf" v-model="editing.shelf" class="field num" placeholder="01" />
      </div>
      <div>
        <label class="field-label" for="l-layer">層號</label>
        <input id="l-layer" v-model="editing.layer" class="field num" placeholder="01" />
      </div>
      <div class="col-span-3">
        <label class="field-label" for="l-id">庫位代碼 *</label>
        <input
          id="l-id"
          v-model="editing.id"
          class="field num"
          :disabled="!isNew"
          placeholder="LOC-A-01-01"
          @input="idTouched = true"
        />
        <p class="mt-1 text-[12px] text-mute">
          {{ isNew ? '依倉區、貨架、層號自動產生，也可自行修改。建立後不可更改。' : '庫位代碼建立後不可更改。' }}
        </p>
      </div>
      <div class="col-span-3">
        <label class="field-label" for="l-desc">描述</label>
        <input id="l-desc" v-model="editing.description" class="field" placeholder="例如：飲品展示架第一層" />
      </div>
      <p v-if="formError" class="col-span-3 text-[13px] text-signal">{{ formError }}</p>
      <button type="submit" class="hidden" />
    </form>
    <template #footer>
      <button type="button" class="btn-ghost" @click="editing = null">取消</button>
      <button type="button" class="btn-primary" :disabled="saving" @click="save">儲存庫位</button>
    </template>
  </ModalDialog>

  <ModalDialog v-if="confirmDelete" title="刪除庫位" @close="confirmDelete = null">
    <p>確定要刪除庫位 <strong class="num">{{ confirmDelete.id }}</strong> 嗎？</p>
    <p class="mt-2 text-[13px] text-mute">尚有庫存的庫位無法刪除，請先調撥或盤點歸零。</p>
    <template #footer>
      <button type="button" class="btn-ghost" @click="confirmDelete = null">取消</button>
      <button type="button" class="btn-danger" @click="remove">刪除庫位</button>
    </template>
  </ModalDialog>
</template>
