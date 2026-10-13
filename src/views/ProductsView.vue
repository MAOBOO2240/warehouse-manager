<script setup>
import { computed, nextTick, ref } from 'vue'
import PageHeader from '../components/PageHeader.vue'
import ModalDialog from '../components/ModalDialog.vue'
import { EMPTY_PRODUCT, useProductStore } from '../stores/products'
import { useStockStore } from '../stores/stock'
import { useUiStore } from '../stores/ui'
import { useProjectStore } from '../stores/project'
import { fmtInt, fmtMoney } from '../utils/format'

const products = useProductStore()
const stock = useStockStore()
const ui = useUiStore()
const project = useProjectStore()

const q = ref('')
const category = ref('')
const list = computed(() => products.search(q.value, category.value))

const editing = ref(null) // 表單資料
const formError = ref('')
const saving = ref(false)
const firstField = ref(null)
const confirmDelete = ref(null)

// 搜尋框按 Enter：條碼完全相符就直接開啟編輯
function onSearchEnter() {
  const hit = products.findExact(q.value)
  if (hit) openEdit(hit)
}

async function openNew() {
  editing.value = EMPTY_PRODUCT()
  formError.value = ''
  await nextTick()
  firstField.value?.focus()
}

async function openEdit(p) {
  editing.value = { ...p }
  formError.value = ''
  await nextTick()
  firstField.value?.focus()
}

async function save() {
  saving.value = true
  formError.value = ''
  try {
    const isNew = !editing.value.id
    const saved = await products.save(editing.value)
    editing.value = null
    ui.ok(isNew ? `已新增商品「${saved.name}」（${saved.id}）` : `已更新商品「${saved.name}」`)
  } catch (err) {
    formError.value = err.message
  } finally {
    saving.value = false
  }
}

async function remove() {
  const p = confirmDelete.value
  try {
    await products.remove(p.id)
    ui.ok(`已刪除商品「${p.name}」`)
  } catch (err) {
    ui.error(err)
  } finally {
    confirmDelete.value = null
  }
}

const fields = [
  { key: 'sku', label: 'SKU' },
  { key: 'barcode', label: '條碼', hint: '游標放在此欄可直接用掃描槍掃入' },
  { key: 'category', label: '分類', list: true },
  { key: 'spec', label: '規格' },
  { key: 'unit', label: '單位' },
]
const numbers = [
  { key: 'cost_price', label: '進貨成本', step: '0.01' },
  { key: 'sale_price', label: '零售售價', step: '0.01' },
  { key: 'min_stock', label: '最低安全庫存', step: '1' },
  { key: 'max_stock', label: '最高庫存', step: '1' },
]
</script>

