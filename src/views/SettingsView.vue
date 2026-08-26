<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { SCard } from '@/ui/components/card'
import { SButton } from '@/ui/components/button'
import { SInput } from '@/ui/components/input'
import { SSwitch } from '@/ui/components/switch'
import { SIcon } from '@/ui/components/icon'
import { SSeparator } from '@/ui/components/separator'
import { useDeviceStore } from '@/stores/device'
import { useTransferStore } from '@/stores/transfer'

const deviceStore = useDeviceStore()
const transferStore = useTransferStore()
const savePath = ref('Download/FlashLAN')
const deviceName = ref('')
const autoReceive = computed<boolean>({
  get: () => transferStore.autoReceiveEnabled,
  set: value => {
    void transferStore.setAutoReceive(value).catch(error => {
      console.error('[FlashLAN] update auto receive failed', error)
    })
  },
})

onMounted(async () => {
  await deviceStore.fetchLocal()
  if (deviceStore.localDevice) {
    deviceName.value = deviceStore.localDevice.name
  }
})
</script>

<template>
  <div class="p-4 md:p-6 max-w-3xl mx-auto w-full space-y-4 md:space-y-6">
    <div>
      <h1 class="text-xl font-bold tracking-tight">设置</h1>
      <p class="text-sm text-muted-foreground mt-1">管理设备与传输偏好</p>
    </div>

    <SCard>
      <div class="divide-y">
        <div
          class="py-3 md:py-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3 first:pt-0 last:pb-0"
        >
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

        <div class="py-3 md:py-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div class="flex items-center gap-3">
            <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
              <SIcon icon="lucide:folder" />
            </div>
            <div>
              <div class="text-sm font-medium">保存路径</div>
              <div class="text-xs text-muted-foreground truncate max-w-[280px]">
                {{ savePath }}
              </div>
            </div>
          </div>
          <SButton variant="outline" size="sm">
            <SIcon icon="lucide:folder-open" />
            选择
          </SButton>
        </div>

        <div class="py-3 md:py-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div class="flex items-center gap-3">
            <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
              <SIcon icon="lucide:shield-check" />
            </div>
            <div>
              <div class="text-sm font-medium">可信设备自动接收</div>
              <div class="text-xs text-muted-foreground">开启后可信设备发送文件将自动接收</div>
            </div>
          </div>
          <SSwitch v-model="autoReceive" />
        </div>

        <div class="py-3 md:py-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div class="flex items-center gap-3">
            <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
              <SIcon icon="lucide:plug" />
            </div>
            <div>
              <div class="text-sm font-medium">端口</div>
              <div class="text-xs text-muted-foreground">发现 mDNS / 传输 17321</div>
            </div>
          </div>
          <span class="text-xs font-mono bg-muted px-2.5 py-1 rounded-md">17321</span>
        </div>

        <div class="py-3 md:py-4 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div class="flex items-center gap-3">
            <div class="size-9 rounded-lg bg-muted flex items-center justify-center shrink-0">
              <SIcon icon="lucide:info" />
            </div>
            <div>
              <div class="text-sm font-medium">本机信息</div>
              <div class="text-xs text-muted-foreground font-mono">
                {{ deviceStore.localDevice?.id || '-' }}
              </div>
            </div>
          </div>
          <span class="text-xs bg-muted px-2 py-1 rounded">
            {{ deviceStore.localDevice?.platform || '-' }}
          </span>
        </div>
      </div>
    </SCard>

    <SSeparator />

    <div class="text-xs text-muted-foreground flex items-center gap-2">
      <SIcon icon="lucide:info" class="text-xs" />
      FlashLAN v0.1.0 · Tauri 2 · Vue 3 · SoybeanUI · UnoCSS · mDNS
    </div>
  </div>
</template>
