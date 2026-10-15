<script setup>
import { nextTick, ref } from 'vue'
import ModalDialog from './ModalDialog.vue'
import { useProjectStore } from '../stores/project'
import { useUiStore } from '../stores/ui'

const emit = defineEmits(['show-issues'])
const project = useProjectStore()
const ui = useUiStore()

const showNew = ref(false)
const newName = ref('')
const newError = ref('')
const busy = ref(false)
const nameInput = ref(null)
const confirmDelete = ref(false)

async function switchTo(e) {
  const name = e.target.value
  if (name === project.current) return
  try {
    await project.open(name)
    ui.go('dashboard')
    ui.ok(`已切換至「${name}」`)
  } catch (err) {
    ui.error(err)
    e.target.value = project.current
  }
}

async function openNew() {
  newName.value = ''
  newError.value = ''
  showNew.value = true
  await nextTick()
  nameInput.value?.focus()
}

async function createProject() {
  busy.value = true
  newError.value = ''
  try {
    const name = await project.create(newName.value)
    showNew.value = false
    ui.go('dashboard')
    ui.ok(`已建立並切換至「${name}」`)
  } catch (err) {
    newError.value = err.message
  } finally {
    busy.value = false
  }
}

async function deleteProject() {
  busy.value = true
  const name = project.current
  try {
    const opened = await project.remove(name)
    confirmDelete.value = false
    ui.go('dashboard')
    ui.ok(`已刪除專案「${name}」，目前開啟「${opened}」`)
  } catch (err) {
    ui.error(err)
  } finally {
    busy.value = false
  }
}

async function exportZip() {
  try {
    const path = await project.exportZip()
    if (path) ui.ok(`專案備份已匯出：\n${path}`)
  } catch (err) {
    ui.error(err)
  }
}
</script>

<template>
  <header class="flex shrink-0 items-center gap-2.5 px-7 pt-5 pb-1">
    <label class="pl-1 text-[13px] font-semibold text-mute" for="project-select">目前專案</label>
    <select
      id="project-select"
      class="field w-56 rounded-full py-2 font-semibold"
      :value="project.current"
      :disabled="project.loading"
      @change="switchTo"
    >
      <option v-for="p in project.projects" :key="p" :value="p">{{ p }}</option>
    </select>
    <button type="button" class="btn-ghost btn-sm" @click="openNew">新增專案</button>
    <button type="button" class="btn-ghost btn-sm text-signal" :disabled="!project.current" @click="confirmDelete = true">刪除專案</button>
    <button type="button" class="btn-ghost btn-sm" :disabled="!project.current" @click="exportZip">匯出備份 (ZIP)</button>

    <button
      v-if="project.readOnly"
      type="button"
      class="btn btn-sm ml-1 bg-signal-soft text-signal"
      @click="emit('show-issues')"
    >
      資料檔異常・唯讀模式
    </button>

    <div class="ml-auto truncate pl-4 text-[12px] text-mute" :title="project.rootPath">
      資料位置：<span class="selectable">{{ project.rootPath }}</span>
    </div>
  </header>

  <ModalDialog v-if="showNew" title="新增專案" @close="showNew = false">
    <form @submit.prevent="createProject">
      <label class="field-label" for="new-project">專案名稱</label>
      <input id="new-project" ref="nameInput" v-model="newName" class="field" placeholder="例如：分店倉" maxlength="50" />
      <p class="mt-2 pl-1 text-[12.5px] text-mute">會在 projects 資料夾下建立同名資料夾，並自動切換過去。</p>
      <p v-if="newError" class="mt-2 pl-1 text-[13px] text-signal">{{ newError }}</p>
    </form>
    <template #footer>
      <button type="button" class="btn-ghost" @click="showNew = false">取消</button>
      <button type="button" class="btn-primary" :disabled="busy || !newName.trim()" @click="createProject">建立專案</button>
    </template>
  </ModalDialog>

  <ModalDialog v-if="confirmDelete" title="刪除專案" @close="confirmDelete = false">
    <div class="flex gap-4">
      <div class="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-signal-soft text-signal" aria-hidden="true">
        <svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M12 8v5M12 16.5v.5M10.3 3.9 2.4 17.6A2 2 0 0 0 4.1 20.6h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z" /></svg>
      </div>
      <div class="text-[14.5px] leading-relaxed">
        <p>確定要刪除專案「<strong>{{ project.current }}</strong>」嗎？</p>
        <p class="mt-2 text-[13px] text-mute">
          此專案的品項、庫存與異動流水帳都會從系統中移除。資料夾會移到 projects 內的「_已刪除專案」，必要時仍可手動救回。
        </p>
      </div>
    </div>
    <template #footer>
      <button type="button" class="btn-ghost" @click="confirmDelete = false">取消</button>
      <button type="button" class="btn-danger" :disabled="busy" @click="deleteProject">刪除專案</button>
    </template>
  </ModalDialog>
</template>
