<script setup lang="ts">
import { ref, onMounted, computed, watch } from 'vue';
import { useTabStore } from '../stores/tabs';
import { useQueryStore } from '../stores/query';
import { useSessionStore } from '../stores/session';
import { useConnectionStore } from '../stores/connections';
import { invoke } from '@tauri-apps/api/core';
import { useRoute } from 'vue-router';
import { 
  History, 
  Search, 
  Trash2, 
  Play, 
  Clock, 
  CheckCircle, 
  XCircle,
  FileCode,
  Box
} from 'lucide-vue-next';
import type { QueryHistoryEntry } from '../types';

const tabStore = useTabStore();
const queryStore = useQueryStore();
const sessionStore = useSessionStore();
const connectionStore = useConnectionStore();
const route = useRoute();
const history = ref<QueryHistoryEntry[]>([]);
const searchQuery = ref('');
const loading = ref(true);
const activeTab = ref<'queries' | 'scripts' | 'routines'>('queries');
const routineSnapshots = ref<any[]>([]);
const selectedSnapshotGroup = ref<any | null>(null);
const groupSnapshots = ref<any[]>([]);
const previewContent = ref('');
const loadingPreview = ref(false);
const isWindow = computed(() => route.query.window === 'true');

const filteredHistory = computed(() => {
  if (!searchQuery.value) return history.value;
  const q = searchQuery.value.toLowerCase();
  return history.value.filter(entry => 
    entry.query.toLowerCase().includes(q) || 
    entry.connectionId.toLowerCase().includes(q)
  );
});

const sessionTabs = computed(() => tabStore.tabs);

async function loadHistory() {
  loading.value = true;
  if (activeTab.value === 'queries') {
    const rawHistory = await queryStore.getQueryHistory();
    history.value = rawHistory.sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());
  } else if (activeTab.value === 'routines') {
    routineSnapshots.value = await sessionStore.getAllSnapshotsSummary();
    
    // Pre-select from query params
    const { tabId, connectionId, database, schema } = route.query;
    if (tabId && !selectedSnapshotGroup.value) {
      const group = routineSnapshots.value.find(g => 
        g.tabId === tabId && 
        g.connectionId === connectionId && 
        (g.database || null) === (database || null) && 
        (g.schema || null) === (schema || null)
      );
      if (group) {
        selectSnapshotGroup(group);
      }
    }
  }
  loading.value = false;
}

watch(activeTab, () => {
  loadHistory();
  selectedSnapshotGroup.value = null;
  groupSnapshots.value = [];
  previewContent.value = '';
});

async function selectSnapshotGroup(group: any) {
  selectedSnapshotGroup.value = group;
  loadingPreview.value = true;
  groupSnapshots.value = await sessionStore.getSnapshots(
    group.tabId, 
    group.connectionId, 
    group.database, 
    group.schema
  );
  if (groupSnapshots.value.length > 0) {
    await previewSnapshot(groupSnapshots.value[0]);
  }
  loadingPreview.value = false;
}

async function previewSnapshot(snapshot: any) {
  loadingPreview.value = true;
  try {
    previewContent.value = await invoke<string>('read_snapshot', { path: snapshot.path });
  } catch (e) {
    console.error('Failed to read snapshot:', e);
  } finally {
    loadingPreview.value = false;
  }
}

function restoreFromHistory() {
  if (!selectedSnapshotGroup.value || !previewContent.value) return;
  
  const group = selectedSnapshotGroup.value;
  // Parse tabId to determine type and name
  // Format: routine-{connectionId}-{name} or view-{connectionId}-{name} or query-...
  let type: 'routine_editor' | 'view_editor' | 'sql_query' = 'sql_query';
  let name = group.tabId;
  
  if (group.tabId.startsWith('routine-')) {
    type = 'routine_editor';
    const parts = group.tabId.split('-');
    name = parts[parts.length - 1];
  } else if (group.tabId.startsWith('view-')) {
    type = 'view_editor';
    const parts = group.tabId.split('-');
    name = parts[parts.length - 1];
  }

  if (type === 'routine_editor') {
    tabStore.editRoutine(group.connectionId, name, 'PROCEDURE', group.database, group.schema);
  } else if (type === 'view_editor') {
    tabStore.editView(group.connectionId, name, group.database, group.schema);
  } else {
    tabStore.addTab({
      id: `history-restore-${Date.now()}`,
      title: 'Restored Script',
      type: 'sql_query',
      connectionId: group.connectionId,
      database: group.database,
      schema: group.schema,
      content: previewContent.value,
      isDirty: true
    });
  }
  
  // Go back to workspace
  window.history.back();
}

