<script setup lang="ts">
import { ref, onMounted, computed, watch, onUnmounted, nextTick } from 'vue';
import { useRoute } from 'vue-router';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  Check, Search, Table,
  ChevronRight, ChevronLeft, Loader2,
  ChevronDown, FileSpreadsheet
} from 'lucide-vue-next';

const route = useRoute();
const step = ref(1);

const connectionId = ref(route.query.connectionId as string);
const catalog = ref(route.query.catalog as string || null);
const schema = ref(route.query.schema as string || null);
const targetTable = ref(route.query.tableName as string || '');

const filePath = ref('');
const filePreview = ref<any>(null);
const loadingPreview = ref(false);

const sheetName = ref('');
const hasHeader = ref(true);
const delimiter = ref(',');

const columnMappings = ref<Record<string, string>>({});
const columnTypes = ref<Record<string, string>>({});
const targetColumns = ref<any[]>([]);
const existingTables = ref<string[]>([]);

// Multi-sheet import
const multiSheetMode = ref(false);
const sheetTargets = ref<Record<string, string>>({});

// Dropdown state
const showTableDropdown = ref(false);
const tableSearch = ref('');
const tableDropdownRef = ref<HTMLElement | null>(null);
const logContainerRef = ref<HTMLElement | null>(null);

const commonDataTypes = ['TEXT', 'INTEGER', 'REAL', 'BLOB', 'VARCHAR(255)', 'INT', 'DECIMAL', 'BOOLEAN', 'TIMESTAMP'];

const filteredTables = computed(() => {
  if (!tableSearch.value) return existingTables.value;
  return existingTables.value.filter(t => t.toLowerCase().includes(tableSearch.value.toLowerCase()));
});

const isExistingTable = computed(() => {
  return existingTables.value.includes(targetTable.value);
});

const progress = ref({
  total: 0,
  current: 0,
  success: 0,
  error: 0,
  message: '',
  logs: [] as string[],
  finished: false
});

const isWorking = ref(false);

async function fetchTables() {
  if (!connectionId.value) return;
  try {
    const res: any = await invoke('get_table_list', {
      id: connectionId.value,
      catalog: catalog.value,
      schema: schema.value
    });
    if (res && res.rows) {
      existingTables.value = res.rows.map((row: any) => {
        return row.Name || row.name || Object.values(row)[0]?.toString() || '';
      }).filter(Boolean);
    }
  } catch (e) {
    console.error('Failed to fetch tables:', e);
  }
}

async function browseFile() {
  const selected = await open({
    multiple: false,
    filters: [{ name: 'Data Files', extensions: ['csv', 'xlsx', 'xls', 'ods'] }]
  });
  if (selected && typeof selected === 'string') {
    filePath.value = selected;
    loadPreview();
  }
}

async function loadPreview() {
  if (!filePath.value) return;
  loadingPreview.value = true;
  try {
    filePreview.value = await invoke('get_file_preview', { 
      path: filePath.value, 
      sheetName: sheetName.value || null,
      delimiter: delimiter.value
    });
    
    // Auto map columns
    columnMappings.value = {};
    columnTypes.value = {};
    if (filePreview.value && filePreview.value.columns) {
      filePreview.value.columns.forEach((sourceCol: string, index: number) => {
        const colId = hasHeader.value ? sourceCol : index.toString();
        // Smart type detection for "id" column
        let detectedType = 'TEXT';
        if (sourceCol.toLowerCase() === 'id' && filePreview.value?.rows) {
          const sampleValues = filePreview.value.rows.map((row: any) => row[sourceCol]);
          const isNumeric = sampleValues.length > 0 && sampleValues.every((val: any) => {
            if (val === null || val === undefined || val === '') return true;
            const strVal = String(val).trim();
            return strVal !== '' && !isNaN(Number(strVal));
          });
          if (isNumeric) detectedType = 'INT';
        }

        if (isExistingTable.value) {
            const match = targetColumns.value.find(tc => tc.name.toLowerCase() === sourceCol.toLowerCase());
            columnMappings.value[colId] = match ? match.name : '';
            columnTypes.value[colId] = match ? match.dataType : detectedType;
        } else {
            // For new table, default to source column name (sanitized)
            columnMappings.value[colId] = sourceCol.replace(/[^a-zA-Z0-9_]/g, '_');
            columnTypes.value[colId] = detectedType;
        }
      });
    }

    if (filePreview.value?.sheets) {
      if (!sheetName.value) sheetName.value = filePreview.value.sheets[0];
      // Auto-enable multi-sheet mode when Excel has multiple sheets
      if (filePreview.value.sheets.length > 1) {
        multiSheetMode.value = true;
        const newTargets: Record<string, string> = {};
        filePreview.value.sheets.forEach((s: string) => {
          newTargets[s] = sheetTargets.value[s] ?? s.replace(/[^a-zA-Z0-9_]/g, '_').toLowerCase();
        });
        sheetTargets.value = newTargets;
      } else {
        multiSheetMode.value = false;
      }
    } else {
      multiSheetMode.value = false;
    }
  } catch (e) {
    console.error('Failed to load preview:', e);
  } finally {
    loadingPreview.value = false;
  }
}

