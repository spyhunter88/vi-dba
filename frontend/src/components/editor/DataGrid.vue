<script setup lang="ts">
import { ref, computed, watch, onBeforeUnmount, onMounted, nextTick } from 'vue';
import {
  X, ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight,
  Trash2, Check,
  Scaling,
  RefreshCw,
  Code,
  FileText as FileTextIcon,
  Globe,
  Calendar,
  Loader2
} from 'lucide-vue-next';
import * as monaco from 'monaco-editor';
import { useTabStore } from '../../stores/tabs';
import { useQueryStore } from '../../stores/query';
import { useUiStore } from '../../stores/ui';
import type { QueryResult } from '../../types';

const props = defineProps<{
  tabId: string;
  result?: QueryResult;
  totalTimeMs?: number;
}>();

const tabStore = useTabStore();
const queryStore = useQueryStore();

const uiStore = useUiStore();
const editingCell = ref<{ rowIndex: number; colName: string; value: any; initialValue: any; isPending?: boolean } | null>(null);
const selectedCell = ref<{ rowIndex: number; colName: string; value: any } | null>(null);
const pendingRows = ref<any[]>([]);

const detailMode = ref<'text' | 'json' | 'html'>('text');
const monacoContainer = ref<HTMLElement | null>(null);
let monacoInstance: monaco.editor.IStandaloneCodeEditor | null = null;

const isLoading = computed(() => queryStore.queryResults[props.tabId]?.loading ?? false);

const displayResult = computed(() => {
  if (props.result) return props.result;
  const tabResult = queryStore.queryResults[props.tabId];
  return tabResult?.results?.[0] || null;
});

const displayRows = computed(() => displayResult.value?.rows || []);
const displayColumns = computed(() => displayResult.value?.columns || []);

// --- Virtual scrolling ---
// For result sets above VIRTUAL_THRESHOLD, only the visible window of rows is rendered.
// Top/bottom spacer rows preserve scroll height so the scrollbar reflects the full data set.
const ROW_HEIGHT = 26;          // matches .data-grid tbody tr { height: 26px }
const BUFFER_ROWS = 12;         // rows rendered outside viewport on each side
const VIRTUAL_THRESHOLD = 500;  // below this, render everything (no virt overhead)

const scrollContainer = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const containerHeight = ref(600);

const shouldVirtualize = computed(() => displayRows.value.length >= VIRTUAL_THRESHOLD);

const visibleRange = computed<{ start: number; end: number }>(() => {
  const total = displayRows.value.length;
  if (!shouldVirtualize.value) return { start: 0, end: total };
  const visibleCount = Math.ceil(containerHeight.value / ROW_HEIGHT) + BUFFER_ROWS * 2;
  const start = Math.max(0, Math.floor(scrollTop.value / ROW_HEIGHT) - BUFFER_ROWS);
  const end = Math.min(total, start + visibleCount);
  return { start, end };
});

const visibleRows = computed(() =>
  shouldVirtualize.value
    ? displayRows.value.slice(visibleRange.value.start, visibleRange.value.end)
    : displayRows.value
);

const topSpacerHeight = computed(() =>
  shouldVirtualize.value ? visibleRange.value.start * ROW_HEIGHT : 0
);

const bottomSpacerHeight = computed(() => {
  if (!shouldVirtualize.value) return 0;
  return Math.max(0, displayRows.value.length - visibleRange.value.end) * ROW_HEIGHT;
});

// Translate a v-for index inside the visible window to the absolute row index.
// Wrapping `idx` here keeps the template free of `Number()` casts — Vue infers
// v-for indices as `string | number` and won't accept `+` against a `number`.
function absRowIndex(idx: number | string): number {
  return visibleRange.value.start + (typeof idx === 'number' ? idx : Number(idx));
}

let scrollRaf: number | null = null;
function onContainerScroll() {
  if (!scrollContainer.value) return;
  // Coalesce scroll events into rAF so we don't trigger reactivity on every pixel.
  if (scrollRaf !== null) return;
  scrollRaf = requestAnimationFrame(() => {
    scrollRaf = null;
    if (scrollContainer.value) scrollTop.value = scrollContainer.value.scrollTop;
  });
}

