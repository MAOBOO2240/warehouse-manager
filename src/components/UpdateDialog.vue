<script setup>
import { computed } from 'vue'
import ModalDialog from './ModalDialog.vue'
import { useUpdateStore } from '../stores/update'

const update = useUpdateStore()

const when = computed(() => {
  const d = new Date(update.result?.published || '')
  return Number.isNaN(d.getTime()) ? '' : `${d.getFullYear()}/${d.getMonth() + 1}/${d.getDate()} 發布`
})
const latestBuild = computed(() => update.result?.latest_build || '')
// 同一個版本重新編譯過（例如換了更新權杖）：編譯編號和目前的一樣，所以另外註明
const rebuilt = computed(() => !!update.result?.rebuilt)
const subtitle = computed(() =>
  [latestBuild.value ? `編譯 #${latestBuild.value}${rebuilt.value ? '（重新編譯）' : ''}` : '', when.value].filter(Boolean).join('・'),
)
// 「自動更新」開著、剛開啟程式就發現新版本，分兩段：
//   counting＝倒數幾秒（這時可以按「稍後」取消，或按「立即更新」不用等）
//   autoRunning＝正在下載安裝（裝好會自己重新開啟，所以沒有按鈕、也不能關）
const counting = computed(() => update.auto && update.countdown > 0 && !update.installing)
const autoRunning = computed(() => update.auto && update.installing)
const title = computed(() => (autoRunning.value ? '正在自動更新' : counting.value ? '即將自動更新' : '有新版本'))
const found = computed(() => `發現${rebuilt.value ? '重新編譯的版本' : '新版本'}${latestBuild.value ? `（編譯 #${latestBuild.value}）` : ''}`)
</script>

<template>
  <ModalDialog :title="title" :subtitle="subtitle || '倉儲管理系統'" tone="accent" :closable="!update.installing" @close="update.later()">
    <template #icon>
      <svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 3.5v11M7.5 10 12 14.5 16.5 10M4.5 18.5h15" />
      </svg>
    </template>

    <div v-if="autoRunning" class="tone-card flex items-start gap-4" role="status" aria-live="polite">
      <span class="spinner mt-0.5" aria-hidden="true" />
      <div class="min-w-0 flex-1 text-[14.5px] leading-relaxed">
        <p class="font-semibold">{{ found }}，正在下載並安裝…</p>
        <p class="mt-2 text-[13px] text-ink-2">完成後會自動重新開啟，只要幾秒鐘。所有專案資料都在 projects 資料夾裡，更新不會動到。</p>
      </div>
    </div>
    <!-- 倒數：左邊一個大數字，一眼看得出還有幾秒 -->
    <div v-else-if="counting" class="tone-card flex items-center gap-4">
      <span
        class="num flex h-14 w-14 shrink-0 items-center justify-center rounded-full text-[28px] leading-none"
        :style="{ color: 'var(--tone-fg)', background: 'var(--tone)' }"
        aria-hidden="true"
      >{{ update.countdown }}</span>
      <div class="min-w-0 flex-1 text-[14.5px] leading-relaxed">
        <!-- 給螢幕報讀的文字只在開始倒數時唸一次，不會每秒重唸 -->
        <p class="font-semibold" role="status">{{ found }}，<span aria-hidden="true">{{ update.countdown }} 秒後</span>自動下載並安裝</p>
        <p class="mt-2 text-[13px] text-ink-2">完成後會自動重新開啟，只要幾秒鐘。所有專案資料都在 projects 資料夾裡，更新不會動到。</p>
      </div>
    </div>
    <div v-else class="tone-card text-[14.5px] leading-relaxed">
      <p>{{ rebuilt ? '這個版本重新編譯過，要現在更新嗎？' : '要現在更新嗎？' }}</p>
      <p class="mt-2 text-[13px] text-ink-2">會自動下載新版本、替換程式並重新開啟，只要幾秒鐘。所有專案資料都在 projects 資料夾裡，更新不會動到。</p>
    </div>
    <p v-if="counting" class="mt-3 pl-1 text-[12.5px] text-mute">現在不方便的話請按「稍後」，下次開啟程式時再更新。</p>
    <p v-if="counting || autoRunning" class="pl-1 text-[12.5px] text-mute" :class="counting ? 'mt-1' : 'mt-3'">不想自動更新的話，可以到左下角「設定」關閉。</p>
    <p v-if="update.info?.build" class="pl-1 text-[12.5px] text-mute" :class="counting || autoRunning ? 'mt-1' : 'mt-3'">目前是編譯 #{{ update.info.build }}</p>
    <!-- 上一次沒裝成功（例如網路中斷）：留著原因，讓使用者決定要不要再試 -->
    <p v-if="update.error && !autoRunning" class="selectable mt-4 rounded-2xl bg-signal-soft px-4 py-3 text-[13.5px] leading-relaxed text-signal" role="alert">
      上次更新沒有成功：{{ update.error }}
    </p>

    <template v-if="!autoRunning" #footer>
      <button type="button" class="btn-ghost" :disabled="update.installing" @click="update.later()">稍後</button>
      <button type="button" class="btn-primary" :disabled="update.installing" @click="update.install()">
        {{ update.installing ? '下載中…' : update.error ? '再試一次' : '立即更新' }}
      </button>
    </template>
  </ModalDialog>
</template>