function formatSnapshotName(tabId: string) {
  if (tabId.startsWith('routine-')) {
    const parts = tabId.split('-');
    return `Routine: ${parts[parts.length - 1]}`;
  }
  if (tabId.startsWith('view-')) {
    const parts = tabId.split('-');
    return `View: ${parts[parts.length - 1]}`;
  }
  if (tabId.startsWith('query-')) {
    return 'Ad-hoc Query';
  }
  return tabId;
}

function formatTimestamp(ts: string) {
  if (ts.length !== 15) return ts;
  const year = ts.substring(0, 4);
  const month = ts.substring(4, 6);
  const day = ts.substring(6, 8);
  const hour = ts.substring(9, 11);
  const minute = ts.substring(11, 13);
  return `${year}-${month}-${day} ${hour}:${minute}`;
}

async function clearHistory() {
  if (confirm('Are you sure you want to clear all query history?')) {
    await queryStore.clearQueryHistory();
    await loadHistory();
  }
}

function formatDate(dateStr: string) {
  const date = new Date(dateStr);
  return date.toLocaleString();
}

function getStatusIcon(status: string) {
  return status === 'success' ? CheckCircle : XCircle;
}

function getStatusClass(status: string) {
  return status === 'success' ? 'text-success' : 'text-error';
}

function rerunQuery(query: string, connectionId: string) {
  tabStore.addTab({
    id: `query-rerun-${Date.now()}`,
    title: 'Rerun Query',
    type: 'sql_query',
    connectionId,
    content: query,
    isDirty: true
  });
}

function openSessionTab(tab: any) {
  tabStore.activeTabId = tab.id;
  window.history.back(); // Simple way to return
}

function getConnectionName(id: string) {
  const conn = connectionStore.connections.find(c => c.id === id);
  return conn ? conn.name : id;
}

onMounted(() => {
  const { tabId } = route.query;
  if (tabId) {
    activeTab.value = 'routines';
  }
  loadHistory();
});
</script>