let resizeObserver: ResizeObserver | null = null;

// Reset scroll when the underlying result changes so we don't show stale rows.
watch(displayResult, () => {
  scrollTop.value = 0;
  if (scrollContainer.value) scrollContainer.value.scrollTop = 0;
});

const isEditable = computed(() => {
  const data = displayResult.value;
  const hasPk = !!(data && data.primaryKeys && data.primaryKeys.length > 0);
  const hasTable = !!(data && data.tableName);
  console.log(`[DataGrid] isEditable for tab ${props.tabId}:`, { hasPk, hasTable, tableName: data?.tableName, pks: data?.primaryKeys });
  return hasPk && hasTable;
});

const tab = computed(() => tabStore.tabs.find(t => t.id === props.tabId));
const pagination = computed(() => tab.value?.pagination);

const vFocus = {
  mounted: (el: HTMLInputElement) => el.focus()
};

async function addNewRow() {
  if (!isEditable.value) return;
  const newRow: any = {};
  displayColumns.value.forEach((col: string) => {
    newRow[col] = '';
  });
  pendingRows.value.push(newRow);
  await nextTick();
  const newIdx = pendingRows.value.length - 1;
  if (displayColumns.value.length > 0) {
    startPendingEditing(newIdx, displayColumns.value[0], '');
  }
}

function removePendingRow(index: number) {
  pendingRows.value.splice(index, 1);
}

async function savePendingRow(index: number) {
  const row = pendingRows.value[index];
  try {
    await queryStore.insertRow(props.tabId, row);
    pendingRows.value.splice(index, 1);
  } catch (e) {
    console.error('Failed to save new row:', e);
  }
}

function startEditing(rowIndex: number, colName: string, value: any) {
  if (!isEditable.value) return;
  // For objects (JSON columns), serialize to string for editing
  const preparedValue = (value !== null && typeof value === 'object') 
    ? JSON.stringify(value) 
    : prepareValueForInput(value);
  editingCell.value = { rowIndex, colName, value: preparedValue, initialValue: value, isPending: false };
}

function startPendingEditing(rowIndex: number, colName: string, value: any) {
  editingCell.value = { rowIndex, colName, value, initialValue: value, isPending: true };
}

function stopEditing() {
  editingCell.value = null;
}

async function saveCell() {
  if (!editingCell.value) return;
  
  let newValue: any = editingCell.value.value;
  const { rowIndex, colName, initialValue, isPending } = editingCell.value;
  
  // Convert type-specific input values back if needed
  if (isDateColumn(colName) && newValue) {
    newValue = newValue.replace('T', ' ');
  }
  
  // For JSON columns, try to parse the string back to an object
  const colType = getColumnType(colName);
  if (colType === 'json' && typeof newValue === 'string' && newValue.trim()) {
    try {
      newValue = JSON.parse(newValue);
    } catch (e) {
      // Keep as string if not valid JSON
    }
  }
  
  // Compare: for objects, compare stringified versions
  const isSame = (typeof newValue === 'object' && typeof initialValue === 'object')
    ? JSON.stringify(newValue) === JSON.stringify(initialValue)
    : newValue === initialValue;
  if (isSame) {
    stopEditing();
    return;
  }

  if (isPending) {
    pendingRows.value[rowIndex][colName] = newValue;
    stopEditing();
    return;
  }

  try {
    const row = isPending ? pendingRows.value[rowIndex] : displayRows.value[rowIndex];
    await queryStore.updateCellValue(props.tabId, row, colName, newValue);
  } catch (e) {
    console.error('Failed to save cell:', e);
  } finally {
    stopEditing();
  }
}

function selectCell(rowIndex: number, colName: string, value: any) {
  selectedCell.value = { rowIndex, colName, value };
}