async function fetchTargetColumns() {
  if (!isExistingTable.value || !targetTable.value || !connectionId.value) {
    targetColumns.value = [];
    return;
  }
  try {
    const def = await invoke('get_table_definition', {
      id: connectionId.value,
      tableName: targetTable.value,
      catalog: catalog.value,
      schema: schema.value
    });
    targetColumns.value = (def as any).columns;
  } catch (e) {
    console.error('Failed to fetch table definition:', e);
    targetColumns.value = [];
  }
}

function selectDropdownTable(t: string) {
  targetTable.value = t;
  showTableDropdown.value = false;
  tableSearch.value = '';
}

watch(targetTable, async () => {
    await fetchTargetColumns();
    // Re-run mapping logic when table changes
    if (filePreview.value) loadPreview();
});

watch([sheetName, hasHeader, delimiter], loadPreview);

async function nextStep() {
  if (step.value === 1) {
    if (!filePath.value) return;
    if (multiSheetMode.value) {
      // Skip column mapping for multi-sheet; go straight to execution
      step.value = 3;
      startImport();
    } else {
      if (!targetTable.value) return;
      await fetchTargetColumns();
      step.value = 2;
    }
  } else if (step.value === 2) {
    step.value = 3;
    startImport();
  }
}

async function startImport() {
  isWorking.value = true;
  progress.value.finished = false;
  progress.value.logs = [];
  progress.value.success = 0;
  progress.value.error = 0;
  progress.value.current = 0;

  const unlistenPromise = listen('import-progress', (event: any) => {
    const data = event.payload;
    progress.value.current = data.current;
    progress.value.total = data.total;
    progress.value.success = data.successCount;
    progress.value.error = data.errorCount;
    progress.value.message = data.message;
    progress.value.logs = (data.logs || []).slice();

    if (data.isFinished) {
      unlistenPromise.then(fn => fn());
      progress.value.finished = true;
      isWorking.value = false;
      step.value = 4;
    }

    nextTick(() => {
      if (logContainerRef.value) {
        logContainerRef.value.scrollTop = logContainerRef.value.scrollHeight;
      }
    });
  });

  try {
    if (multiSheetMode.value) {
      const sheetMappings = Object.entries(sheetTargets.value)
        .filter(([, t]) => t.trim() !== '')
        .map(([sheet, table]) => ({ sheetName: sheet, targetTable: table }));
      await invoke('perform_multi_import', {
        config: {
          connectionId: connectionId.value,
          filePath: filePath.value,
          catalog: catalog.value,
          schema: schema.value,
          sheetMappings,
          hasHeader: true
        }
      });
    } else {
      await invoke('perform_import', {
        config: {
          connectionId: connectionId.value,
          filePath: filePath.value,
          targetTable: targetTable.value,
          catalog: catalog.value,
          schema: schema.value,
          columnMappings: columnMappings.value,
          columnTypes: columnTypes.value,
          sheetName: sheetName.value || null,
          hasHeader: hasHeader.value,
          delimiter: delimiter.value
        }
      });
    }
  } catch (e) {
    unlistenPromise.then(fn => fn());
    progress.value.message = 'Error: ' + e;
    progress.value.logs.push('Fatal Error: ' + e);
    isWorking.value = false;
  }
}

function goBack() {
  // In multi-sheet mode, step jumps 1→3, so going back from 3 returns to 1
  if (step.value === 3 && multiSheetMode.value) {
    step.value = 1;
  } else {
    step.value--;
  }
}

