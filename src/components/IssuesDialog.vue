<script setup>
// 開啟專案時偵測到資料檔損壞或被鎖定 → 詢問是否從最近一次 .bak 備份還原
import { computed, ref } from 'vue'
import ModalDialog from './ModalDialog.vue'
import { FILE_LABELS, useProjectStore } from '../stores/project'
import { useUiStore } from '../stores/ui'

const emit = defineEmits(['close'])
const project = useProjectStore()
const ui = useUiStore()
const busy = ref(false)

const hasLocked = computed(() => project.issues.some((i) => i.code === 'FILE_LOCKED'))
const restorable = computed(() => project.issues.filter((i) => i.code !== 'FILE_LOCKED' && i.has_backup))

async function restore() {
  busy.value = true
  try {
    const failed = await project.restoreFromBackups()
    if (!project.issues.length) {
      ui.ok('已從備份還原，資料檔恢復正常')
      emit('close')
    } else if (failed.length) {
      ui.error(`仍有 ${failed.length} 個檔案無法還原，請參考說明處理`)
    }
  } catch (err) {
    ui.error(err)
  } finally {
    busy.value = false
  }
}

async function retry() {
  busy.value = true
  try {
    await project.open(project.current)
    if (!project.issues.length) {
      ui.ok('資料檔已恢復正常')
      emit('close')
    }
  } catch (err) {
    ui.error(err)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <ModalDialog title="偵測到資料檔異常" width="max-w-xl" @close="emit('close')">
    <p class="text-[14px] leading-relaxed">
      專案「{{ project.current }}」有以下資料檔無法正常讀取。為避免覆蓋資料，系統目前為<strong>唯讀模式</strong>。
    </p>
    <ul class="mt-3 space-y-2">
      <li v-for="i in project.issues" :key="i.file" class="rounded-lg border border-signal/25 bg-signal-soft/60 px-3 py-2">
        <div class="font-semibold">{{ FILE_LABELS[i.file] ?? i.file }}</div>
        <div class="mt-0.5 text-[13px] text-ink-3">{{ i.message }}</div>
        <div class="mt-0.5 text-[12.5px]" :class="i.has_backup ? 'text-go' : 'text-signal'">
          {{ i.code === 'FILE_LOCKED' ? '請關閉正在開啟此檔的程式後按「重新讀取」' : i.has_backup ? '有可用的 .bak 備份' : '找不到可用的備份' }}
        </div>
      </li>
    </ul>
    <p v-if="restorable.length" class="mt-4 text-[14px] font-semibold">是否從最近一次 .bak 備份還原？</p>
    <p class="mt-2 text-[12.5px] text-mute">還原前，損壞的原檔會另存在 backups 資料夾（檔名含 corrupt），不會遺失。</p>
    <template #footer>
      <button type="button" class="btn-ghost" @click="emit('close')">稍後處理</button>
      <button v-if="hasLocked" type="button" class="btn-ghost" :disabled="busy" @click="retry">重新讀取</button>
      <button v-if="restorable.length" type="button" class="btn-danger" :disabled="busy" @click="restore">從備份還原</button>
    </template>
  </ModalDialog>
</template>
