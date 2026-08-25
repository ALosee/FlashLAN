<script setup lang="ts">
import { ref } from 'vue'
import { SButton } from '@/ui/components/button'
import { SCard } from '@/ui/components/card'
import { SIcon } from '@/ui/components/icon'
import { SBadge } from '@/ui/components/badge'
import { SSeparator } from '@/ui/components/separator'

const isDragging = ref(false)

function onDragOver(e: DragEvent) {
  e.preventDefault()
  isDragging.value = true
}
function onDragLeave() {
  isDragging.value = false
}
function onDrop(e: DragEvent) {
  e.preventDefault()
  isDragging.value = false
  // TODO: handle file drop -> invoke Tauri
}

const devices = [
  { name: 'MacBook Air', ip: '192.168.1.102', icon: 'lucide:laptop', online: true },
  { name: 'Windows PC', ip: '192.168.1.105', icon: 'lucide:monitor', online: true },
  { name: 'iPhone 15', icon: 'lucide:smartphone', ip: '192.168.1.201', online: true },
]
</script>

<template>
  <div class="p-6 space-y-6 max-w-5xl mx-auto w-full">
    <!-- Header -->
    <div class="flex items-center justify-between gap-4">
      <div>
        <h1 class="text-xl font-bold tracking-tight">快速传文件</h1>
        <p class="text-sm text-muted-foreground mt-1">拖拽文件到此处，或选择设备直接发送</p>
      </div>
      <div class="flex items-center gap-3 shrink-0">
        <span class="text-xs text-muted-foreground">本机可被发现</span>
        <div class="size-2 rounded-full bg-success" />
        <span class="text-xs font-medium text-success">在线</span>
      </div>
    </div>

    <!-- Drop zone -->
    <SCard class="border-dashed! border-2!">
      <div
        class="p-10 text-center transition-colors cursor-pointer rounded-xl"
        :class="isDragging ? 'bg-primary/5 border-primary' : 'bg-card hover:bg-muted/30'"
        @dragover="onDragOver"
        @dragleave="onDragLeave"
        @drop="onDrop"
      >
        <div class="size-14 rounded-2xl bg-primary/10 flex items-center justify-center mx-auto">
          <SIcon icon="lucide:folder-open" class="text-2xl text-primary" />
        </div>
        <div class="mt-4 font-medium">拖拽文件或文件夹到此处</div>
        <div class="text-sm text-muted-foreground mt-1">支持多文件、文件夹，单次可传任意大小</div>
        <div class="mt-5 flex items-center justify-center gap-3">
          <SButton>
            <SIcon icon="lucide:file-up" />
            选择文件
          </SButton>
          <SButton variant="outline">
            <SIcon icon="lucide:folder" />
            选择文件夹
          </SButton>
        </div>
        <div class="text-xs text-muted-foreground mt-3 flex items-center justify-center gap-1.5">
          <SIcon icon="lucide:clipboard" class="text-xs" />
          或直接粘贴剪贴板内容 Ctrl+V
        </div>
      </div>
    </SCard>

    <!-- Devices grid -->
    <div>
      <div class="flex items-center justify-between">
        <h2 class="font-semibold flex items-center gap-2">
          附近设备
          <SBadge color="secondary" size="sm">{{ devices.length }}</SBadge>
        </h2>
        <SButton variant="ghost" size="sm">
          <SIcon icon="lucide:refresh-cw" />
          刷新
        </SButton>
      </div>

      <div class="grid grid-cols-3 gap-3 mt-3">
        <SCard
          v-for="device in devices"
          :key="device.ip"
          class="hover:border-primary/30 hover:shadow-sm cursor-pointer transition-all group p-4!"
        >
          <div class="flex items-start justify-between">
            <div class="size-10 rounded-xl bg-muted flex items-center justify-center">
              <SIcon :icon="device.icon" class="text-lg" />
            </div>
            <div class="size-2 rounded-full bg-success mt-1 shrink-0" />
          </div>
          <div class="mt-3 font-medium text-sm truncate">{{ device.name }}</div>
          <div class="text-xs text-muted-foreground font-mono truncate">{{ device.ip }}</div>
          <div class="mt-3">
            <SButton size="sm" class="w-full">
              <SIcon icon="lucide:send" />
              发送
            </SButton>
          </div>
        </SCard>

        <!-- Add manual IP -->
        <SCard
          class="border-dashed! bg-muted/20 hover:bg-card hover:border-border transition-colors min-h-[168px] flex flex-col items-center justify-center text-center cursor-pointer p-4!"
        >
          <div
            class="size-8 rounded-full border flex items-center justify-center text-muted-foreground"
          >
            <SIcon icon="lucide:plus" />
          </div>
          <div class="text-sm font-medium mt-2">手动添加设备</div>
          <div class="text-xs text-muted-foreground">输入 IP 地址</div>
        </SCard>
      </div>
    </div>

    <SSeparator />

    <!-- Recent transfers -->
    <SCard>
      <template #header>
        <div class="flex items-center justify-between w-full">
          <span class="font-medium text-sm flex items-center gap-2">
            <SIcon icon="lucide:arrow-left-right" class="text-muted-foreground" />
            正在传输
          </span>
          <SBadge color="secondary" size="sm">1 项进行中</SBadge>
        </div>
      </template>
      <div class="flex items-center gap-4">
        <div class="size-10 rounded-lg bg-orange-500/10 flex items-center justify-center shrink-0">
          <SIcon icon="lucide:file-text" class="text-orange-500" />
        </div>
        <div class="flex-1 min-w-0">
          <div class="text-sm font-medium truncate">design-spec-2026.pdf · 24.5 MB</div>
          <div class="text-xs text-muted-foreground flex items-center gap-1.5 mt-1">
            <SIcon icon="lucide:send" class="text-xs" />
            发送至 Windows PC · 12.4 MB/s
          </div>
          <div class="mt-2 h-1.5 rounded-full bg-muted overflow-hidden">
            <div class="h-full bg-primary rounded-full transition-all" style="width: 62%"></div>
          </div>
        </div>
        <div class="text-right shrink-0 flex flex-col items-end gap-1">
          <div class="text-sm font-medium">62%</div>
          <SButton
            variant="ghost"
            size="xs"
            class="h-6 px-2 text-xs text-muted-foreground hover:text-destructive"
          >
            <SIcon icon="lucide:x" class="text-xs" />
            取消
          </SButton>
        </div>
      </div>
    </SCard>
  </div>
</template>