const handleClickOutside = (e: MouseEvent) => {
  if (tableDropdownRef.value && !tableDropdownRef.value.contains(e.target as Node)) {
    showTableDropdown.value = false;
  }
};

onMounted(() => {
  fetchTables();
  if (targetTable.value) fetchTargetColumns();
  document.addEventListener('mousedown', handleClickOutside);
});

onUnmounted(() => {
  document.removeEventListener('mousedown', handleClickOutside);
});

const progressPercent = computed(() => {
  if (progress.value.total === 0) return 0;
  return Math.round((progress.value.current / progress.value.total) * 100);
});
</script>

<template>
  <div class="import-wizard glass">
    <div class="wizard-header">
      <div class="steps">
        <div class="step" :class="{ active: step === 1, completed: step > 1 }">
          <div class="step-num">1</div>
          <span>Source & Target</span>
        </div>
        <div class="step-divider"></div>
        <div class="step" :class="{ active: step === 2, completed: step > 2 }">
          <div class="step-num">2</div>
          <span>Mapping</span>
        </div>
        <div class="step-divider"></div>
        <div class="step" :class="{ active: step === 3, completed: step > 3 }">
          <div class="step-num">3</div>
          <span>Execution</span>
        </div>
        <div class="step-divider"></div>
        <div class="step" :class="{ active: step === 4, completed: step > 4 }">
          <div class="step-num">4</div>
          <span>Finish</span>
        </div>
      </div>
    </div>

    <div class="wizard-content scrollbar-thin">
      <!-- Step 1: File selection & Target -->
      <div v-if="step === 1" class="step-pane anim-fade-in">
        <h3>Select Source File</h3>
        <div class="form-group mb-4">
          <label>File Path</label>
          <div class="flex gap-2">
            <input v-model="filePath" placeholder="Click to browse..." readonly class="input-primary flex-1" @click="browseFile" />
            <button class="button-secondary" @click="browseFile">Browse</button>
          </div>
        </div>

        <!-- Multi-sheet mapping (shown when Excel with multiple sheets) -->
        <div v-if="multiSheetMode && filePreview?.sheets">
          <div class="flex-between mb-3">
            <h3 class="m-0">Sheet → Table Mapping</h3>
            <label class="flex items-center gap-2 text-xs cursor-pointer opacity-70">
              <input type="checkbox" v-model="multiSheetMode" /> Multi-sheet mode
            </label>
          </div>
          <div class="multi-notice mb-3">
            <FileSpreadsheet :size="14" class="text-accent" />
            <span>Each sheet will be imported into its own table. Tables will be created automatically if they don't exist.</span>
          </div>
          <div class="sheet-mapping-list scrollbar-thin">
            <div v-for="sheet in filePreview.sheets" :key="sheet" class="sheet-mapping-row">
              <span class="sheet-label">
                <FileSpreadsheet :size="13" class="opacity-50" />
                {{ sheet }}
              </span>
              <ChevronRight :size="14" class="opacity-30 flex-shrink-0" />
              <input
                v-model="sheetTargets[sheet]"
                :placeholder="`table name...`"
                class="input-primary text-xs flex-1"
                :list="`tables-for-${sheet}`"
              />
              <datalist :id="`tables-for-${sheet}`">
                <option v-for="t in existingTables" :key="t" :value="t" />
              </datalist>
            </div>
          </div>
        </div>

        <!-- Single-target table (shown when single sheet or CSV) -->
        <div v-else>
          <h3>Target Table</h3>
          <div class="form-group">
            <label>Table Name</label>
            <div class="custom-select" ref="tableDropdownRef">
               <div class="select-box" @click="showTableDropdown = !showTableDropdown">
                  <input
                    v-model="targetTable"
                    placeholder="Enter or select table..."
                    class="select-input"
                    @focus="showTableDropdown = true"
                  />
                  <ChevronDown :size="16" class="select-arrow" />
               </div>
               <div v-if="showTableDropdown" class="select-dropdown glass">
                  <div class="search-box">
                     <Search :size="14" />
                     <input v-model="tableSearch" placeholder="Search tables..." class="search-input" ref="dropdownSearch" />
                  </div>
                  <div class="options-list scrollbar-thin">
                     <div v-if="targetTable && !existingTables.includes(targetTable)" class="option-item new-option" @click="showTableDropdown = false">
                        <Table :size="14" class="text-accent" />
                        <span>Create: <strong>{{ targetTable }}</strong></span>
                     </div>
                     <div v-for="t in filteredTables" :key="t" class="option-item" @click="selectDropdownTable(t)">
                        <Table :size="14" />
                        <span>{{ t }}</span>
                     </div>
                     <div v-if="filteredTables.length === 0" class="no-options">No tables found</div>
                  </div>
               </div>
            </div>
            <p class="text-xs mt-2" :class="isExistingTable ? 'text-success' : 'text-warning'">
               {{ isExistingTable ? 'Targeting existing table: ' + targetTable : 'Targeting new table: ' + targetTable }}
            </p>
            <p class="text-xs text-secondary mt-1">If the table doesn't exist, it will be automatically created using the mapping from Step 2.</p>
          </div>
        </div>
      </div>

      <!-- Step 2: Mapping -->
      <div v-if="step === 2" class="step-pane anim-fade-in">
        <div class="flex-between mb-4">
          <h3 class="m-0">Column Mapping</h3>
          <div v-if="filePreview?.sheets" class="flex items-center gap-2">
            <label class="text-xs">Sheet:</label>
            <select v-model="sheetName" class="input-primary py-1 px-2 text-xs">
              <option v-for="s in filePreview.sheets" :key="s" :value="s">{{ s }}</option>
            </select>
          </div>
          <div v-if="!filePreview?.sheets" class="flex items-center gap-4">
            <label class="flex items-center gap-2 text-xs cursor-pointer">
              <input type="checkbox" v-model="hasHeader" /> Has Header
            </label>
            <div class="flex items-center gap-2">
               <label class="text-xs">Delimiter:</label>
               <input v-model="delimiter" class="input-primary py-1 px-2 text-xs w-10 text-center" maxlength="1" />
            </div>
          </div>
        </div>

        <div v-if="loadingPreview" class="flex-center p-8">
          <Loader2 class="spin" />
        </div>
        <div v-else class="mapping-table-container scrollbar-thin">
          <table class="mapping-table">
            <thead>
              <tr>
                <th>Source Column</th>
                <th>Sample Value</th>
                <th class="arrow-col"></th>
                <th>Target Column</th>
                <th>Data Type</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(col, idx) in filePreview?.columns" :key="idx">
                <td>
                  <span class="font-mono text-xs">{{ hasHeader ? col : 'Col ' + idx }}</span>
                </td>
                <td>
                  <span class="text-xs text-secondary truncate inline-block max-w-[150px]" :title="filePreview?.rows[0]?.[idx]">
                    {{ filePreview?.rows[0]?.[idx] || 'NULL' }}
                  </span>
                </td>
                <td class="arrow-col"><ChevronRight :size="14" class="opacity-30" /></td>
                <td>
                  <select v-if="isExistingTable" v-model="columnMappings[hasHeader ? col : idx]" class="input-primary w-full text-xs">
                    <option value="">(Ignore)</option>
                    <option v-for="tc in targetColumns" :key="tc.name" :value="tc.name">
                      {{ tc.name }}
                    </option>
                  </select>
                  <input v-else v-model="columnMappings[hasHeader ? col : idx]" class="input-primary w-full text-xs" placeholder="New column name..." />
                </td>
                <td>
                   <div class="flex gap-1">
                      <input 
                        v-model="columnTypes[hasHeader ? col : idx]" 
                        class="input-primary w-full text-xs" 
                        placeholder="Type..."
                        list="datatypes-list"
                        :disabled="isExistingTable && columnMappings[hasHeader ? col : idx] !== ''"
                      />
                      <datalist id="datatypes-list">
                         <option v-for="dt in commonDataTypes" :key="dt" :value="dt" />
                      </datalist>
                   </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <!-- Step 3: Execution -->
      <div v-if="step === 3" class="step-pane anim-fade-in">
        <div class="progress-section mb-6">
           <div class="flex-between mb-2">
              <span class="text-sm font-medium">{{ progress.message }}</span>
              <span class="text-sm font-bold">{{ progressPercent }}%</span>
           </div>
           <div class="progress-bar-bg">
              <div class="progress-bar-fill" :style="{ width: progressPercent + '%' }"></div>
           </div>
           
           <div class="stats-grid mt-6">
              <div class="stat-item">
                 <div class="stat-label">Total Rows</div>
                 <div class="stat-value">{{ progress.total }}</div>
              </div>
              <div class="stat-item">
                 <div class="stat-label">Imported</div>
                 <div class="stat-value text-success">{{ progress.success }}</div>
              </div>
              <div class="stat-item">
                 <div class="stat-label">Errors</div>
                 <div class="stat-value text-error">{{ progress.error }}</div>
              </div>
           </div>
        </div>

        <h3>Execution Log</h3>
        <div class="log-container scrollbar-thin" ref="logContainerRef">
           <div v-for="(log, idx) in progress.logs" :key="idx" class="log-line" :class="{ 'log-error': log.startsWith('Row') || log.startsWith('Failed') || log.startsWith('Fatal') }">
              <span class="log-idx">[{{ idx + 1 }}]</span> {{ log }}
           </div>
           <div v-if="progress.logs.length === 0" class="text-secondary text-xs p-2 italic">Waiting for logs...</div>
        </div>
      </div>

      <!-- Step 4: Completion -->
      <div v-if="step === 4" class="step-pane anim-fade-in flex-center flex-col">
          <div class="success-circle mb-4">
             <Check :size="48" class="text-success" />
          </div>
          <h2>Import Finished</h2>
          <p class="text-secondary mt-2">Process completed for {{ progress.total }} rows.</p>
          
          <div class="stats-grid mt-8">
              <div class="stat-item">
                 <div class="stat-label">Successfully Imported</div>
                 <div class="stat-value text-success">{{ progress.success }}</div>
              </div>
              <div class="stat-item">
                 <div class="stat-label">Failed Rows</div>
                 <div class="stat-value text-error">{{ progress.error }}</div>
              </div>
           </div>

           <div class="mt-8 text-center">
              <p v-if="progress.error === 0" class="text-success font-medium">All rows imported successfully!</p>
              <p v-else class="text-warning">Imported with {{ progress.error }} errors. Check logs in Step 3 if needed.</p>
           </div>
      </div>
    </div>

    <div class="wizard-footer">
       <div class="flex-between w-full">
          <div>
             <button v-if="step > 1 && step < 4 && !isWorking" class="button-secondary" @click="goBack">
               <ChevronLeft :size="16" /> Back
             </button>
             <button v-if="step === 3 && isWorking" class="button-secondary" @click="isWorking = false; step = 1">
               Stop & Reset
             </button>
          </div>
          <div class="flex gap-3">
             <button
               v-if="step < 3"
               class="button-primary"
               :disabled="step === 1 && (!filePath || (multiSheetMode ? Object.values(sheetTargets).some(t => !t.trim()) : !targetTable))"
               @click="nextStep"
             >
               Next <ChevronRight :size="16" />
             </button>
             <button v-if="step === 3 && !isWorking" class="button-primary" @click="step = 4">
               Continue to Result
             </button>
             <button v-if="step === 4" class="button-primary" @click="() => getCurrentWindow().close()">
                Finish
             </button>
          </div>
       </div>
    </div>
  </div>
