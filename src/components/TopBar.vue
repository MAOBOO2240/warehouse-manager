<script setup>
import { nextTick, ref } from 'vue'
import ModalDialog from './ModalDialog.vue'
import ProjectSwitcher from './ProjectSwitcher.vue'
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

async function switchTo(name) {
  if (name === project.current) return
  try {
    await project.open(name)
    ui.go('dashboard')
    ui.ok(`已切換至「${name}」`)
  } catch (err) {
    ui.error(err)
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

async function importZip() {
  busy.value = true
  try {
    const name = await project.importZip()
    if (!name) return
    ui.go('dashboard')
    ui.ok(`已將備份匯入為新專案「${name}」，並切換過去`)
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
    <ProjectSwitcher :projects="project.projects" :current="project.current" :disabled="project.loading" @select="switchTo" />
    <button type="button" class="btn-ghost btn-sm" @click="openNew">新增專案</button>
    <button type="button" class="btn-ghost btn-sm text-signal" :disabled="!project.current" @click="confirmDelete = true">刪除專案</button>
    <button type="button" class="btn-ghost btn-sm" :disabled="busy" title="選擇先前匯出的備份 ZIP，匯入成一個新專案" @click="importZip">匯入備份 (ZIP)</button>
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

  <ModalDialog v-if="showNew" title="新增專案" subtitle="建立一個獨立的倉庫資料" tone="go" @close="showNew = false">
    <template #icon>
      <svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M12 5v14M5 12h14" /></svg>
    </template>
    <form @submit.prevent="createProject">
      <label class="field-label" for="new-project">專案名稱</label>
      <input id="new-project" ref="nameInput" v-model="newName" class="field" placeholder="例如：分店倉" maxlength="50" />
      <p class="mt-2 pl-1 text-[12.5px] text-mute">會在 projects 資料夾下建立同名資料夾，並自動切換過去。</p>
      <p v-if="newError" class="mt-2 pl-1 text-[13px] text-signal">{{ newError }}</p>
    </form>
    <template #footer>
      <button type="button" class="btn-ghost" @click="showNew = false">取消</button>
      <button type="button" class="btn-go" :disabled="busy || !newName.trim()" @click="createProject">建立專案</button>
    </template>
  </ModalDialog>

  <ModalDialog v-if="confirmDelete" title="刪除專案" subtitle="此動作需要再次確認" tone="signal" @close="confirmDelete = false">
    <template #icon>
      <svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 9v4.5M12 17v.01M10.3 3.9 2.4 17.6A2 2 0 0 0 4.1 20.6h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z" /></svg>
    </template>
    <div class="tone-card">
      <div class="text-[14.5px] leading-relaxed">
        <p>確定要刪除專案「<strong>{{ project.current }}</strong>」嗎？</p>
        <p class="mt-2 text-[13px] text-ink-2">
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
