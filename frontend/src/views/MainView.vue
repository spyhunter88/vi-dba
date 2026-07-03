<script setup lang="ts">
import Sidebar from '../components/layout/Sidebar.vue';
import TabBar from '../components/layout/Tabs.vue';
import StatusBar from '../components/layout/StatusBar.vue';
import Workspace from '../components/layout/Workspace.vue';
import Toolbar from '../components/layout/Toolbar.vue';
import ConnectionModal from '../components/ui/ConnectionModal.vue';
import { useConnectionStore } from '../stores/connections';
import { onMounted } from 'vue';

const connectionStore = useConnectionStore();

onMounted(() => {
  connectionStore.loadConnections();
});
</script>

<template>
  <div class="main-layout">
    <Sidebar />
    <div class="content-area">
      <Toolbar />
      <TabBar />
      <Workspace @add-connection="connectionStore.openNewConnectionModal($event)" />
      <StatusBar />
    </div>
    <ConnectionModal 
      :show="connectionStore.showConnectionModal" 
      :initialType="connectionStore.connectionModalType"
      @close="connectionStore.showConnectionModal = false"
    />
  </div>
</template>

<style scoped>
.main-layout {
  display: flex;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background-color: var(--bg-primary);
}

.content-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
}
</style>