function formatCellValue(value: any, colName: string) {
  if (value === null || value === undefined) return 'NULL';
  
  // Find column type
  const colIndex = displayColumns.value.indexOf(colName);
  const type = displayResult.value?.columnTypes?.[colIndex]?.toLowerCase() || '';
  
  if (type.includes('datetime') || type.includes('timestamp')) {
    if (typeof value === 'string' && value.includes('-') && value.includes(':')) {
      return prepareValueForInput(value);
    }
    const date = new Date(value);
    if (!isNaN(date.getTime())) {
      const year = date.getFullYear();
      const month = String(date.getMonth() + 1).padStart(2, '0');
      const day = String(date.getDate()).padStart(2, '0');
      const hours = String(date.getHours()).padStart(2, '0');
      const minutes = String(date.getMinutes()).padStart(2, '0');
      const seconds = String(date.getSeconds()).padStart(2, '0');
      return `${year}-${month}-${day} ${hours}:${minutes}:${seconds}`;
    }
  }
  
  return typeof value === 'object' ? JSON.stringify(value) : String(value);
}

function isNull(value: any) {
  return value === null || value === undefined;
}

function getColumnType(colName: string): string {
  const colIndex = displayColumns.value.indexOf(colName);
  return displayResult.value?.columnTypes?.[colIndex]?.toLowerCase() || '';
}

function isDateColumn(colName: string): boolean {
  const type = getColumnType(colName);
  return type.includes('datetime') || type.includes('timestamp') || type.includes('date');
}

function getCellInputType(colName: string): string {
  const type = getColumnType(colName);
  // Revert to text for dates to allow manual entry, but we'll add a picker button
  if (type.includes('email')) return 'email';
  if (type.includes('int') || type.includes('decimal') || type.includes('float') || type.includes('number')) return 'number';
  return 'text';
}

function prepareValueForInput(value: any) {
  if (value === null || value === undefined) return '';
  // For objects (JSON), serialize to string
  if (typeof value === 'object') return JSON.stringify(value);
  let s = String(value);
  // Replace ISO 'T' with space (global)
  s = s.replace(/T/g, ' ');
  // Strip timezone/fractional suffixes if it looks like a datetime (simple check)
  if (s.includes('-') && s.includes(':')) {
    s = s.split('.')[0] || '';
    s = s.split('+')[0] || '';
    s = s.split('Z')[0] || '';
    s = s.trim();
  }
  return s;
}

function onPickerChange(e: Event) {
  const target = e.target as HTMLInputElement;
  if (target.value && editingCell.value) {
    // Convert YYYY-MM-DDTHH:mm:ss to YYYY-MM-DD HH:mm:ss
    editingCell.value.value = target.value.replace('T', ' ');
  }
}

function triggerPicker(e: MouseEvent) {
  const btn = e.currentTarget as HTMLElement;
  const picker = btn.previousElementSibling as HTMLInputElement;
  if (picker && editingCell.value) {
    // Sync picker value with current text value
    const currentVal = editingCell.value.value;
    if (currentVal) {
      // Convert "YYYY-MM-DD HH:mm:ss" to "YYYY-MM-DDTHH:mm:ss"
      picker.value = String(currentVal).replace(' ', 'T');
    }

    // Try showPicker first (modern browser), fallback to click
    if (typeof (picker as any).showPicker === 'function') {
      (picker as any).showPicker();
    } else {
      (picker as HTMLInputElement).click();
    }
  }
}

function initMonaco() {
  if (!monacoContainer.value) return;
  
  if (monacoInstance) {
    monacoInstance.dispose();
  }
  
  monacoInstance = monaco.editor.create(monacoContainer.value, {
    value: '',
    language: 'json',
    theme: uiStore.resolvedTheme === 'light' ? 'vs' : 'vs-dark',
    automaticLayout: true,
    minimap: { enabled: false },
    readOnly: true,
    scrollBeyondLastLine: false,
    fontSize: 12,
    lineNumbers: 'off',
    padding: { top: 8, bottom: 8 },
    wordWrap: 'on',
  });
}

