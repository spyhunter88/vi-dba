<script setup lang="ts">
import { computed, ref, onMounted, nextTick } from 'vue';
import { useTabStore } from '../../stores/tabs';
import { useQueryStore } from '../../stores/query';
import { useRouter } from 'vue-router';
import { RefreshCw, Search, Database, Box, AlertCircle, Edit2, Trash2, Clock } from 'lucide-vue-next';

const props = defineProps<{
  tabId: string;
}>();

const tabStore = useTabStore();
const queryStore = useQueryStore();
const router = useRouter();

const tab = computed(() => tabStore.tabs.find((t: any) => t.id === props.tabId));
const resultData = computed(() => queryStore.queryResults[props.tabId]);

const columns = computed(() => resultData.value?.results?.[0]?.columns || []);
const rows = computed(() => resultData.value?.results?.[0]?.rows || []);
const searchQuery = ref('');

const filteredRows = computed(() => {
  if (!searchQuery.value) return rows.value;
  const q = searchQuery.value.toLowerCase();
  return rows.value.filter((row: any) => {
    // Check multiple possible name columns
    const nameStr = String(row['Name'] || row['name'] || row['ROUTINE_NAME'] || '').toLowerCase();
    return nameStr.includes(q);
  });
});

async function refresh() {
  await queryStore.loadRoutineList(props.tabId);
}

function handleRowDoubleClick(row: any) {
  if (!tab.value) return;
  
  const name = row.Name || row.name || row.ROUTINE_NAME;
  const type = row.Type || row.type || row.ROUTINE_TYPE;
  
  if (name) {
    tabStore.editRoutine(
      tab.value.connectionId, 
      name, 
      type || 'PROCEDURE',
      tab.value.database,
      tab.value.schema
    );
  }
}

const menuVisible = ref(false);
const menuPos = ref({ x: 0, y: 0 });
const contextRow = ref<any>(null);
const menuEl = ref<HTMLElement | null>(null);

function showContextMenu(e: MouseEvent, row: any) {
  e.preventDefault();
  contextRow.value = row;
  menuPos.value = { x: e.clientX, y: e.clientY };
  menuVisible.value = true;
  
  nextTick(() => {
    if (menuEl.value) {
      const menuRect = menuEl.value.getBoundingClientRect();
      const windowWidth = window.innerWidth;
      const windowHeight = window.innerHeight;
      
      let x = e.clientX;
      let y = e.clientY;
      
      if (x + menuRect.width > windowWidth) {
        x = windowWidth - menuRect.width - 5;
      }
      
      if (y + menuRect.height > windowHeight) {
        y = windowHeight - menuRect.height - 5;
      }
      
      menuPos.value = { x, y };
    }
  });

  window.addEventListener('click', closeMenu);
}

function closeMenu() {
  menuVisible.value = false;
  window.removeEventListener('click', closeMenu);
}

function handleEditRoutine() {
  if (contextRow.value) {
    handleRowDoubleClick(contextRow.value);
  }
  closeMenu();
}

function handleViewHistory() {
  if (contextRow.value && tab.value) {
    const row = contextRow.value;
    const name = row.Name || row.name || row.ROUTINE_NAME;
    const type = 'routine';
    const tabId = `${type}-${tab.value.connectionId}-${name}`;
    router.push({ 
      path: '/history', 
      query: { 
        tabId, 
        connectionId: tab.value.connectionId, 
        database: tab.value.database, 
        schema: tab.value.schema 
      } 
    });
  }
  closeMenu();
}

function formatValue(col: string, val: any) {
  if (val === null || val === undefined) return '';
  
  // Convert timestamps to local timezone
  if (col.toLowerCase().includes('created') || col.toLowerCase().includes('modified') || col.toLowerCase().includes('updated')) {
    try {
      const date = new Date(val);
      if (!isNaN(date.getTime())) {
        return date.toLocaleString();
      }
    } catch (e) {
      // ignore
    }
  }
  return val;
}

onMounted(() => {
    queryStore.loadRoutineList(props.tabId);
});
</script>

