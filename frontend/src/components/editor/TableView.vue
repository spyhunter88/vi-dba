<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import {
  Database, RefreshCw, Plus, Sparkles, FileText, Unplug, Edit, ChevronDown, Code
} from 'lucide-vue-next';
import { useTabStore } from '../../stores/tabs';
import { useConnectionStore } from '../../stores/connections';
import { useQueryStore } from '../../stores/query';
import DataGrid from './DataGrid.vue';

const props = defineProps<{
  tabId: string;
}>();

const tabStore = useTabStore();
const connectionStore = useConnectionStore();
const queryStore = useQueryStore();

const tab = computed(() => tabStore.tabs.find(t => t.id === props.tabId));

const dataGridRef = ref<any>(null);
const toolbarRef = ref<HTMLElement | null>(null);
const toolbarWidth = ref(9999);
const showQueryDropdown = ref(false);
const dropdownBtnRef = ref<HTMLElement | null>(null);
const dropdownPos = ref({ top: 0, right: 0 });

// >= 680: text labels + 4 separate query buttons
// 480–679: query buttons → dropdown, text labels kept
// < 480: dropdown + icons only
const showQueryAsDropdown = computed(() => toolbarWidth.value < 680);
const iconsOnly = computed(() => toolbarWidth.value < 480);

const connectionName = computed(() => {
  if (!tab.value?.connectionId) return 'No Connection';
  const conn = connectionStore.connections.find(c => c.id === tab.value?.connectionId);
  return conn?.name || 'Unknown';
});

const isConnected = computed(() =>
  tab.value?.connectionId ? connectionStore.connectedIds.has(tab.value.connectionId) : false
);

function handleReload() {
  queryStore.executeQueryInTab(props.tabId);
}

function handleAddRow() {
  if (dataGridRef.value) dataGridRef.value.addNewRow();
}

function handleEditTable() {
  const currentTab = tab.value;
  if (currentTab?.connectionId) {
    tabStore.editTable(currentTab.connectionId, currentTab.title, currentTab.database, currentTab.schema);
  }
}

function handleGenerateQuery(type: 'SELECT' | 'INSERT' | 'UPDATE' | 'DELETE') {
  const currentTab = tab.value;
  if (currentTab?.connectionId && currentTab.metadata?.tableName) {
    queryStore.generateTableQuery(
      currentTab.connectionId,
      currentTab.metadata.tableName,
      type,
      currentTab.database,
      currentTab.schema
    );
  }
  showQueryDropdown.value = false;
}

function toggleQueryDropdown() {
  showQueryDropdown.value = !showQueryDropdown.value;
  if (showQueryDropdown.value && dropdownBtnRef.value) {
    const rect = dropdownBtnRef.value.getBoundingClientRect();
    dropdownPos.value = {
      top: rect.bottom + 4,
      right: window.innerWidth - rect.right
    };
  }
}

function handleClickOutside(e: MouseEvent) {
  const target = e.target as HTMLElement;
  if (!target.closest('.query-dropdown-trigger')) {
    showQueryDropdown.value = false;
  }
}

function checkAndLoad() {
  const currentTab = tab.value;
  if (currentTab?.type === 'table_data') {
    const existingResult = queryStore.queryResults[props.tabId];
    if (!existingResult || (existingResult.results.length === 0 && !existingResult.loading && !existingResult.error)) {
      queryStore.executeQueryInTab(props.tabId);
    }
  }
}

let resizeObserver: ResizeObserver | null = null;

watch(() => props.tabId, () => { checkAndLoad(); });

onMounted(() => {
  checkAndLoad();
  resizeObserver = new ResizeObserver(entries => {
    for (const entry of entries) {
      toolbarWidth.value = entry.contentRect.width;
    }
  });
  if (toolbarRef.value) resizeObserver.observe(toolbarRef.value);
  document.addEventListener('click', handleClickOutside, true);
});

onUnmounted(() => {
  resizeObserver?.disconnect();
  document.removeEventListener('click', handleClickOutside, true);
});
</script>