function updateDetailContent() {
  if (!selectedCell.value) return;
  
  const value = selectedCell.value.value;
  const stringValue = value === null || value === undefined ? '' : String(value);
  
  if (detailMode.value === 'json') {
    if (!monacoInstance) initMonaco();
    // Re-check after init attempt
    if (!monacoInstance) return;

    if (value === null || value === undefined) {
      monacoInstance.setValue('null');
      const model = monacoInstance.getModel();
      if (model) monaco.editor.setModelLanguage(model, 'json');
      return;
    }
    try {
      const parsed = typeof value === 'string' ? JSON.parse(value) : value;
      monacoInstance.setValue(JSON.stringify(parsed, null, 2));
      const model = monacoInstance.getModel();
      if (model) monaco.editor.setModelLanguage(model, 'json');
    } catch (e) {
      monacoInstance.setValue(stringValue);
      const model = monacoInstance.getModel();
      if (model) monaco.editor.setModelLanguage(model, 'text');
    }
  }
}

watch([selectedCell, detailMode], async () => {
  if (tab.value?.showDetail) {
    await nextTick();
    updateDetailContent();
  }
});

watch(() => tab.value?.showDetail, async (shown) => {
  if (shown) {
    await nextTick();
    updateDetailContent();
  }
});

async function handleKeydown(e: KeyboardEvent) {
  // If we are editing, let the input handle keys
  if (editingCell.value) return;

  if (e.key === 'Delete' && selectedCell.value) {
    const { rowIndex, colName } = selectedCell.value;
    const row = displayRows.value[rowIndex];
    if (row) {
       try {
         await queryStore.updateCellValue(props.tabId, row, colName, null);
         // Update selected cell local value if needed (usually handled by queryStore/result refresh)
       } catch (err) {
         console.error('Failed to set cell to NULL:', err);
       }
    }
  } else if (e.key === 'Enter' && selectedCell.value) {
     const { rowIndex, colName, value } = selectedCell.value;
     startEditing(rowIndex, colName, value);
     e.preventDefault();
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeydown);
  if (scrollContainer.value) {
    containerHeight.value = scrollContainer.value.clientHeight;
    resizeObserver = new ResizeObserver((entries) => {
      for (const e of entries) containerHeight.value = e.contentRect.height;
    });
    resizeObserver.observe(scrollContainer.value);
  }
});

onBeforeUnmount(() => {
  monacoInstance?.dispose();
  window.removeEventListener('keydown', handleKeydown);
  resizeObserver?.disconnect();
  if (scrollRaf !== null) cancelAnimationFrame(scrollRaf);
});

watch(() => props.tabId, (newId) => {
  console.log(`[DataGrid] tabId changed to ${newId}`);
});

defineExpose({
    addNewRow
});
</script>

