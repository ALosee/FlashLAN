<script setup lang="ts">
defineOptions({
  name: 'SSkeleton',
  inheritAttrs: false,
})
</script>

<template>
  <span v-bind="$attrs" class="block animate-pulse rounded-md bg-muted" aria-hidden="true" />
</template>
