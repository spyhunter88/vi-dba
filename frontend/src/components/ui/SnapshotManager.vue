<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';
import { useUiStore } from '../../stores/ui';
import { useSessionStore } from '../../stores/session';
import { Clock, X, FileText, ChevronRight } from 'lucide-vue-next';
import { invoke } from '@tauri-apps/api/core';

const props = defineProps<{
  tabId: string;
  show: boolean;
}>();

const emit = defineEmits(['close', 'restore']);

const uiStore = useUiStore();
const sessionStore = useSessionStore();
const snapshots = ref([] as any[]);
const loading = ref(false);

function parseSnapshotDate(dateStr: string): Date | null {
  if (dateStr.length !== 15) return null;
  const year   = parseInt(dateStr.substring(0, 4));
  const month  = parseInt(dateStr.substring(4, 6)) - 1;
  const day    = parseInt(dateStr.substring(6, 8));
  const hour   = parseInt(dateStr.substring(9, 11));
  const minute = parseInt(dateStr.substring(11, 13));
  const second = parseInt(dateStr.substring(13, 15));
  return new Date(year, month, day, hour, minute, second);
}

function formatRelativeTime(dateStr: string) {
  const date = parseSnapshotDate(dateStr);
  if (!date) return dateStr;
  const diffInSeconds = Math.floor((Date.now() - date.getTime()) / 1000);
  if (diffInSeconds < 60)     return 'Just now';
  if (diffInSeconds < 3600)   return `${Math.floor(diffInSeconds / 60)} min ago`;
  if (diffInSeconds < 86400)  return `${Math.floor(diffInSeconds / 3600)}h ago`;
  if (diffInSeconds < 172800) return 'Yesterday';
  if (diffInSeconds < 2592000) return `${Math.floor(diffInSeconds / 86400)} days ago`;
  return date.toLocaleDateString();
}

function formatLocalTime(dateStr: string) {
  const date = parseSnapshotDate(dateStr);
  return date ? date.toLocaleString() : dateStr;
}

function formatTime(dateStr: string) {
  const date = parseSnapshotDate(dateStr);
  if (!date) return dateStr;
  return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

async function loadSnapshots() {
  if (!props.show) return;
  loading.value = true;
  try {
    snapshots.value = await sessionStore.getSnapshots(props.tabId);
  } catch (e) {
    console.error('Failed to load snapshots:', e);
  } finally {
    loading.value = false;
  }
}

async function restoreSnapshot(snapshot: any) {
  try {
    const content = await invoke<string>('read_snapshot', { path: snapshot.path });
    emit('restore', content);
    uiStore.showToast('Snapshot restored', 'success');
  } catch (e) {
    console.error('Failed to restore snapshot:', e);
    uiStore.showToast('Failed to restore snapshot: ' + e, 'error');
  }
}

watch(() => props.show, (newVal) => {
  if (newVal) loadSnapshots();
});

onMounted(() => {
  if (props.show) loadSnapshots();
});
</script>

<template>
  <div v-if="show" class="manager-sidebar">
    <div class="sidebar-header">
      <div class="flex-center gap-1.5">
        <Clock :size="13" class="text-accent" style="opacity: 0.65;" />
        <span class="sidebar-title">Snapshots</span>
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
      <div v-else-if="snapshots.length === 0" class="state-box empty">
        <FileText :size="22" class="state-icon" />
        <span class="state-text">No snapshots yet</span>
      </div>
      <div v-else class="item-list">
        <div v-for="ss in snapshots"
             :key="ss.timestamp"
             class="sidebar-item"
             :title="formatLocalTime(ss.timestamp)"
             @click="restoreSnapshot(ss)">
          <div class="item-row">
            <div class="flex-center gap-1.5">
              <Clock :size="11" style="opacity: 0.45; flex-shrink: 0;" />
              <span class="meta-text">{{ formatTime(ss.timestamp) }}</span>
            </div>
            <ChevronRight :size="10" style="opacity: 0.25;" />
          </div>
          <div class="relative-time">{{ formatRelativeTime(ss.timestamp) }}</div>
        </div>
      </div>
    </div>

    <div class="sidebar-footer">Auto-saved on execution</div>
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
  margin-bottom: 3px;
}

.meta-text {
  font-size: 10px;
  opacity: 0.55;
}

.relative-time {
  font-size: 11px;
  font-weight: 600;
  opacity: 0.85;
  padding-left: 19px; /* align under text past the clock icon */
}

.sidebar-footer {
  padding: 6px 10px;
  border-top: 1px solid var(--border-color);
  font-size: 10px;
  opacity: 0.3;
  text-align: center;
  font-style: italic;
  flex-shrink: 0;
}

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