<template>
  <div class="results-view">
    <div class="scroll-container" ref="scrollContainer" @scroll.passive="onContainerScroll">
      <div v-if="isLoading" class="empty-state">
        <div class="flex-center flex-col gap-3 mt-4">
          <Loader2 :size="28" class="spin opacity-50" />
          <span class="text-secondary">Loading...</span>
        </div>
      </div>
      <div v-else-if="!displayResult" class="empty-state">
          <div class="flex-center flex-col gap-3 mt-4">
            <Scaling :size="32" class="opacity-20" />
            <span>No results available for this tab.</span>
          </div>
      </div>
      <div v-else-if="displayResult.rows.length === 0 && displayResult.columns.length === 0" class="empty-state">
          <div class="flex-center flex-col gap-2">
            <Check :size="32" class="text-success" />
            <span>Query executed successfully.</span>
            <span class="text-lg font-bold">{{ displayResult.affectedRows }} rows affected.</span>
          </div>
      </div>
      <div v-else-if="displayRows.length === 0" class="empty-state">
        <div class="flex-center flex-col gap-3">
          <span>0 records found.</span>
          <button class="button-secondary" @click="queryStore.executeQueryInTab(tabId)">
            <RefreshCw :size="14" class="mr-2" />
            Reload
          </button>
        </div>
      </div>
      <table v-else class="data-grid">
        <thead>
          <tr>
            <th v-if="isEditable" class="action-col sticky"></th>
            <th v-for="col in displayColumns" :key="col" :title="getColumnType(col)">
              {{ col }}
              <span class="col-type">{{ getColumnType(col) }}</span>
            </th>
          </tr>
        </thead>
        <tbody>
          <tr v-if="topSpacerHeight > 0" class="spacer-row" :style="{ height: topSpacerHeight + 'px' }">
            <td :colspan="(isEditable ? 1 : 0) + displayColumns.length"></td>
          </tr>
          <tr v-for="(row, idx) in visibleRows" :key="absRowIndex(idx)">
            <td v-if="isEditable" class="action-col sticky">
              <div class="flex-center gap-1">
                <template v-if="editingCell && editingCell.rowIndex === absRowIndex(idx) && !editingCell.isPending">
                  <button class="icon-btn-sm text-success" title="Save" @click.stop="saveCell">
                    <Check :size="12" />
                  </button>
                  <button class="icon-btn-sm text-secondary" title="Cancel" @click.stop="stopEditing">
                    <X :size="12" />
                  </button>
                </template>
              </div>
            </td>
            <td
              v-for="col in displayColumns"
              :key="col"
              :class="{
                selected: selectedCell?.rowIndex === absRowIndex(idx) && selectedCell?.colName === col,
                'is-null': isNull(row[col]),
                'editing': editingCell && editingCell.rowIndex === absRowIndex(idx) && editingCell.colName === col,
                'is-date': isDateColumn(col)
              }"
              @click="selectCell(absRowIndex(idx), col, row[col])"
              @dblclick="startEditing(absRowIndex(idx), col, row[col])"
            >
              <div class="cell-wrapper">
                <div v-if="editingCell && !editingCell.isPending && editingCell.rowIndex === absRowIndex(idx) && editingCell.colName === col" class="cell-editor-with-picker">
                  <template v-if="isDateColumn(col)">
                    <input 
                      type="datetime-local" 
                      step="1" 
                      class="hidden-picker" 
                      :style="{ colorScheme: uiStore.resolvedTheme }"
                      @input="onPickerChange"
                    />
                    <button class="picker-trigger left" title="Open Date Picker" @mousedown.prevent @click.stop="triggerPicker">
                      <Calendar :size="12" />
                    </button>
                  </template>
                  <input 
                    v-if="editingCell"
                    v-model="editingCell.value"
                    :type="getCellInputType(col)"
                    class="cell-input"
                    @blur="saveCell"
                    @keyup.enter="saveCell"
                    @keyup.esc="stopEditing"
                    v-focus
                  />
                </div>
                <template v-else>
                  <span class="cell-text">{{ formatCellValue(row[col], col) }}</span>
                </template>
              </div>
            </td>
          </tr>
          <tr v-if="bottomSpacerHeight > 0" class="spacer-row" :style="{ height: bottomSpacerHeight + 'px' }">
            <td :colspan="(isEditable ? 1 : 0) + displayColumns.length"></td>
          </tr>
          <tr v-for="(row, i) in pendingRows" :key="'pending-' + i" class="pending-row">
              <td class="action-col sticky">
                <div class="flex-center gap-1">
                  <button class="icon-btn-sm text-success" title="Save row" @click.stop="savePendingRow(i)">
                    <Check :size="12" />
                  </button>
                  <button class="icon-btn-sm text-error" title="Remove" @click.stop="removePendingRow(i)">
                    <Trash2 :size="12" />
                  </button>
                </div>
              </td>
              <td
                v-for="col in displayColumns"
                :key="col"
                :class="{
                  'editing': editingCell?.rowIndex === i && editingCell?.colName === col && editingCell.isPending
                }"
                @click="startPendingEditing(i, col, row[col])"
              >
                <div class="cell-wrapper">
                  <div v-if="editingCell?.rowIndex === i && editingCell?.colName === col && editingCell.isPending" class="cell-editor-with-picker">
                    <template v-if="isDateColumn(col)">
                      <input 
                        type="datetime-local" 
                        step="1" 
                        class="hidden-picker" 
                        :style="{ colorScheme: uiStore.resolvedTheme }"
                        @input="onPickerChange"
                      />
                      <button class="picker-trigger left" title="Open Date Picker" @mousedown.prevent @click.stop="triggerPicker">
                        <Calendar :size="12" />
                      </button>
                    </template>
                    <input 
                      v-model="editingCell.value"
                      :type="getCellInputType(col)"
                      class="cell-input"
                      @blur="saveCell"
                      @keyup.enter="saveCell"
                      @keyup.esc="stopEditing"
                      v-focus
                    />
                  </div>
                  <div v-else class="flex-between w-full">
                    <span class="cell-text text-secondary italic">{{ formatCellValue(row[col], col) || '(null)' }}</span>
                  </div>
                </div>
              </td>
            </tr>
        </tbody>
      </table>
    </div>
    
    <div class="result-footer flex-between">
      <div class="flex-center gap-4">
        <div class="stats">
          {{ displayResult?.rows.length || 0 }} rows 
          <span v-if="pagination?.total" class="opacity-50">of {{ pagination.total.toLocaleString() }}</span>
          <span v-if="totalTimeMs"> | Total: {{ (totalTimeMs / 1000).toFixed(3) }} s</span>
          <span v-else-if="displayResult?.executionTimeMs"> | {{ (displayResult.executionTimeMs / 1000).toFixed(3) }} s</span>
        </div>
        <button 
          class="text-btn flex-center gap-1" 
          :class="{ active: tab?.showDetail }"
          @click="tab && (tab.showDetail = !tab.showDetail)"
          title="Toggle Cell Detail View"
        >
          <FileTextIcon :size="12" />
          <span>Text</span>
        </button>
      </div>
      
      <div v-if="pagination" class="pagination-controls flex-center gap-1">
        <button class="icon-btn xs" :disabled="pagination.page <= 1" @click="queryStore.changePage(props.tabId, 1)">
          <ChevronsLeft :size="14" />
        </button>
        <button class="icon-btn xs" :disabled="pagination.page <= 1" @click="queryStore.changePage(props.tabId, pagination.page - 1)">
          <ChevronLeft :size="14" />
        </button>
        <span class="page-info">
          Page {{ pagination.page }} 
          <span v-if="pagination.total" class="opacity-50">of {{ Math.ceil(pagination.total / pagination.pageSize) }}</span>
        </span>
        <button class="icon-btn xs" :disabled="!pagination.total" @click="queryStore.changePage(props.tabId, pagination.page + 1)">
          <ChevronRight :size="14" />
        </button>
        <button class="icon-btn xs" title="Last Page" :disabled="!pagination.total" @click="queryStore.goToLastPage(props.tabId)">
          <ChevronsRight :size="14" />
        </button>
      </div>
    </div>

    <!-- Data Detail Area -->
    <div v-if="tab?.showDetail" class="detail-area-bottom">
      <div class="detail-header flex-between">
        <div class="flex-center gap-4">
          <span class="detail-title">Cell Detail: {{ selectedCell?.colName || 'None' }}</span>
          <div class="detail-tabs flex-center">
            <button 
              class="detail-tab" 
              :class="{ active: detailMode === 'text' }"
              @click="detailMode = 'text'"
            >
              <FileTextIcon :size="12" />
              <span>Text</span>
            </button>
            <button 
              class="detail-tab" 
              :class="{ active: detailMode === 'json' }"
              @click="detailMode = 'json'"
            >
              <Code :size="12" />
              <span>JSON</span>
            </button>
            <button 
              class="detail-tab" 
              :class="{ active: detailMode === 'html' }"
              @click="detailMode = 'html'"
            >
              <Globe :size="12" />
              <span>HTML</span>
            </button>
          </div>
        </div>
        <button class="icon-btn xs" @click="tab.showDetail = false">
          <X :size="12" />
        </button>
      </div>
      <div class="detail-content flex-1 overflow-hidden">
        <textarea 
          v-if="detailMode === 'text'"
          readonly 
          class="detail-textarea" 
          :value="selectedCell ? (typeof selectedCell.value === 'object' && selectedCell.value !== null ? JSON.stringify(selectedCell.value, null, 2) : String(selectedCell.value ?? '')) : ''"
          placeholder="Select a cell to view details..."
        ></textarea>
        <div 
          v-if="detailMode === 'json'"
          ref="monacoContainer"
          class="monaco-detail-wrapper"
        ></div>
        <div 
          v-if="detailMode === 'html'"
          class="html-detail-view"
          v-html="selectedCell ? String(selectedCell.value) : ''"
        ></div>
      </div>
    </div>
  </div>