<template>
  <div class="history-view">
    <div class="history-header glass">
      <div class="flex-between w-full">
        <div class="flex-center gap-4">
          <div class="flex-center gap-2">
            <History :size="18" class="text-accent" />
            <h2 class="text-lg font-bold whitespace-nowrap">History & Sessions</h2>
          </div>
          
          <div class="tabs-nav glass p-1 rounded-lg flex gap-1">
            <button 
              class="tab-btn whitespace-nowrap" 
              :class="{ active: activeTab === 'queries' }"
              @click="activeTab = 'queries'"
            >
              Executed Queries
            </button>
            <button 
              class="tab-btn whitespace-nowrap" 
              :class="{ active: activeTab === 'scripts' }"
              @click="activeTab = 'scripts'"
            >
              Session Scripts
            </button>
            <button 
              class="tab-btn whitespace-nowrap" 
              :class="{ active: activeTab === 'routines' }"
              @click="activeTab = 'routines'"
            >
              Object History
            </button>
          </div>
        </div>

        <div class="flex-center gap-3">
          <div class="search-box glass" v-if="activeTab === 'queries'">
            <Search :size="16" />
            <input v-model="searchQuery" type="text" placeholder="Search history..." />
          </div>
          <button v-if="activeTab === 'queries'" class="button-danger" @click="clearHistory" title="Clear History">
            <Trash2 :size="16" />
          </button>
          <button v-if="!isWindow" class="button-secondary" @click="$router.push('/')">Back to Workspace</button>
        </div>
      </div>
    </div>

    <div class="history-content">
      <!-- Executed Queries Tab -->
      <template v-if="activeTab === 'queries'">
        <div v-if="loading" class="loading-state">
          <div class="spinner"></div>
          <p>Loading history...</p>
        </div>
        <div v-else-if="filteredHistory.length === 0" class="empty-state">
          <Clock :size="48" style="opacity: 0.2; margin-bottom: 12px" />
          <p>No query history found</p>
        </div>
        <div v-else class="history-list">
          <div v-for="entry in filteredHistory" :key="entry.id" class="history-item glass card">
            <div class="entry-header">
              <div class="flex-center gap-2 flex-wrap">
                <component :is="getStatusIcon(entry.status)" :size="16" :class="getStatusClass(entry.status)" />
                <span class="timestamp">{{ formatDate(entry.timestamp) }}</span>
                <span class="connection-pill">{{ getConnectionName(entry.connectionId) }}</span>
                <span class="result-pill" :class="getStatusClass(entry.status)">
                  {{ entry.status === 'success' ? `${entry.affectedRows || 0} rows` : 'Error' }}
                </span>
                <span class="duration">{{ entry.durationMs }}ms</span>
              </div>
              <button class="icon-btn" title="Rerun Query" @click="rerunQuery(entry.query, entry.connectionId)">
                <Play :size="14" />
              </button>
            </div>
            <div class="query-box">
              <pre><code>{{ entry.query }}</code></pre>
            </div>
            <div v-if="entry.errorMessage" class="error-box">
              {{ entry.errorMessage }}
            </div>
          </div>
        </div>
      </template>

      <!-- Session Scripts Tab -->
      <template v-if="activeTab === 'scripts'">
        <div v-if="sessionTabs.length === 0" class="empty-state">
          <FileCode :size="48" style="opacity: 0.2; margin-bottom: 12px" />
          <p>No active session scripts</p>
        </div>
        <div v-else class="scripts-grid">
          <div v-for="tab in sessionTabs" :key="tab.id" class="script-card glass card">
            <div class="flex-between mb-3">
              <div class="flex-center gap-2">
                <Box :size="18" class="text-accent" />
                <span class="font-bold">{{ tab.title }}</span>
              </div>
              <span class="tab-type-pill">{{ tab.type }}</span>
            </div>
            <div class="script-meta mb-3">
              <div class="meta-item">
                <span class="label">Connection:</span>
                <span class="value">{{ tab.connectionId }}</span>
              </div>
              <div v-if="tab.filePath" class="meta-item">
                <span class="label">File:</span>
                <span class="value">{{ tab.filePath }}</span>
              </div>
            </div>
            <div class="script-preview" v-if="tab.content">
              <code>{{ tab.content.substring(0, 100) }}{{ tab.content.length > 100 ? '...' : '' }}</code>
            </div>
            <div class="card-footer mt-4 flex justify-end">
              <button class="button-primary btn-sm" @click="openSessionTab(tab)">Open in Workspace</button>
            </div>
          </div>
        </div>
      </template>

      <!-- Object History Tab -->
      <template v-if="activeTab === 'routines'">
        <div v-if="loading" class="loading-state">
          <div class="spinner"></div>
          <p>Loading object history...</p>
        </div>
        <div v-else-if="routineSnapshots.length === 0" class="empty-state">
          <Clock :size="48" style="opacity: 0.2; margin-bottom: 12px" />
          <p>No object history found. Snapshots are created when you execute or save objects.</p>
        </div>
        <div v-else class="routines-history-layout">
          <div class="history-sidebar glass card">
            <div class="sidebar-header-text">Automatic Snapshots</div>
            <div class="groups-list">
              <div 
                v-for="group in routineSnapshots" 
                :key="group.tabId" 
                class="group-item"
                :class="{ active: selectedSnapshotGroup?.tabId === group.tabId }"
                @click="selectSnapshotGroup(group)"
              >
                <div class="group-name">{{ formatSnapshotName(group.tabId) }}</div>
                <div class="group-meta">
                  <span>{{ group.connectionId }}</span>
                  <span v-if="group.database">• {{ group.database }}</span>
                  <span v-if="group.schema">• {{ group.schema }}</span>
                </div>
                <div class="group-footer">
                  <span>{{ group.snapshot_count }} versions</span>
                  <span>Last: {{ formatTimestamp(group.lastSnapshot) }}</span>
                </div>
              </div>
            </div>
          </div>

          <div class="history-detail glass card">
            <template v-if="selectedSnapshotGroup">
              <div class="detail-header">
                <div class="flex-column">
                  <h3 class="font-bold">{{ formatSnapshotName(selectedSnapshotGroup.tabId) }}</h3>
                  <div class="text-xs opacity-60">
                    {{ selectedSnapshotGroup.connectionId }} 
                    {{ selectedSnapshotGroup.database ? '/ ' + selectedSnapshotGroup.database : '' }}
                    {{ selectedSnapshotGroup.schema ? '/ ' + selectedSnapshotGroup.schema : '' }}
                  </div>
                </div>
                <button class="button-primary sm" @click="restoreFromHistory">Restore this version</button>
              </div>

              <div class="detail-content">
                <div class="versions-column">
                  <div 
                    v-for="ss in groupSnapshots" 
                    :key="ss.timestamp"
                    class="version-item"
                    @click="previewSnapshot(ss)"
                  >
                    <Clock :size="14" />
                    <span>{{ formatTimestamp(ss.timestamp) }}</span>
                  </div>
                </div>
                <div class="preview-column">
                  <div v-if="loadingPreview" class="loading-state h-full">
                    <div class="spinner-xs"></div>
                  </div>
                  <pre v-else><code>{{ previewContent }}</code></pre>
                </div>
              </div>
            </template>
            <div v-else class="empty-detail flex-center flex-column opacity-30">
              <Clock :size="64" class="mb-4" />
              <p>Select an object to view its history</p>
            </div>
          </div>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.history-view {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--bg-primary);
  color: var(--text-primary);
}