</template>

<style scoped>
.import-wizard {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--bg-primary);
  color: var(--text-primary);
}

.wizard-header {
  padding: 20px;
  border-bottom: 1px solid var(--border-color);
  background: var(--bg-tertiary);
}

.steps {
  display: flex;
  justify-content: space-between;
  align-items: center;
  max-width: 600px;
  margin: 0 auto;
}

.step {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  opacity: 0.4;
  transition: all 0.3s;
}

.step.active { opacity: 1; }
.step.completed { opacity: 0.8; color: var(--accent-primary); }

.step-num {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  border: 2px solid currentColor;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: bold;
  font-size: 0.8rem;
}

.step.active .step-num {
  background: var(--accent-primary);
  border-color: var(--accent-primary);
  color: white;
}

.step span { font-size: 0.75rem; font-weight: 600; }

.step-divider {
  flex: 1;
  height: 2px;
  background: var(--border-color);
  margin: 0 15px;
  margin-top: -20px;
}

.wizard-content {
  flex: 1;
  padding: 40px;
  overflow-y: auto;
}

.step-pane {
  max-width: 800px;
  margin: 0 auto;
}

.step-pane h3 {
  margin-bottom: 20px;
  font-size: 1.1rem;
  border-bottom: 1px solid var(--border-color);
  padding-bottom: 10px;
}