</template>

<style scoped>
@keyframes spin {
  to { transform: rotate(360deg); }
}
.spin {
  animation: spin 1s linear infinite;
}

.col-type {
  display: block;
  font-size: 0.6rem;
  font-weight: normal;
  opacity: 0.5;
  text-transform: uppercase;
  margin-top: 2px;
}

/* Detail Area Bottom */
.detail-area-bottom {
  height: 200px;
  border-top: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  background: var(--bg-secondary);
}

.detail-header {
  height: 32px;
  padding: 0 12px;
  background: var(--bg-tertiary);
  border-bottom: 1px solid var(--border-color);
}

.detail-tabs {
  background: rgba(0, 0, 0, 0.1);
  border-radius: 4px;
  padding: 2px;
}

.detail-tab {
  background: transparent;
  border: none;
  color: var(--text-secondary);
  font-size: 0.7rem;
  padding: 2px 8px;
  border-radius: 3px;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 4px;
}

.detail-tab:hover {
  background: rgba(255, 255, 255, 0.05);
  color: var(--text-primary);
}

.detail-tab.active {
  background: var(--bg-secondary);
  color: var(--accent-primary);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

.results-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  overflow: hidden;
  background: var(--bg-primary);
}

.scroll-container {
  flex: 1;
  overflow: auto;
  width: 100%;
  position: relative;
}

