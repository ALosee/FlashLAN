<script setup lang="ts">
import { computed } from 'vue'
import { SCard } from '@/ui/components/card'
import { SIcon } from '@/ui/components/icon'
import { SBadge } from '@/ui/components/badge'
import { useTransferStore } from '@/stores/transfer'

const transferStore = useTransferStore()
const records = computed(() =>
  transferStore.tasks.filter(task => task.status === 'completed' || task.status === 'failed'),
)

function formatBytes(bytes: number) {
  if (!bytes) return '未知大小'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`
}
</script>

<template>
  <div class="p-4 md:p-6 max-w-5xl mx-auto w-full space-y-4">
    <div>
      <h1 class="text-xl font-bold tracking-tight">传输记录</h1>
      <p class="text-sm text-muted-foreground mt-1">查看历史收发文件</p>
    </div>
    <SCard v-if="records.length">
      <div class="divide-y -m-6">
        <div
          v-for="item in records"
          :key="item.id"
          class="flex items-center gap-4 p-4 hover:bg-muted/30 transition-colors"
        >
          <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
            <SIcon :icon="item.direction === 'receive' ? 'lucide:download' : 'lucide:upload'" />
          </div>
          <div class="flex-1 min-w-0">
            <div class="text-sm font-medium truncate flex items-center gap-2">
              {{ item.fileName }}
              <span class="text-xs text-muted-foreground font-normal">
                · {{ formatBytes(item.total) }}
              </span>
            </div>
            <div class="text-xs text-muted-foreground flex items-center gap-1.5 mt-1 truncate">
              <SIcon icon="lucide:clock-3" class="text-xs" />
              {{ item.direction === 'receive' ? '接收自' : '发送至' }} {{ item.targetIp }}
              <span v-if="item.direction === 'receive'">
                · 已保存至 {{ item.filePath || 'Download/FlashLAN' }}
              </span>
            </div>
          </div>
          <SBadge
            :color="item.status === 'completed' ? 'success' : 'destructive'"
            size="sm"
            class="shrink-0"
          >
            <SIcon
              :icon="item.status === 'completed' ? 'lucide:check' : 'lucide:x'"
              class="text-xs"
            />
            {{ item.status === 'completed' ? '已完成' : '失败' }}
          </SBadge>
        </div>
      </div>
    </SCard>
    <SCard v-else>
      <div class="p-8 text-center text-sm text-muted-foreground">暂无已完成的传输记录</div>
    </SCard>
  </div>
</template>
