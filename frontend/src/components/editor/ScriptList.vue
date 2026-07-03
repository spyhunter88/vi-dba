<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { useTabStore } from '../../stores/tabs';
import { useQueryStore } from '../../stores/query';
import { FileCode, Clock, Calendar, Search } from 'lucide-vue-next';
import type { ScriptInfo } from '../../types';

const props = defineProps<{
  tabId: string;
}>();

const tabStore = useTabStore();
const queryStore = useQueryStore();

const scripts = ref<ScriptInfo[]>([]);
const loading = ref(true);
const searchQuery = ref('');

const tab = computed(() => tabStore.tabs.find((t: any) => t.id === props.tabId));

async function loadScripts() {
  if (!tab.value?.connectionId) return;
  loading.value = true;
  try {
    scripts.value = await queryStore.loadScripts(tab.value.connectionId);
  } catch (e) {
    console.error('Failed to load scripts:', e);
  } finally {
    loading.value = false;
  }
}

function formatDate(dateStr: string) {
  const date = new Date(dateStr);
  return date.toLocaleString();
}

async function openScript(script: ScriptInfo) {
  if (!tab.value?.connectionId) return;
  
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const content = await invoke<string>('read_script', { 
      connectionId: tab.value.connectionId, 
      name: script.name 
    });

    tabStore.addTab({
      id: `query-file-${script.path}`,
      title: script.name,
      type: 'sql_query',
      connectionId: tab.value.connectionId,
      database: script.database || tab.value.database,
      schema: script.schema || tab.value.schema,
      content: content,
      filePath: script.path,
      isDirty: false
    });
  } catch (e) {
    console.error('Failed to read script:', e);
    alert('Failed to read script: ' + e);
  }
}

onMounted(loadScripts);
</script>

<template>
  <div class="script-list-container">
    <div class="list-header flex-between glass">
      <div class="header-left flex-center gap-4">
        <h2>Saved Scripts</h2>
        <div class="search-box">
          <Search :size="14" class="search-icon" />
          <input type="text" v-model="searchQuery" placeholder="Search scripts..." />
        </div>
      </div>
      <button class="button-secondary sm" @click="loadScripts">Refresh</button>
    </div>

    <div class="list-content">
      <div v-if="loading" class="flex-center p-8">
        <div class="spinner"></div>
      </div>
      
      <table v-else class="script-table">
        <thead>
          <tr>
            <th>Name</th>
            <th>Created At</th>
            <th>Modified At</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="script in scripts" :key="script.path" @dblclick="openScript(script)">
            <td>
              <div class="flex-center gap-2 justify-start">
                <FileCode :size="16" class="text-accent" />
                <span>{{ script.name }}</span>
              </div>
            </td>
            <td>
              <div class="flex-center gap-2 justify-start text-secondary text-sm">
                <Calendar :size="14" />
                {{ formatDate(script.createdAt) }}
              </div>
            </td>
            <td>
              <div class="flex-center gap-2 justify-start text-secondary text-sm">
                <Clock :size="14" />
                {{ formatDate(script.modifiedAt) }}
              </div>
            </td>
          </tr>
          <tr v-if="scripts.length === 0">
            <td colspan="3" class="text-center p-8 text-secondary">
              No saved scripts found for this connection.
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.script-list-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg-tertiary);
}

.list-header {
  height: 56px;
  padding: 0 24px;
  border-bottom: 1px solid var(--border-color);
  background: var(--bg-secondary);
}

.list-header h2 {
  font-size: 1.1rem;
  font-weight: 600;
  margin: 0;
}

.search-box {
  position: relative;
  display: flex;
  align-items: center;
}

.search-icon {
  position: absolute;
  left: 10px;
  color: var(--text-secondary);
}

.search-box input {
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 6px 12px 6px 32px;
  color: var(--text-primary);
  font-size: 0.85rem;
  width: 240px;
  outline: none;
}

.search-box input:focus {
  border-color: var(--accent-primary);
}

.list-content {
  flex: 1;
  overflow: auto;
  padding: 16px 24px;
}

.script-table {
  width: 100%;
  border-collapse: collapse;
  text-align: left;
}

.script-table th {
  padding: 12px;
  border-bottom: 2px solid var(--border-color);
  font-size: 0.85rem;
  color: var(--text-secondary);
  font-weight: 600;
}

.script-table td {
  padding: 12px;
  border-bottom: 1px solid var(--border-color);
  font-size: 0.9rem;
}

.script-table tr:hover {
  background: var(--glass-bg);
  cursor: pointer;
}

.justify-start {
  justify-content: flex-start;
}

.text-sm {
  font-size: 0.8rem;
}

.spinner {
  width: 24px;
  height: 24px;
  border: 2px solid var(--border-color);
  border-top-color: var(--accent-primary);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}
</style>