.data-grid {
  width: 100%;
  border-collapse: separate;
  border-spacing: 0;
  font-size: 0.75rem;
  table-layout: auto;
}

.data-grid th {
  position: sticky;
  top: 0;
  background: var(--bg-tertiary);
  padding: 6px 12px;
  text-align: left;
  font-weight: 600;
  border-bottom: 2px solid var(--border-color);
  border-right: 1px solid var(--border-color);
  white-space: nowrap;
  /* Sticky-top header layer — above body data and the sticky-left action column. */
  z-index: 20;
}

/* Fixed row height keeps virtual scrolling math accurate. */
.data-grid tbody tr:not(.spacer-row) {
  height: 26px;
}

.data-grid tr.spacer-row td {
  padding: 0;
  border: none;
  background: transparent;
}

.detail-title {
  font-size: 0.7rem;
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.detail-textarea {
  flex: 1;
  width: 100%;
  height: 100%;
  background: transparent;
  border: none;
  color: var(--text-primary);
  padding: 8px;
  font-family: var(--font-mono);
  font-size: 0.8rem;
  resize: none;
  outline: none;
}

.monaco-detail-wrapper {
  width: 100%;
  height: 100%;
}

.html-detail-view {
  width: 100%;
  height: 100%;
  padding: 12px;
  overflow: auto;
  background: white;
  color: black;
  font-family: system-ui, -apple-system, sans-serif;
}

.data-grid td {
  padding: 0;
  border-bottom: 1px solid var(--border-color);
  border-right: 1px solid var(--border-color);
  cursor: default;
  white-space: nowrap;
}

.data-grid td:not(.action-col) {
  overflow: hidden;
}

.data-grid td.selected {
  background: rgba(var(--accent-primary-rgb, 0, 102, 255), 0.1);
  box-shadow: inset 0 0 0 1px var(--accent-primary);
}

.data-grid td.is-null .cell-text {
  color: var(--text-secondary);
  opacity: 0.5;
  font-style: italic;
}

.data-grid td.editing .cell-wrapper {
  padding: 0 !important;
}

.cell-wrapper {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 2px 8px;
  min-width: 50px;
  max-width: 550px;
  gap: 8px;
  min-height: 22px;
}

.cell-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  font-size: 0.72rem;
}

  .cell-input {
    width: 100%;
    height: 100%;
    background: transparent;
    border: none;
    color: var(--text-primary);
    padding: 0 4px;
    font-size: 0.75rem;
    font-family: inherit;
    outline: none;
    border-radius: 0;
  }

