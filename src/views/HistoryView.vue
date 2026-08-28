<script setup lang="ts">
import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { SButton } from '@/ui/components/button'
import { SDialog } from '@/ui/components/dialog'
import { SIcon } from '@/ui/components/icon'
import { type TransferTask, useTransferStore } from '@/stores/transfer'
import { isTauri } from '@/utils/tauri'

const transferStore = useTransferStore()
const showClearConfirm = ref(false)
const records = computed(() =>
  transferStore.tasks
    .filter(task => task.status === 'completed' || task.status === 'failed')
    .slice()
    .sort((a, b) => (b.createdAt ?? 0) - (a.createdAt ?? 0)),
)
const completedCount = computed(
  () => records.value.filter(item => item.status === 'completed').length,
)
const failedCount = computed(() => records.value.filter(item => item.status === 'failed').length)
const openingRecordId = ref('')
const feedback = ref('')
const feedbackIsError = ref(false)
const isAndroid = typeof navigator !== 'undefined' && /Android/i.test(navigator.userAgent)

function recordOpenPath(item: TransferTask) {
  return item.fileOpenPath || item.filePath
}

function recordPathLabel(item: TransferTask) {
  if (isAndroid && item.direction === 'send') return '来自手机文件'
  return item.filePath
}

function canOpenRecord(item: TransferTask) {
  const path = recordOpenPath(item)
  if (!path) return false
  return !isAndroid || (item.direction === 'receive' && path.startsWith('content://'))
}

