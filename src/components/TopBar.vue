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
  <header class="flex h-14 shrink-0 items-center gap-3 border-b border-line bg-white px-5">
    <label class="text-[13px] text-mute" for="project-select">目前專案</label>
    <select
      id="project-select"
      class="field w-56 py-1.5 font-semibold"
      :value="project.current"
      :disabled="project.loading"
      @change="switchTo"
    >
      <option v-for="p in project.projects" :key="p" :value="p">{{ p }}</option>
    </select>
    <button type="button" class="btn-ghost btn-sm" @click="openNew">新增專案</button>
    <button type="button" class="btn-ghost btn-sm" :disabled="!project.current" @click="exportZip">匯出專案備份 (ZIP)</button>

    <button
      v-if="project.readOnly"
      type="button"
      class="ml-2 rounded-md bg-signal-soft px-3 py-1.5 text-[13px] font-semibold text-signal"
      @click="emit('show-issues')"
    >
      資料檔異常・唯讀模式
    </button>

    <div class="ml-auto truncate text-[12px] text-mute" :title="project.rootPath">
      資料位置：<span class="selectable">{{ project.rootPath }}</span>
    </div>
  </header>

  <ModalDialog v-if="showNew" title="新增專案" @close="showNew = false">
    <form @submit.prevent="createProject">
      <label class="field-label" for="new-project">專案名稱</label>
      <input id="new-project" ref="nameInput" v-model="newName" class="field" placeholder="例如：分店倉" maxlength="50" />
      <p class="mt-2 text-[12.5px] text-mute">會在 projects 資料夾下建立同名資料夾，並自動切換過去。</p>
      <p v-if="newError" class="mt-2 text-[13px] text-signal">{{ newError }}</p>
    </form>
    <template #footer>
      <button type="button" class="btn-ghost" @click="showNew = false">取消</button>
      <button type="button" class="btn-primary" :disabled="busy || !newName.trim()" @click="createProject">建立專案</button>
    </template>
  </ModalDialog>
</template>