<template>
  <div class="routine-list-view">
    <div v-if="resultData?.loading" class="flex-center h-full gap-2 text-secondary">
      <RefreshCw :size="20" class="spin" />
      Fetching routines...
    </div>

    <template v-else>
      <div v-if="resultData?.error" class="error-container flex-center flex-direction-column gap-3">
        <AlertCircle :size="48" class="text-error" />
        <div class="error-message">{{ resultData.error }}</div>
      </div>
      
      <div v-else class="routine-container">
        <div class="routine-toolbar flex-between">
          <div class="toolbar-left flex-center gap-2">
            <div class="connection-info flex-center gap-2 mr-4">
              <Database :size="14" class="text-accent" />
              <div class="flex flex-col">
                <span class="text-xs font-bold text-accent leading-tight">{{ tab?.database || 'Default' }}</span>
                <span v-if="tab?.schema" class="text-9px opacity-70 text-accent leading-none">
                  {{ tab.schema }}
                </span>
              </div>
            </div>
            <button class="toolbar-btn" @click="refresh">
              <RefreshCw :size="14" :class="{ 'spin': resultData?.loading }" />
            </button>
          </div>

          <div class="toolbar-right">
            <div class="search-input-wrapper relative">
              <Search class="search-icon" :size="14" />
              <input 
                v-model="searchQuery"
                type="text" 
                placeholder="Filter routines..." 
                class="premium-input"
              />
            </div>
          </div>
        </div>

        <table class="data-grid">
          <thead>
            <tr>
              <th v-for="col in columns" :key="col">{{ col }}</th>
            </tr>
          </thead>
          <tbody>
            <tr 
              v-for="(row, idx) in filteredRows" 
              :key="idx"
              :class="{ selected: contextRow === row }"
              @dblclick="handleRowDoubleClick(row)"
              @contextmenu="showContextMenu($event, row)"
            >
              <td 
                v-for="col in columns" 
                :key="col"
                :class="{ 'font-medium': col === 'Name' || col === 'ROUTINE_NAME' }"
              >
                <div v-if="col === 'Name' || col === 'ROUTINE_NAME'" class="flex-center gap-2 justify-start">
                  <Box :size="14" class="text-accent" />
                  <span class="text-accent">{{ row[col] }}</span>
                </div>
                <template v-else>
                  {{ formatValue(col, row[col]) }}
                </template>
              </td>
            </tr>
            <tr v-if="rows.length === 0">
              <td :colspan="columns.length + 1" class="text-center p-8 text-secondary">
                No routines found.
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <div class="routine-footer flex-between">
        <div class="stats">
          {{ filteredRows.length }} routines {{ searchQuery ? `(filtered from ${rows.length})` : '' }}
        </div>
      </div>
    </template>

    <div 
      v-if="menuVisible" 
      ref="menuEl"
      class="context-menu glass"
      :style="{ top: menuPos.y + 'px', left: menuPos.x + 'px' }"
      @click.stop
    >
      <button class="menu-item" @click="handleEditRoutine">
        <Edit2 :size="14" />
        Edit Routine
      </button>
      <button class="menu-item" @click="handleViewHistory">
        <Clock :size="14" />
        View History
      </button>
      <div class="menu-divider"></div>
      <button class="menu-item text-error" disabled>
        <Trash2 :size="14" />
        Drop Routine
      </button>
    </div>
  </div>
</template>

<style scoped>
.routine-list-view {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg-primary);
}

.routine-container {
  flex: 1;
  overflow: auto;
}

.data-grid {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.75rem;
}

.data-grid th {
  position: sticky;
  top: 0;
  background: var(--bg-secondary);
  padding: 6px 12px;
  text-align: left;
  border-bottom: 1px solid var(--border-color);
  font-weight: 600;
  z-index: 1;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  font-size: 0.7rem;
}

.data-grid td {
  padding: 4px 12px;
  border-bottom: 1px solid var(--border-color);
  white-space: nowrap;
  color: var(--text-primary);
}

.data-grid tr:hover {
  background: var(--glass-border);
}

.routine-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
  height: 40px;
  flex-shrink: 0;
}

.routine-footer {
  height: 28px;
  padding: 0 12px;
  font-size: 0.75rem;
  color: var(--text-secondary);
  border-top: 1px solid var(--border-color);
  background: var(--bg-tertiary);
  flex-shrink: 0;
  display: flex;
  align-items: center;
}

.stats {
  white-space: nowrap;
}

.toolbar-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  background: transparent;
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  padding: 4px 10px;
  border-radius: 4px;
  font-size: 0.75rem;
  cursor: pointer;
  transition: all 0.2s;
}

.toolbar-btn:hover:not(:disabled) {
  background: var(--glass-border);
}

.toolbar-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.search-input-wrapper {
  position: relative;
  display: flex;
  align-items: center;
}

.search-icon {
  position: absolute;
  left: 10px;
  color: var(--text-secondary);
  pointer-events: none;
}

.premium-input {
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  padding: 4px 12px 4px 32px;
  border-radius: 4px;
  outline: none;
  transition: all 0.2s;
  font-size: 0.75rem;
  width: 200px;
}

.premium-input:focus {
  border-color: var(--accent-primary);
  box-shadow: 0 0 0 2px rgba(var(--accent-primary-rgb, 59, 130, 246), 0.1);
}

.error-container {
  height: 100%;
  padding: 20px;
}

.error-message {
  color: var(--text-error);
  font-family: var(--font-mono);
  text-align: center;
  max-width: 80%;
}

.spin {
  animation: spin 2s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.data-grid tr.selected {
  background: rgba(var(--accent-primary-rgb, 59, 130, 246), 0.1);
  outline: 1px solid var(--accent-primary);
  outline-offset: -1px;
}

.context-menu {
  position: fixed;
  min-width: 180px;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 4px;
  z-index: 3000;
  box-shadow: var(--shadow-lg);
}

.menu-item {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  background: transparent;
  border: none;
  border-radius: 6px;
  color: var(--text-primary);
  font-size: 0.85rem;
  cursor: pointer;
  transition: all 0.2s;
  text-align: left;
}

.menu-item:hover {
  background: var(--glass-border);
}

.menu-divider {
  height: 1px;
  background: var(--border-color);
  margin: 4px;
}

.text-error { color: var(--text-error); }
.text-accent { color: var(--accent-primary); }
.font-medium { font-weight: 500; }
.text-center { text-align: center; }
.justify-start { justify-content: flex-start; }
</style>
