<script setup>
import { computed } from 'vue'
import ModalDialog from './ModalDialog.vue'
import { useUpdateStore } from '../stores/update'

const update = useUpdateStore()

const when = computed(() => {
  const d = new Date(update.result?.published || '')
  return Number.isNaN(d.getTime()) ? '' : `${d.getFullYear()}/${d.getMonth() + 1}/${d.getDate()} 發布`
})
const subtitle = computed(() => [update.result?.latest_build ? `編譯 #${update.result.latest_build}` : '', when.value].filter(Boolean).join('・'))
</script>

<template>
  <ModalDialog title="有新版本" :subtitle="subtitle || '倉儲管理系統'" tone="accent" :closable="!update.installing" @close="update.later()">
    <template #icon>
      <svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 3.5v11M7.5 10 12 14.5 16.5 10M4.5 18.5h15" />
      </svg>
    </template>
    <div class="tone-card text-[14.5px] leading-relaxed">
      <p>要現在更新嗎？</p>
      <p class="mt-2 text-[13px] text-ink-2">會自動下載新版本、替換程式並重新開啟，只要幾秒鐘。所有專案資料都在 projects 資料夾裡，更新不會動到。</p>
    </div>
    <p v-if="update.info?.build" class="mt-3 pl-1 text-[12.5px] text-mute">目前是編譯 #{{ update.info.build }}</p>
    <template #footer>
      <button type="button" class="btn-ghost" :disabled="update.installing" @click="update.later()">稍後</button>
      <button type="button" class="btn-primary" :disabled="update.installing" @click="update.install()">
        {{ update.installing ? '下載中…' : '立即更新' }}
      </button>
    </template>
  </ModalDialog>
</template>