.data-grid tr:hover {
  background: var(--glass-border);
}

.result-footer {
  height: 28px;
  padding: 0 12px;
  font-size: 0.75rem;
  color: var(--text-secondary);
  border-top: 1px solid var(--border-color);
  background: var(--bg-tertiary);
  flex-shrink: 0;
  z-index: 30;
}

.text-btn {
  background: transparent;
  border: 1px solid var(--border-color);
  color: var(--text-secondary);
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 0.7rem;
  cursor: pointer;
  transition: all 0.2s;
}

.text-btn:hover {
  background: var(--glass-border);
  color: var(--text-primary);
}

.text-btn.active {
  background: var(--accent-primary);
  color: white;
  border-color: var(--accent-primary);
}
.stats {
  white-space: nowrap;
}

.pagination-controls {
  gap: 4px;
}

  .pagination-controls .icon-btn {
    padding: 2px;
  }

  .cell-editor-with-picker {
    display: flex;
    align-items: stretch;
    width: 100%;
    height: 22px; /* Explicit height to match cell-wrapper */
    position: relative;
    border: 1px solid var(--accent-primary);
    background: var(--bg-secondary);
  }

  .cell-editor-with-picker .cell-input {
    flex: 1;
    min-width: 0;
  }

  .picker-trigger {
    background: transparent;
    border: none;
    border-right: 1px solid var(--border-color);
    height: 100%;
    width: 22px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    color: var(--text-secondary);
    border-radius: 0;
  }

  .picker-trigger:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .hidden-picker {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    opacity: 0;
    pointer-events: none;
    z-index: -1;
  }

  .is-date .cell-text {
    padding-left: 20px;
  }

  .picker-trigger.left {
    border-right: 1px solid var(--border-color);
    border-left: none;
  }

.page-info {
  padding: 0 8px;
  min-width: 60px;
  text-align: center;
}

.icon-btn.xs {
  padding: 2px;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  border-radius: 4px;
  display: flex;
}

.icon-btn.xs:hover {
  background: var(--glass-border);
  color: var(--text-primary);
}

.icon-btn.xs:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.pending-row td {
  background: rgba(var(--accent-primary-rgb), 0.05);
}

.icon-btn-sm {
  background: transparent;
  border: none;
  color: inherit;
  cursor: pointer;
  padding: 2px;
  border-radius: 3px;
  display: flex;
}

.icon-btn-sm:hover {
  background: rgba(255, 255, 255, 0.1);
}

.action-col {
  width: 40px;
  min-width: 40px;
  max-width: 40px;
  text-align: center;
  padding: 4px !important;
}

/* Sticky-left action column body cells — above body data, below sticky header. */
.action-col.sticky {
  position: sticky;
  left: 0;
  background: var(--bg-secondary);
  border-right: 1px solid var(--border-color);
  z-index: 10;
}

/* Corner cell (sticky top + left) — must sit above both the header row and the
   sticky-left body action column. Otherwise the body cell scrolls underneath the
   header but covers the header text on intersection. */
.data-grid th.action-col.sticky {
  background: var(--bg-tertiary);
  z-index: 30;
}

.data-grid tr:hover td.action-col.sticky {
  background: var(--glass-border);
}



/* CSS Cleaned up */
.flex-center { display: flex; align-items: center; justify-content: center; }
.flex-between { display: flex; align-items: center; justify-content: space-between; }
.flex-col { flex-direction: column; }
.gap-1 { gap: 4px; }
.gap-2 { gap: 8px; }
.gap-3 { gap: 12px; }
.gap-4 { gap: 16px; }
.text-success { color: var(--text-success); }
.text-error { color: var(--text-error); }
.text-secondary { color: var(--text-secondary); }
.italic { font-style: italic; }
.w-full { width: 100%; }
.mr-2 { margin-right: 8px; }
.text-9px { font-size: 9px; }
.leading-tight { line-height: 1.25; }
.leading-none { line-height: 1; }
.opacity-20 { opacity: 0.2; }
.opacity-70 { opacity: 0.7; }
.font-bold { font-weight: 700; }
</style>