/* Custom Select */
.custom-select {
  position: relative;
  width: 100%;
}

.select-box {
  display: flex;
  align-items: center;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  overflow: hidden;
  transition: all 0.2s;
}

.select-box:focus-within {
  border-color: var(--accent-primary);
  box-shadow: 0 0 0 2px var(--accent-shadow);
}

.select-input {
  flex: 1;
  background: transparent;
  border: none;
  color: var(--text-primary);
  padding: 10px 12px;
  outline: none;
  font-size: 0.9rem;
}

.select-arrow {
  margin-right: 12px;
  opacity: 0.5;
}

.select-dropdown {
  position: absolute;
  top: calc(100% + 5px);
  left: 0;
  right: 0;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  box-shadow: var(--shadow-xl);
  z-index: 100;
  overflow: hidden;
  animation: slideSelect 0.2s ease-out;
}

@keyframes slideSelect {
  from { opacity: 0; transform: translateY(-5px); }
  to { opacity: 1; transform: translateY(0); }
}

.search-box {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border-color);
  background: var(--bg-secondary);
}

.search-input {
  flex: 1;
  background: transparent;
  border: none;
  color: var(--text-primary);
  outline: none;
  font-size: 0.85rem;
}

.options-list {
  max-height: 250px;
  overflow-y: auto;
}

