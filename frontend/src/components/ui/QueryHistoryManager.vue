<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';
import { useUiStore } from '../../stores/ui';
import { useQueryStore } from '../../stores/query';
import { History, X, CheckCircle, XCircle, ChevronRight, Copy, Clock } from 'lucide-vue-next';
import type { QueryHistoryEntry } from '../../types';

const props = defineProps<{
  tabId: string;
  show: boolean;
}>();

const emit = defineEmits(['close', 'restore']);

const uiStore = useUiStore();
const queryStore = useQueryStore();
const history = ref<QueryHistoryEntry[]>([]);
const loading = ref(false);

async function loadHistory() {
  if (!props.show) return;
  loading.value = true;
  try {
    const allHistory = await queryStore.getQueryHistory();
    history.value = allHistory.filter(e => e.scriptId === props.tabId).reverse();
  } catch (err: any) {
    console.error('Failed to load history:', err);
  } finally {
    loading.value = false;
  }
}

function formatDate(dateStr: string) {
  const date = new Date(dateStr);
  return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

function getStatusIcon(status: string) {
  return status === 'success' ? CheckCircle : XCircle;
}

function getStatusClass(status: string) {
  return status === 'success' ? 'text-success' : 'text-error';
}

watch(() => props.show, (newVal) => {
  if (newVal) loadHistory();
});

onMounted(() => {
  if (props.show) loadHistory();
});
</script>

<template>
  <div v-if="show" class="manager-sidebar">
    <div class="sidebar-header">
      <div class="flex-center gap-1.5">
        <History :size="13" class="text-accent" style="opacity: 0.65;" />
        <span class="sidebar-title">Execution History</span>
      </div>
      <button class="close-btn" @click="$emit('close')" title="Close">
        <X :size="13" />
      </button>
    </div>

    <div class="sidebar-content">
      <div v-if="loading" class="state-box">
        <div class="spinner-xs"></div>
        <span class="state-text">Loading…</span>
      </div>
      <div v-else-if="history.length === 0" class="state-box empty">
        <Clock :size="22" class="state-icon" />
        <span class="state-text">No execution history</span>
      </div>
      <div v-else class="item-list">
        <div v-for="entry in history"
             :key="entry.id"
             class="sidebar-item group"
             @click="uiStore.openHistoryPopup(entry)">
          <div class="item-row">
            <div class="flex-center gap-1.5">
              <component :is="getStatusIcon(entry.status)" :size="11" :class="getStatusClass(entry.status)" />
              <span class="meta-text">{{ formatDate(entry.timestamp) }}</span>
            </div>
            <div class="flex-center gap-1">
              <span class="meta-text" style="opacity: 0.35;">{{ entry.durationMs }}ms</span>
              <button class="icon-action group-show" title="Copy" @click.stop="uiStore.copyToClipboard(entry.query)">
                <Copy :size="10" />
              </button>
            </div>
          </div>
          <div class="query-preview">{{ entry.query }}</div>
          <div class="item-row" style="margin-top: 4px;">
            <span class="meta-text" style="opacity: 0.4; font-size: 9px;">
              {{ entry.status === 'success' ? `${entry.affectedRows || 0} rows` : 'Failed' }}
            </span>
            <ChevronRight :size="10" style="opacity: 0.25;" />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.manager-sidebar {
  width: 220px;
  height: 100%;
  border-left: 1px solid var(--border-color);
  background: var(--bg-secondary);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  z-index: 5;
}

.sidebar-header {
  height: 36px;
  padding: 0 10px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
}

.sidebar-title {
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  opacity: 0.6;
}

.close-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  opacity: 0.5;
  transition: opacity 0.15s, background 0.15s;
}

.close-btn:hover {
  background: var(--glass-border);
  opacity: 1;
}

.sidebar-content {
  flex: 1;
  overflow-y: auto;
  padding: 6px;
}

/* States */
.state-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 32px 0;
  opacity: 0.4;
}

.state-box.empty { opacity: 0.25; }
.state-icon { opacity: 0.6; }
.state-text { font-size: 11px; }

/* List */
.item-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.sidebar-item {
  padding: 7px 8px;
  border-radius: 6px;
  border: 1px solid transparent;
  background: var(--glass-bg);
  cursor: pointer;
  transition: border-color 0.15s, background 0.15s;
}

.sidebar-item:hover {
  border-color: rgba(var(--accent-primary-rgb), 0.35);
  background: rgba(var(--accent-primary-rgb), 0.05);
}

.item-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 4px;
}

.meta-text {
  font-size: 10px;
  opacity: 0.55;
}

.query-preview {
  font-family: 'Fira Code', 'JetBrains Mono', monospace;
  font-size: 11px;
  opacity: 0.8;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* Group-show: hidden until item is hovered */
.icon-action {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border: none;
  border-radius: 3px;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.15s, background 0.15s;
}

.sidebar-item:hover .icon-action {
  opacity: 1;
}

.icon-action:hover {
  background: var(--glass-border);
  color: var(--text-primary);
}

.text-success { color: #10b981; }
.text-error   { color: #f87171; }

.spinner-xs {
  width: 14px;
  height: 14px;
  border: 2px solid var(--glass-border);
  border-top-color: var(--accent-primary);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin { to { transform: rotate(360deg); } }
</style>
