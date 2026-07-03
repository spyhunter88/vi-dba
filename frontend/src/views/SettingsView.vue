<script setup lang="ts">
import { onMounted, ref } from 'vue';
import GeneralSettings from '../components/settings/GeneralSettings.vue';
import DisplaySettings from '../components/settings/DisplaySettings.vue';
import SessionHistorySettings from '../components/settings/SessionHistorySettings.vue';
import AiSettings from '../components/settings/AiSettings.vue';
import { Settings, Monitor, Cog, ArrowLeft, History, Sparkles } from 'lucide-vue-next';

const activeTab = ref('general');

onMounted(() => {
  // Initialization now handled by App.vue
});

const tabs = [
  { id: 'general', name: 'General', icon: Cog },
  { id: 'display', name: 'Display', icon: Monitor },
  { id: 'session', name: 'Session & History', icon: History },
  { id: 'ai', name: 'AI Assistant', icon: Sparkles },
];

</script>

<template>
  <div class="settings-view glass">
    <div class="settings-sidebar">
      <div class="sidebar-header">
        <Settings :size="20" class="text-accent" />
        <span class="font-bold">Settings</span>
      </div>
      <div class="sidebar-nav">
        <button 
          v-for="tab in tabs" 
          :key="tab.id"
          class="nav-item"
          :class="{ active: activeTab === tab.id }"
          @click="activeTab = tab.id"
        >
          <component :is="tab.icon" :size="18" />
          <span>{{ tab.name }}</span>
        </button>
      </div>
      <div class="sidebar-footer">
        <button class="icon-btn" @click="$router.push('/')" v-if="!$route.query.window">
           <ArrowLeft :size="16" />
           <span>Back to App</span>
        </button>
      </div>
    </div>
    
    <main class="settings-main">
      <GeneralSettings v-if="activeTab === 'general'" />
      <DisplaySettings v-if="activeTab === 'display'" />
      <SessionHistorySettings v-if="activeTab === 'session'" />
      <AiSettings v-if="activeTab === 'ai'" />
    </main>
  </div>
</template>

<style scoped>
.settings-view {
  display: flex;
  height: 100vh;
  width: 100vw;
  background: var(--bg-primary);
  color: var(--text-primary);
  overflow: hidden;
}

.settings-sidebar {
  width: 240px;
  border-right: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  background: var(--bg-secondary);
}

.sidebar-header {
  padding: 24px;
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 1.1rem;
  border-bottom: 1px solid var(--border-color);
}

.sidebar-nav {
  flex: 1;
  padding: 16px 8px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 16px;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
  text-align: left;
}

.nav-item:hover {
  background: var(--glass-border);
  color: var(--text-primary);
}

.nav-item.active {
  background: rgba(59, 130, 246, 0.1);
  color: var(--accent-primary);
}

.settings-main {
  flex: 1;
  overflow-y: auto;
}

.sidebar-footer {
  padding: 16px;
  border-top: 1px solid var(--border-color);
}

.icon-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 8px;
  border-radius: 4px;
  width: 100%;
}

.icon-btn:hover {
  background: var(--glass-border);
  color: var(--text-primary);
}

.font-bold { font-weight: 700; }
.text-accent { color: var(--accent-primary); }
</style>