.option-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  cursor: pointer;
  font-size: 0.85rem;
  transition: background 0.2s;
}

.option-item:hover {
  background: var(--glass-border);
}

.option-item.new-option {
  border-bottom: 1px solid var(--border-color);
  background: rgba(59, 130, 246, 0.05);
}

.no-options {
  padding: 20px;
  text-align: center;
  color: var(--text-secondary);
  font-size: 0.8rem;
}

.wizard-footer {
  padding: 20px 40px;
  background: var(--bg-tertiary);
  border-top: 1px solid var(--border-color);
}

.mapping-table-container {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  max-height: 400px;
  overflow-y: auto;
  background: var(--bg-secondary);
}

.mapping-table {
  width: 100%;
  border-collapse: collapse;
}

.mapping-table th {
  text-align: left;
  padding: 10px;
  font-size: 0.75rem;
  text-transform: uppercase;
  color: var(--text-secondary);
  border-bottom: 1px solid var(--border-color);
  position: sticky;
  top: 0;
  background: var(--bg-tertiary);
  z-index: 10;
}

.mapping-table td {
  padding: 8px 10px;
  border-bottom: 1px solid var(--border-color);
}

.arrow-col { width: 40px; text-align: center; }

/* Log Container */
.log-container {
  background: #000;
  color: #0f0;
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 0.75rem;
  padding: 10px;
  border-radius: 6px;
  height: 250px;
  overflow-y: auto;
  border: 1px solid var(--border-color);
}

.log-line {
  line-height: 1.4;
  white-space: pre-wrap;
  word-break: break-all;
}

.log-idx { color: #666; margin-right: 5px; }
.log-error { color: #ff5555; }

.progress-bar-bg {
  width: 100%;
  height: 8px;
  background: var(--border-color);
  border-radius: 4px;
  overflow: hidden;
}

.progress-bar-fill {
  height: 100%;
  background: var(--accent-primary);
  transition: width 0.3s ease;
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 20px;
  width: 100%;
}

.stat-item {
  background: var(--bg-secondary);
  padding: 15px;
  border-radius: 8px;
  text-align: center;
  border: 1px solid var(--border-color);
}

.stat-label { font-size: 0.7rem; color: var(--text-secondary); text-transform: uppercase; }
.stat-value { font-size: 1.2rem; font-weight: bold; margin-top: 5px; }

.success-circle {
  width: 80px;
  height: 80px;
  border-radius: 50%;
  background: rgba(16, 185, 129, 0.1);
  display: flex;
  align-items: center; justify-content: center;
}

.multi-notice {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: rgba(59, 130, 246, 0.07);
  border: 1px solid rgba(59, 130, 246, 0.2);
  border-radius: 6px;
  font-size: 0.78rem;
  color: var(--text-secondary);
}

.sheet-mapping-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 320px;
  overflow-y: auto;
  padding: 2px 0;
}

.sheet-mapping-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 6px;
}

.sheet-label {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 140px;
  font-size: 0.82rem;
  color: var(--text-secondary);
  font-family: 'Consolas', 'Monaco', monospace;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.anim-fade-in {
  animation: fadeIn 0.3s ease-out;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
}

.spin { animation: spin 1s linear infinite; }
@keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>