<template>
  <div class="table-view-container h-full flex flex-col">
    <div ref="toolbarRef" class="editor-toolbar glass">
      <div class="toolbar-inner">
        <!-- Connection Info -->
        <div class="connection-info flex-shrink-0" v-if="!tab?.isDetached">
          <Database :size="14" class="text-accent" />
          <div class="flex flex-col justify-center" v-if="!iconsOnly">
            <span class="text-xs font-bold text-accent leading-none">{{ connectionName }}</span>
            <span v-if="tab?.database || tab?.schema" class="text-[10px] opacity-70 text-accent leading-none">
              {{ tab.database || '' }}{{ tab.database && tab.schema ? '.' : '' }}{{ tab.schema || '' }}
            </span>
          </div>
          <template v-if="!isConnected">
            <span v-if="!iconsOnly" class="text-xs text-muted-foreground italic">(Disconnected)</span>
            <button class="button-primary sm flex-shrink-0" @click="tab?.connectionId && connectionStore.connect(tab.connectionId)">
              Connect
            </button>
          </template>
        </div>

        <!-- Actions -->
        <div class="actions-group">
          <button class="button-secondary sm" :title="iconsOnly ? 'Reload' : undefined" @click="handleReload">
            <RefreshCw :size="14" />
            <span v-if="!iconsOnly">Reload</span>
          </button>

          <button class="button-secondary sm" :title="iconsOnly ? 'Structure' : undefined" @click="handleEditTable">
            <Edit :size="14" />
            <span v-if="!iconsOnly">Structure</span>
          </button>

          <button class="button-secondary sm" :title="iconsOnly ? 'Add Row' : undefined" @click="handleAddRow">
            <Plus :size="14" />
            <span v-if="!iconsOnly">Add Row</span>
          </button>

          <button class="button-ai-sparkle sm" :title="iconsOnly ? 'Ask AI' : undefined" @click="tabStore.openAiSqlEditor(tab?.connectionId || '', tab?.database, tab?.schema)">
            <Sparkles :size="14" />
            <span v-if="!iconsOnly">Ask AI</span>
          </button>

          <button class="button-secondary sm" :title="iconsOnly ? 'Text' : undefined" @click="tab && (tab.showDetail = !tab.showDetail)" :class="{ 'active': tab?.showDetail }">
            <FileText :size="14" />
            <span v-if="!iconsOnly">Text</span>
          </button>

          <div class="toolbar-divider"></div>

          <!-- Query buttons: dropdown when toolbar is narrow -->
          <template v-if="showQueryAsDropdown">
            <div ref="dropdownBtnRef" class="query-dropdown-trigger">
              <button class="button-ghost sm" @click.stop="toggleQueryDropdown" title="Generate Query">
                <Code :size="14" />
                <ChevronDown :size="12" />
              </button>
            </div>
          </template>
          <template v-else>
            <div class="query-btns">
              <button class="button-ghost sm text-[10px] font-bold" @click="handleGenerateQuery('SELECT')">SELECT</button>
              <button class="button-ghost sm text-[10px] font-bold" @click="handleGenerateQuery('INSERT')">INSERT</button>
              <button class="button-ghost sm text-[10px] font-bold" @click="handleGenerateQuery('UPDATE')">UPDATE</button>
              <button class="button-ghost sm text-[10px] font-bold" @click="handleGenerateQuery('DELETE')">DELETE</button>
            </div>
          </template>
        </div>
      </div>
    </div>

    <!-- Query dropdown — Teleport to body to escape stacking context -->
    <Teleport to="body">
      <div
        v-if="showQueryDropdown"
        class="query-dropdown"
        :style="{ top: dropdownPos.top + 'px', right: dropdownPos.right + 'px' }"
        @click.stop
      >
        <button class="query-dropdown-item" @click="handleGenerateQuery('SELECT')">SELECT</button>
        <button class="query-dropdown-item" @click="handleGenerateQuery('INSERT')">INSERT</button>
        <button class="query-dropdown-item" @click="handleGenerateQuery('UPDATE')">UPDATE</button>
        <button class="query-dropdown-item" @click="handleGenerateQuery('DELETE')">DELETE</button>
      </div>
    </Teleport>

    <!-- Detached Banner -->
    <div v-if="tab?.isDetached" class="detached-banner flex-between">
      <div class="flex-center gap-2">
        <Unplug :size="14" />
        <span>This table view is detached.</span>
      </div>
      <button class="button-warning sm" @click="tabStore.openReconnectModal(tab)">Reconnect</button>
    </div>

    <!-- Main Grid Content -->
    <div class="flex-1 overflow-hidden">
      <DataGrid ref="dataGridRef" :tab-id="tabId" />
    </div>
  </div>
</template>

<style scoped>
.table-view-container {
  background: var(--bg-primary);
}

.editor-toolbar {
  height: 40px;
  padding: 0 12px;
  border-bottom: 1px solid var(--border-color);
  background: var(--bg-secondary);
  flex-shrink: 0;
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: none;
}
.editor-toolbar::-webkit-scrollbar { display: none; }

/* Left-aligned flex row — no justify-content: center */
.toolbar-inner {
  height: 100%;
  min-width: max-content;
  display: flex;
  align-items: center;
  gap: 12px;
}

.connection-info {
  display: flex;
  align-items: center;
  gap: 6px;
}

.actions-group {
  display: flex;
  align-items: center;
  gap: 6px;
}

.query-btns {
  display: flex;
  align-items: center;
  gap: 2px;
}

.sm {
  padding: 4px 10px;
  font-size: 0.78rem;
}

.detached-banner {
  background: rgba(234, 179, 8, 0.1);
  color: rgb(234, 179, 8);
  padding: 8px 12px;
  border-bottom: 1px solid rgba(234, 179, 8, 0.2);
}

.button-warning {
  background: rgba(234, 179, 8, 0.2);
  color: rgb(234, 179, 8);
  border: 1px solid rgba(234, 179, 8, 0.3);
  border-radius: 4px;
  cursor: pointer;
}

.button-secondary.active {
  background: var(--accent-primary);
  color: white;
  border-color: var(--accent-primary);
}

.toolbar-divider {
  width: 1px;
  height: 20px;
  background: var(--border-color);
  opacity: 0.5;
  flex-shrink: 0;
}

.button-ghost {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: transparent;
  border: 1px solid transparent;
  color: var(--text-secondary);
  cursor: pointer;
  border-radius: 4px;
  padding: 4px 8px;
  font-size: 0.78rem;
}
.button-ghost:hover {
  background: var(--glass-border);
  color: var(--text-primary);
  border-color: var(--border-color);
}

/* Dropdown — fixed position via Teleport, no z-index fight */
.query-dropdown {
  position: fixed;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.25);
  z-index: 3000;
  min-width: 110px;
  overflow: hidden;
}

.query-dropdown-item {
  display: block;
  width: 100%;
  padding: 7px 14px;
  text-align: left;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  font-size: 0.75rem;
  font-weight: 700;
  cursor: pointer;
  transition: background 0.15s;
}
.query-dropdown-item:hover {
  background: var(--glass-bg);
  color: var(--text-primary);
}

.query-dropdown-trigger { position: relative; }
</style>
