<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { useUiStore } from '../stores/ui';
import { listen } from '@tauri-apps/api/event';
import { 
  Copy, 
  Trash2, 
  CheckCircle, 
  XCircle, 
  Clock, 
  ExternalLink 
} from 'lucide-vue-next';

const uiStore = useUiStore();
const entry = ref<any>(null);
let unlisten: (() => void) | null = null;

onMounted(async () => {
  // Listen for updates from main window
  unlisten = await listen('vi-history-detail-update', (event) => {
    console.log('[HistoryDetail] Received update:', event.payload);
    entry.value = event.payload;
  });
  
  // Request initial data from main window since it might have been missed during creation
  import('@tauri-apps/api/event').then(({ emit }) => {
    emit('vi-history-detail-ready');
  });
});

onUnmounted(() => {
  if (unlisten) unlisten();
});

async function copyToClipboard() {
  if (entry.value) {
    await uiStore.copyToClipboard(entry.value.query);
  }
}

function pasteAtPointer() {
  if (entry.value) {
    window.dispatchEvent(new CustomEvent('vi-paste-query', { 
      detail: { query: entry.value.query } 
    }));
    // Note: This dispatchEvent is local to THIS window. 
    // We need to EMIT a Tauri event so the main window can catch it.
    uiStore.showToast('Requesting paste in editor...');
    import('@tauri-apps/api/event').then(({ emit }) => {
       emit('vi-paste-query-request', { query: entry.value.query });
    });
  }
}

async function removeEntry() {
  if (!entry.value) return;
  
  if (confirm('Are you sure you want to remove this history record?')) {
    uiStore.showToast('Clearing from view.');
    entry.value = null;
  }
}

function formatDate(dateStr: string) {
  if (!dateStr) return '';
  const date = new Date(dateStr);
  return date.toLocaleString();
}
</script>

<template>
  <div class="history-detail-container flex-column h-screen bg-secondary p-1">
    <div v-if="!entry" class="flex-center flex-1 opacity-50 flex-col gap-4">
      <Clock :size="48" />
      <span>Waiting for query data...</span>
    </div>
    
    <template v-else>
      <div class="detail-header flex-between border-b bg-tertiary">
        <div class="flex-center gap-2">
          <Clock :size="16" class="text-accent opacity-70 p-1" />
          <span class="font-bold text-sm uppercase letter-spacing-wide">Query Details</span>
        </div>
      </div>

      <div class="detail-body p-2 flex-1 flex-column gap-2 overflow-y-auto">
        <div class="info-list flex-column bg-primary p-3 rounded-lg border">
          <div class="info-row">
            <span class="label">Date:</span>
            <span class="value">{{ formatDate(entry.timestamp) }}</span>
          </div>
          <div class="info-row">
            <span class="label">Duration:</span>
            <span class="value">{{ entry.durationMs }}ms</span>
          </div>
          <div class="info-row">
            <span class="label">Status:</span>
            <div class="value flex-center gap-1.5" :class="entry.status === 'success' ? 'text-success' : 'text-error'">
              <component :is="entry.status === 'success' ? CheckCircle : XCircle" :size="14" />
              <span>{{ entry.status === 'success' ? 'Success' : 'Failed' }}</span>
            </div>
          </div>
          <div class="info-row">
            <span class="label">Rows:</span>
            <span class="value">{{ entry.affectedRows || 0 }}</span>
          </div>
        </div>

        <div class="query-section flex-column gap-2 pt-2 flex-1 min-h-0">
          <span class="label-heading">SQL Query</span>
          <div class="query-area flex-1 overflow-auto">
            <pre><code>{{ entry.query }}</code></pre>
          </div>
        </div>

        <div v-if="entry.errorMessage" class="error-section p-2 rounded-lg bg-error-dim border-l-4 border-error">
           <div class="text-xs font-bold text-error mb-1 uppercase">Error Message</div>
           <p class="text-sm opacity-90">{{ entry.errorMessage }}</p>
        </div>
      </div>

      <div class="detail-footer border-t bg-tertiary flex flex-wrap gap-2 justify-end">
        <button class="button-secondary sm flex-center gap-1.5" @click="copyToClipboard">
          <Copy :size="14" />
          Copy
        </button>
        <button class="button-primary sm flex-center gap-1.5" @click="pasteAtPointer">
          <ExternalLink :size="14" />
          Paste at Pointer
        </button>
        <button class="button-danger sm-icon" title="Remove Record" @click="removeEntry">
          <Trash2 :size="14" />
          Remove
        </button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.history-detail-container {
  display: flex;
  flex-direction: column;
  background: var(--bg-secondary);
  color: var(--text-primary);
}

.info-row {
  display: flex;
  align-items: center;
  font-size: 0.9rem;
}

.label {
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  color: var(--text-secondary);
  opacity: 0.6;
  width: 80px;
  flex-shrink: 0;
}

.label-heading {
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  color: var(--text-secondary);
  opacity: 0.6;
}

.value {
  font-weight: 500;
}

.query-area {
  background: rgba(0, 0, 0, 0.3);
  padding: 12px;
  border-radius: 8px;
  font-family: 'Fira Code', monospace;
  font-size: 0.9rem;
  border: 1px solid var(--border-color);
}

pre {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-all;
}

.bg-error-dim { background: rgba(248, 113, 113, 0.05); }
.border-error { border-color: #f87171; }
.text-error { color: #f87171; }
.text-success { color: #10b981; }

.close-btn {
  background: transparent;
  border: none;
  cursor: pointer;
  padding: 4px;
  display: flex;
  border-radius: 4px;
  color: var(--text-secondary);
  transition: all 0.2s;
}

.close-btn:hover {
  background: var(--glass-border);
  color: var(--text-primary);
}

.sm {
  padding: 6px 12px;
  font-size: 0.8rem;
}

.sm-icon {
  padding: 6px;
  border-radius: 6px;
}

.letter-spacing-wide { letter-spacing: 0.1em; }

.flex-column { display: flex; flex-direction: column; }
.flex-1 { flex: 1; }
.h-screen { height: 100vh; }
.p-1 { padding: 0.25rem; }
.pt-2 { padding-top: 0.5rem; }
.pb-2 { padding-bottom: 0.5rem; }
</style>
