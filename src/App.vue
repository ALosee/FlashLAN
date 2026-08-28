<script setup lang="ts">
import { SConfigProvider } from '@soybeanjs/ui'
import { flashlanTheme } from '@/theme'
</script>

<template>
  <SConfigProvider :theme="flashlanTheme" persist-theme>
    <RouterView />
  </SConfigProvider>
</template>
