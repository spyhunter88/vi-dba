<script setup lang="ts">
import { onMounted } from 'vue';
import { useUiStore } from './stores/ui';
import { useConnectionStore } from './stores/connections';
import { useSessionStore } from './stores/session';
import { getCurrentWindow } from '@tauri-apps/api/window';
import ConfirmDialog from './components/ui/ConfirmDialog.vue';
import ToastContainer from './components/ui/ToastContainer.vue';

const uiStore = useUiStore();
const connectionStore = useConnectionStore();
const sessionStore = useSessionStore();

onMounted(async () => {
  const window = getCurrentWindow();
  console.log('[App] Root mounted on window:', window.label);
  
  await uiStore.initTheme();
  
  // Only load connections and restore session in the main window
  if (window.label === 'main') {
    await connectionStore.loadConnections();
    await sessionStore.restoreSession();
  }
});
</script>

<template>
  <router-view />
  <ConfirmDialog 
    :show="uiStore.confirmState.show" 
    :title="uiStore.confirmState.title"
    :message="uiStore.confirmState.message"
    :type="uiStore.confirmState.type"
    @confirm="() => { uiStore.confirmState.onConfirm?.(); uiStore.confirmState.show = false; }"
    @cancel="uiStore.confirmState.show = false"
  />
  <ToastContainer />
</template>

<style>
/* Global styles if needed */
</style>