.history-header {
  padding: 16px 24px;
  border-bottom: 1px solid var(--border-color);
  z-index: 10;
}

.history-content {
  flex: 1;
  overflow-y: auto;
  padding: 24px;
}

.tab-btn {
  padding: 6px 16px;
  border-radius: 6px;
  font-size: 0.9rem;
  background: transparent;
  border: none;
  cursor: pointer;
  color: var(--text-secondary);
  transition: all 0.2s;
}

.tab-btn.active {
  background: var(--accent-primary);
  color: white;
}

.search-box {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border-radius: 8px;
  width: 300px;
}

.search-box input {
  background: transparent;
  border: none;
  color: var(--text-primary);
  width: 100%;
  outline: none;
}

.history-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
  max-width: 1200px;
  margin: 0 auto;
}

.scripts-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(350px, 1fr));
  gap: 20px;
  max-width: 1400px;
  margin: 0 auto;
}

.history-item, .script-card {
  padding: 16px;
  border-radius: 12px;
  transition: transform 0.2s;
}

.history-item:hover, .script-card:hover {
  transform: translateY(-2px);
}

.entry-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.timestamp {
  font-size: 0.85rem;
  color: var(--text-secondary);
}

.connection-pill, .tab-type-pill, .result-pill {
  font-size: 0.75rem;
  padding: 2px 8px;
  background: var(--glass-border);
  border-radius: 12px;
  opacity: 0.8;
}

.result-pill.text-success {
  background: rgba(16, 185, 129, 0.1);
}

.result-pill.text-error {
  background: rgba(248, 113, 113, 0.1);
}

.duration {
  font-size: 0.75rem;
  opacity: 0.6;
}

.query-box, .script-preview {
  background: rgba(0, 0, 0, 0.2);
  padding: 12px;
  border-radius: 8px;
  font-family: 'Fira Code', monospace;
  font-size: 0.9rem;
  max-height: 200px;
  overflow-y: auto;
}

.script-preview {
  max-height: 80px;
  font-size: 0.8rem;
  opacity: 0.7;
}

.script-meta {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.meta-item {
  display: flex;
  gap: 8px;
  font-size: 0.8rem;
}

.meta-item .label {
  color: var(--text-secondary);
  width: 80px;
}

.error-box {
  margin-top: 12px;
  padding: 12px;
  background: rgba(239, 68, 68, 0.1);
  border-left: 3px solid var(--accent-error);
  color: var(--accent-error);
  font-size: 0.85rem;
}

.loading-state, .empty-state {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  opacity: 0.6;
}

.text-accent { color: var(--accent-primary); }
.text-success { color: #10b981; }
.text-error { color: #f87171; }

.card {
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
}

.btn-sm {
  padding: 4px 12px;
  font-size: 0.8rem;
}

.justify-end { justify-content: flex-end; }

.routines-history-layout {
  display: grid;
  grid-template-columns: 350px 1fr;
  gap: 20px;
  height: calc(100vh - 120px);
}

.history-sidebar {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.sidebar-header-text {
  padding: 12px 16px;
  font-size: 0.75rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  opacity: 0.5;
  border-bottom: 1px solid var(--border-color);
}

.groups-list {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
}

.group-item {
  padding: 12px;
  border-radius: 8px;
  margin-bottom: 4px;
  cursor: pointer;
  transition: all 0.2s;
  border: 1px solid transparent;
}

.group-item:hover {
  background: var(--glass-border);
}

.group-item.active {
  background: rgba(var(--accent-primary-rgb, 59, 130, 246), 0.1);
  border: 1px solid var(--accent-primary);
}

.group-name {
  font-weight: 600;
  font-size: 0.9rem;
  margin-bottom: 4px;
}

.group-meta {
  font-size: 0.75rem;
  opacity: 0.6;
  margin-bottom: 8px;
}

.group-footer {
  display: flex;
  justify-content: space-between;
  font-size: 0.7rem;
  opacity: 0.4;
}

.history-detail {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.detail-header {
  padding: 16px 24px;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.detail-content {
  flex: 1;
  display: grid;
  grid-template-columns: 200px 1fr;
  overflow: hidden;
}

.versions-column {
  border-right: 1px solid var(--border-color);
  overflow-y: auto;
  padding: 8px;
}

.version-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: 6px;
  font-size: 0.8rem;
  cursor: pointer;
  transition: all 0.2s;
}

.version-item:hover {
  background: var(--glass-border);
}

.preview-column {
  overflow: auto;
  padding: 16px;
  background: rgba(0, 0, 0, 0.2);
}

.preview-column pre {
  margin: 0;
  font-family: inherit;
  font-size: 0.85rem;
}

.empty-detail {
  height: 100%;
}
</style>
