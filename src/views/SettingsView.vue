<script setup lang="ts">
import { ref } from 'vue'
import { SCard } from '@/ui/components/card'
import { SButton } from '@/ui/components/button'
import { SInput } from '@/ui/components/input'
import { SSwitch } from '@/ui/components/switch'
import { SIcon } from '@/ui/components/icon'
import { SSeparator } from '@/ui/components/separator'

const savePath = ref('/Users/jack/Downloads/FlashLAN')
const deviceName = ref('MacBook Pro')
const autoReceive = ref(false)
</script>

<template>
  <div class="p-6 max-w-3xl mx-auto w-full space-y-6">
    <div>
      <h1 class="text-xl font-bold tracking-tight">设置</h1>
      <p class="text-sm text-muted-foreground mt-1">管理设备与传输偏好</p>
    </div>

    <SCard>
      <div class="divide-y">
        <div class="py-4 flex items-center justify-between gap-4 first:pt-0 last:pb-0">
          <div class="flex items-center gap-3">
            <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
              <SIcon icon="lucide:monitor" />
            </div>
            <div>
              <div class="text-sm font-medium">设备名称</div>
              <div class="text-xs text-muted-foreground">局域网内显示的名称</div>
            </div>
          </div>
          <SInput v-model="deviceName" class="w-56" placeholder="输入设备名称" />
        </div>

        <div class="py-4 flex items-center justify-between gap-4">
          <div class="flex items-center gap-3">
            <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
              <SIcon icon="lucide:folder" />
            </div>
            <div>
              <div class="text-sm font-medium">保存路径</div>
              <div class="text-xs text-muted-foreground font-mono truncate max-w-[260px]">
                {{ savePath }}
              </div>
            </div>
          </div>
          <SButton variant="outline" size="sm">
            <SIcon icon="lucide:folder-open" />
            选择
          </SButton>
        </div>

        <div class="py-4 flex items-center justify-between gap-4">
          <div class="flex items-center gap-3">
            <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
              <SIcon icon="lucide:shield-check" />
            </div>
            <div>
              <div class="text-sm font-medium">可信设备自动接收</div>
              <div class="text-xs text-muted-foreground">开启后无需确认直接保存</div>
            </div>
          </div>
          <SSwitch v-model="autoReceive" />
        </div>

        <div class="py-4 flex items-center justify-between gap-4">
          <div class="flex items-center gap-3">
            <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
              <SIcon icon="lucide:plug" />
            </div>
            <div>
              <div class="text-sm font-medium">端口</div>
              <div class="text-xs text-muted-foreground">发现 5353 / 传输 17321</div>
            </div>
          </div>
          <span class="text-xs font-mono bg-muted px-2.5 py-1 rounded-md">17321</span>
        </div>
      </div>
    </SCard>

    <SSeparator />

    <div class="text-xs text-muted-foreground flex items-center gap-2">
      <SIcon icon="lucide:info" class="text-xs" />
      FlashLAN v0.1.0 · Tauri 2 · Vue 3 · SoybeanUI · UnoCSS
    </div>
  </div>
</template>
