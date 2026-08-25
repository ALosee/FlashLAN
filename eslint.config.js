import { defineConfig } from '@soybeanjs/eslint-config-vue'

export default defineConfig({
  'vue/component-name-in-template-casing': [
    'warn',
    'PascalCase',
    {
      registeredComponentsOnly: false,
      ignores: ['/^icon-/'],
    },
  ],
  // 风格：无分号、始终多行尾逗号
  semi: ['error', 'never'],
  'comma-dangle': ['error', 'always-multiline'],
  '@typescript-eslint/semi': ['error', 'never'],
})
