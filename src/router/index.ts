import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      component: () => import('@/layouts/MainLayout.vue'),
      children: [
        { path: '', name: 'Home', component: () => import('@/views/HomeView.vue') },
        { path: 'messages', name: 'Messages', component: () => import('@/views/MessagesView.vue') },
        { path: 'devices', name: 'Devices', component: () => import('@/views/DevicesView.vue') },
        { path: 'history', name: 'History', component: () => import('@/views/HistoryView.vue') },
        { path: 'settings', name: 'Settings', component: () => import('@/views/SettingsView.vue') },
      ],
    },
  ],
})

export default router