function formatBytes(bytes: number) {
  if (!bytes) return '未知大小'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`
}

function recordStatus(status: TransferTask['status']) {
  return status === 'completed' ? '已完成' : '失败'
}

function formatTime(createdAt?: number) {
  if (!createdAt) return ''
  try {
    return new Intl.DateTimeFormat('zh-CN', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    }).format(new Date(createdAt))
  } catch {
    return new Date(createdAt).toLocaleString()
  }
}

function clearFeedback() {
  feedback.value = ''
  feedbackIsError.value = false
}

async function openRecordLocation(item: TransferTask) {
  clearFeedback()
  const path = recordOpenPath(item)
  if (!path) {
    feedbackIsError.value = true
    feedback.value = '这条记录没有可用的本地文件路径'
    return
  }
  if (isAndroid && !canOpenRecord(item)) {
    feedbackIsError.value = true
    feedback.value = '这条 Android 记录没有可用的文件 URI'
    return
  }
  if (!isTauri()) {
    feedbackIsError.value = true
    feedback.value = '浏览器预览无法打开本机目录，请在桌面应用中使用此功能'
    return
  }

  openingRecordId.value = item.id
  try {
    await invoke('open_file_location', { path, fileName: item.fileName })
  } catch (error) {
    feedbackIsError.value = true
    feedback.value = String(error).replace(/^Error:\s*/, '') || '无法打开文件所在目录'
  } finally {
    openingRecordId.value = ''
  }
}

function removeRecord(item: TransferTask) {
  transferStore.removeTask(item.id)
  feedbackIsError.value = false
  feedback.value = `已删除「${item.fileName}」的传输记录`
}

function confirmClearHistory() {
  showClearConfirm.value = true
}

function doClearHistory() {
  transferStore.clearHistory()
  showClearConfirm.value = false
  feedbackIsError.value = false
  feedback.value = '传输记录已清空，文件本身未被删除'
}
</script>

<template>
  <div class="mx-auto flex w-full max-w-5xl flex-col gap-6 p-4 md:p-8">
    <div class="relative min-w-0">
      <div class="flex min-w-0 items-center gap-2 pr-20 sm:pr-36">
        <h1 class="text-2xl font-bold tracking-tight">传输记录</h1>
        <span
          v-if="records.length"
          class="rounded-full bg-muted/80 px-2 py-0.5 text-[10px] font-medium text-muted-foreground"
        >
          {{ records.length }} 条
        </span>
      </div>
      <p class="mt-1 text-sm text-muted-foreground">
        完整查看历史收发文件，文件本身不会因删除记录而受影响
      </p>
      <div class="absolute right-0 top-0 flex items-center gap-2">
        <div
          v-if="records.length"
          class="hidden items-center gap-2 text-[11px] text-muted-foreground sm:flex"
        >
          <span class="text-success">{{ completedCount }} 已完成</span>
          <span v-if="failedCount" class="text-destructive">{{ failedCount }} 失败</span>
        </div>
        <SButton
          v-if="records.length"
          variant="outline"
          size="sm"
          class="shrink-0"
          @click="confirmClearHistory"
        >
          <SIcon icon="lucide:trash-2" />
          <span class="hidden sm:inline">清空记录</span>
          <span class="sm:hidden">清空</span>
        </SButton>
      </div>
    </div>

    <!--
 SDialog 会把未传的布尔 prop 转发为 false，吞掉库内默认值，
         因此 show-confirm / show-cancel 必须显式给出 
-->
    <SDialog
      v-model:open="showClearConfirm"
      title="清空传输记录"
      description="确定清空全部传输记录吗？文件本身不会被删除。"
      size="sm"
      is-alert
      alert-type="warning"
      :show-confirm="true"
      :show-cancel="true"
      confirm-text="清空"
      cancel-text="取消"
      @confirm="doClearHistory"
      @cancel="showClearConfirm = false"
    />

    <div
      v-if="feedback"
      class="flex items-start gap-2 rounded-xl border px-3 py-2.5 text-xs"
      :class="
        feedbackIsError
          ? 'border-destructive/15 bg-destructive/8 text-destructive'
          : 'border-success/15 bg-success/8 text-success'
      "
      role="status"
    >
      <SIcon
        :icon="feedbackIsError ? 'lucide:circle-alert' : 'lucide:circle-check'"
        class="mt-0.5 shrink-0"
      />
      <span>{{ feedback }}</span>
    </div>

    <div v-if="records.length" class="space-y-3">
      <article
        v-for="item in records"
        :key="item.id"
        class="group relative overflow-hidden rounded-2xl border border-border/80 bg-card p-3.5 shadow-sm transition-all sm:p-4 dark:border-border/10"
      >
        <div class="flex items-start gap-3 sm:gap-4">
          <div
            class="flex size-11 shrink-0 items-center justify-center rounded-2xl"
            :class="
              item.status === 'completed'
                ? 'bg-success/10 text-success'
                : 'bg-destructive/10 text-destructive'
            "
          >
            <SIcon
              :icon="item.direction === 'receive' ? 'lucide:download' : 'lucide:upload'"
              class="text-lg"
            />
          </div>

          <div class="min-w-0 flex-1">
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0">
                <div class="truncate text-sm font-semibold leading-5" :title="item.fileName">
                  {{ item.fileName }}
                </div>
                <div
                  class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-muted-foreground"
                >
                  <span>
                    {{ item.direction === 'receive' ? '接收自' : '发送至' }} {{ item.targetIp }}
                  </span>
                  <span class="text-border">·</span>
                  <span>{{ formatBytes(item.total) }}</span>
                  <template v-if="item.createdAt">
                    <span class="text-border">·</span>
                    <span>{{ formatTime(item.createdAt) }}</span>
                  </template>
                </div>
              </div>
              <span
                class="inline-flex shrink-0 items-center gap-1 rounded-full px-2 py-1 text-[10px] font-medium"
                :class="
                  item.status === 'completed'
                    ? 'bg-success/10 text-success'
                    : 'bg-destructive/10 text-destructive'
                "
              >
                <SIcon
                  :icon="item.status === 'completed' ? 'lucide:check' : 'lucide:x'"
                  class="text-[11px]"
                />
                {{ recordStatus(item.status) }}
              </span>
            </div>

            <div class="mt-3 flex items-center gap-2 border-t border-border/60 pt-3">
              <div
                v-if="recordPathLabel(item)"
                class="flex min-w-0 flex-1 items-center gap-1.5 text-[11px] text-muted-foreground"
                :title="
                  isAndroid && item.direction === 'send'
                    ? '源文件来自手机文件选择器'
                    : item.filePath
                "
              >
                <SIcon icon="lucide:folder" class="shrink-0 text-xs" />
                <span class="truncate">{{ recordPathLabel(item) }}</span>
              </div>
              <span v-else class="min-w-0 flex-1 text-[11px] text-muted-foreground">
                暂无本地文件路径
              </span>

              <div class="flex shrink-0 items-center gap-1.5">
                <SButton
                  v-if="canOpenRecord(item)"
                  variant="soft"
                  color="primary"
                  size="sm"
                  class="h-8 px-2.5 text-[11px]"
                  :disabled="openingRecordId === item.id"
                  :title="isAndroid ? '交给 Android 文件应用打开' : '打开文件所在目录'"
                  @click="openRecordLocation(item)"
                >
                  <SIcon
                    :icon="
                      openingRecordId === item.id ? 'lucide:loader-circle' : 'lucide:folder-open'
                    "
                    :class="openingRecordId === item.id ? 'animate-spin' : ''"
                  />
                  {{ isAndroid ? '打开文件' : '打开目录' }}
                </SButton>
                <span
                  v-else-if="isAndroid && item.direction === 'receive' && item.filePath"
                  class="text-[11px] text-muted-foreground"
                  title="旧记录缺少 Android 文件 URI"
                >
                  已保存
                </span>
                <SButton
                  variant="ghost"
                  color="destructive"
                  size="sm"
                  shape="square"
                  class="size-8"
                  aria-label="删除传输记录"
                  title="删除记录"
                  @click="removeRecord(item)"
                >
                  <SIcon icon="lucide:trash-2" />
                </SButton>
              </div>
            </div>

            <div
              v-if="item.error"
              class="mt-2 flex items-center gap-1 text-[11px] text-destructive"
            >
              <SIcon icon="lucide:circle-alert" class="shrink-0 text-xs" />
              <span class="truncate">{{ item.error }}</span>
            </div>
          </div>
        </div>
      </article>
    </div>

    <div
      v-else
      class="rounded-2xl border border-dashed border-border/80 bg-card/60 p-10 text-center dark:border-border/10"
    >
      <div
        class="mx-auto flex size-14 items-center justify-center rounded-2xl bg-muted text-muted-foreground"
      >
        <SIcon icon="lucide:history" class="text-2xl" />
      </div>
      <div class="mt-4 text-sm font-semibold">暂无传输记录</div>
      <div class="mt-1 text-xs text-muted-foreground">完成文件收发后，记录会显示在这里</div>
    </div>
  </div>
</template>