<template>
  <PageHeader title="商品主檔" :hint="`共 ${products.items.length} 項商品`">
    <button type="button" class="btn-primary" :disabled="project.readOnly" @click="openNew">新增商品</button>
  </PageHeader>

  <div class="mb-4 flex gap-3">
    <input
      v-model="q"
      class="scan-field max-w-md"
      placeholder="搜尋名稱、條碼、SKU（掃描條碼按 Enter 直接開啟）"
      @keydown.enter.prevent="onSearchEnter"
    />
    <select v-model="category" class="field w-48">
      <option value="">全部分類</option>
      <option v-for="c in products.categories" :key="c" :value="c">{{ c }}</option>
    </select>
  </div>

  <div class="panel overflow-hidden">
    <div class="max-h-[calc(100vh-15rem)] overflow-auto">
      <table class="table-base">
        <thead>
          <tr>
            <th>編號</th>
            <th>商品名稱</th>
            <th>SKU / 條碼</th>
            <th>分類</th>
            <th>規格</th>
            <th class="text-right">成本</th>
            <th class="text-right">售價</th>
            <th class="text-right">安全量</th>
            <th class="text-right">全庫存量</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="p in list" :key="p.id">
            <td class="num text-mute">{{ p.id }}</td>
            <td class="font-semibold">{{ p.name }}</td>
            <td class="num text-[12.5px]">
              <div>{{ p.sku || '—' }}</div>
              <div class="text-mute">{{ p.barcode || '—' }}</div>
            </td>
            <td>{{ p.category }}</td>
            <td class="text-mute">{{ p.spec }}<span v-if="p.unit">／{{ p.unit }}</span></td>
            <td class="num text-right">{{ fmtMoney(p.cost_price) }}</td>
            <td class="num text-right">{{ fmtMoney(p.sale_price) }}</td>
            <td class="num text-right text-mute">{{ fmtInt(p.min_stock) }}</td>
            <td
              class="num text-right text-[15px]"
              :class="p.min_stock > 0 && stock.total(p.id) <= p.min_stock ? 'font-bold text-signal' : ''"
            >{{ fmtInt(stock.total(p.id)) }}</td>
            <td class="text-right whitespace-nowrap">
              <button type="button" class="btn-ghost btn-sm" :disabled="project.readOnly" @click="openEdit(p)">編輯</button>
              <button type="button" class="btn-ghost btn-sm ml-1 text-signal" :disabled="project.readOnly" @click="confirmDelete = p">刪除</button>
            </td>
          </tr>
          <tr v-if="!list.length">
            <td colspan="10" class="py-10 text-center text-mute">
              {{ products.items.length ? '沒有符合條件的商品' : '尚未建立任何商品，按右上角「新增商品」開始' }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>

  <ModalDialog v-if="editing" :title="editing.id ? `編輯商品 ${editing.id}` : '新增商品'" width="max-w-2xl" @close="editing = null">
    <form class="grid grid-cols-2 gap-x-4 gap-y-3" @submit.prevent="save">
      <div class="col-span-2">
        <label class="field-label" for="p-name">商品名稱 *</label>
        <input id="p-name" ref="firstField" v-model="editing.name" class="field" maxlength="100" />
      </div>
      <div v-for="f in fields" :key="f.key">
        <label class="field-label" :for="`p-${f.key}`">{{ f.label }}</label>
        <input
          :id="`p-${f.key}`"
          v-model="editing[f.key]"
          :class="f.key === 'barcode' ? 'scan-field py-2 text-[15px]' : 'field'"
          :list="f.list ? 'category-list' : undefined"
          maxlength="80"
          @keydown.enter.prevent="f.key === 'barcode' ? undefined : save()"
        />
        <p v-if="f.hint" class="mt-1 text-[12px] text-mute">{{ f.hint }}</p>
      </div>
      <div v-for="f in numbers" :key="f.key">
        <label class="field-label" :for="`p-${f.key}`">{{ f.label }}</label>
        <input :id="`p-${f.key}`" v-model="editing[f.key]" type="number" min="0" :step="f.step" class="field num" />
      </div>
      <datalist id="category-list">
        <option v-for="c in products.categories" :key="c" :value="c" />
      </datalist>
      <p v-if="formError" class="col-span-2 text-[13px] text-signal">{{ formError }}</p>
      <button type="submit" class="hidden" />
    </form>
    <template #footer>
      <button type="button" class="btn-ghost" @click="editing = null">取消</button>
      <button type="button" class="btn-primary" :disabled="saving" @click="save">儲存商品</button>
    </template>
  </ModalDialog>

  <ModalDialog v-if="confirmDelete" title="刪除商品" @close="confirmDelete = null">
    <p>確定要刪除「<strong>{{ confirmDelete.name }}</strong>」嗎？</p>
    <p class="mt-2 text-[13px] text-mute">尚有庫存的商品無法刪除。已寫入流水帳的歷史紀錄會保留原商品名稱。</p>
    <template #footer>
      <button type="button" class="btn-ghost" @click="confirmDelete = null">取消</button>
      <button type="button" class="btn-danger" @click="remove">刪除商品</button>
    </template>
  </ModalDialog>
</template>
