<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import AppSidebar from './components/AppSidebar.vue'
import TopBar from './components/TopBar.vue'
import ToastHost from './components/ToastHost.vue'
import IssuesDialog from './components/IssuesDialog.vue'
import UpdateDialog from './components/UpdateDialog.vue'
import DashboardView from './views/DashboardView.vue'
import InboundView from './views/InboundView.vue'
import OutboundView from './views/OutboundView.vue'
import InventoryView from './views/InventoryView.vue'
import LogsView from './views/LogsView.vue'
import { useProjectStore } from './stores/project'
import { useUiStore } from './stores/ui'
import { useUpdateStore } from './stores/update'

const project = useProjectStore()
const ui = useUiStore()
const update = useUpdateStore()

const views = {
  dashboard: DashboardView,
  inbound: InboundView,
  outbound: OutboundView,
  inventory: InventoryView,
  logs: LogsView,
}
const view = computed(() => views[ui.page] ?? DashboardView)

const showIssues = ref(false)

// 每次開啟專案後：有異常檔就跳出還原詢問；有補完中斷交易就提示
watch(
  () => [project.current, project.issues, project.recovered],
  () => {
    if (project.issues.length) showIssues.value = true
    if (project.recovered) {
      ui.ok('偵測到上次未完成的異動（可能因斷電中斷），已自動補完並保持庫存與歷史紀錄一致')
      project.recovered = false
    }
  },
)

onMounted(async () => {
  await project.init()
  // 開啟程式時檢查一次更新（等畫面出來再查，不影響開啟速度；查不到就算了）
  await update.loadInfo()
  if (!project.fatal) setTimeout(() => update.check(false), 1500)
})
</script>

<template>
  <div v-if="!project.ready" class="flex h-full items-center justify-center text-mute">載入中…</div>

  <div v-else-if="project.fatal" class="flex h-full items-center justify-center p-10">
    <div class="panel max-w-xl p-8">
      <h1 class="text-xl font-bold text-signal">無法啟動</h1>
      <p class="mt-3 leading-relaxed whitespace-pre-line">{{ project.fatal }}</p>
    </div>
  </div>

  <div v-else class="flex h-full">
    <AppSidebar />
    <div class="flex min-w-0 flex-1 flex-col">
      <TopBar @show-issues="showIssues = true" />
      <main class="min-h-0 flex-1 overflow-auto px-7 py-6">
        <component :is="view" :key="`${project.current}:${ui.page}`" />
      </main>
    </div>
  </div>

  <IssuesDialog v-if="showIssues && project.issues.length" @close="showIssues = false" />
  <UpdateDialog v-if="update.show && !(showIssues && project.issues.length)" />
  <ToastHost />
</template>
