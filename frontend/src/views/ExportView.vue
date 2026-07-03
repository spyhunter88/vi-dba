<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { useRoute } from 'vue-router';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  Download, FileSpreadsheet, FileCode, FileText,
  ChevronRight, ChevronLeft, Check, Loader2,
  Table as TableIcon
} from 'lucide-vue-next';

const route = useRoute();
const step = ref(1);

const connectionId = ref(route.query.connectionId as string);
const sourceType = ref(route.query.sourceType as string); // "table", "query", "current", or "multi"
const sourceName = ref(route.query.sourceName as string || '');
const query = ref(route.query.query as string || '');
const isCurrent = ref(route.query.isCurrent === 'true');
const sourceTables = ref<string[]>(
  route.query.sourceTables
    ? (route.query.sourceTables as string).split(',').map(t => decodeURIComponent(t))
    : []
);

const outputFormat = ref(sourceType.value === 'multi' ? 'excel' : 'csv');
const outputPath = ref('');
const selectedColumns = ref<string[]>([]);
const availableColumns = ref<string[]>([]);
const loadingColumns = ref(false);

const isWorking = ref(false);
const error = ref('');
const finished = ref(false);

const isMulti = computed(() => sourceType.value === 'multi');
const hasStep2 = computed(() => sourceType.value === 'table' || isCurrent.value || isMulti.value);

async function fetchColumns() {
  loadingColumns.value = true;
  try {
    if (isCurrent.value) {
      const cols: string[] | null = await invoke('get_stashed_export_info');
      availableColumns.value = cols || [];
      selectedColumns.value = [...availableColumns.value];
    } else if (sourceType.value === 'table' && sourceName.value) {
      const def: any = await invoke('get_table_definition', {
        id: connectionId.value,
        tableName: sourceName.value
      });
      availableColumns.value = def.columns.map((c: any) => c.name);
      selectedColumns.value = [...availableColumns.value];
    }
  } catch (e) {
    console.error('Failed to fetch columns:', e);
  } finally {
    loadingColumns.value = false;
  }
}

async function browseSavePath() {
  const extension = outputFormat.value === 'csv' ? 'csv' : (outputFormat.value === 'excel' ? 'xlsx' : 'sql');
  const selected = await save({
    filters: [{ name: outputFormat.value.toUpperCase(), extensions: [extension] }],
    defaultPath: sourceName.value ? `${sourceName.value}.${extension}` : `export.${extension}`
  });
  if (selected) {
    outputPath.value = selected;
  }
}

async function nextStep() {
  if (step.value === 1) {
    if (!outputPath.value) {
       error.value = "Please select a destination path.";
       return;
    }
    error.value = "";
    if (isMulti.value) {
      step.value = 2;
    } else if (sourceType.value === 'table' || isCurrent.value) {
       await fetchColumns();
       step.value = 2;
    } else {
       step.value = 3;
    }
  } else if (step.value === 2) {
    step.value = 3;
  }
}

async function startExport() {
  isWorking.value = true;
  error.value = '';
  try {
    await invoke('perform_export', {
      config: {
        connectionId: connectionId.value,
        sourceType: sourceType.value,
        sourceName: sourceName.value || null,
        query: query.value || null,
        outputFormat: outputFormat.value,
        outputPath: outputPath.value,
        columns: selectedColumns.value.length > 0 ? selectedColumns.value : null,
        sourceTables: sourceTables.value.length > 0 ? sourceTables.value : null,
        data: null
      }
    });
    finished.value = true;
  } catch (e: any) {
    error.value = e.toString();
  } finally {
    isWorking.value = false;
  }
}

function toggleColumn(col: string) {
  const idx = selectedColumns.value.indexOf(col);
  if (idx > -1) {
    selectedColumns.value.splice(idx, 1);
  } else {
    selectedColumns.value.push(col);
  }
}

onMounted(() => {
  if (sourceType.value === 'table' || isCurrent.value) fetchColumns();
});
</script>

<template>
  <div class="export-wizard glass">
    <div class="wizard-header">
      <div class="steps">
        <div class="step" :class="{ active: step === 1, completed: step > 1 }">
          <div class="step-num">1</div>
          <span>Format & Path</span>
        </div>
        <div class="step-divider" v-if="hasStep2"></div>
        <div class="step" v-if="hasStep2" :class="{ active: step === 2, completed: step > 2 }">
          <div class="step-num">2</div>
          <span>{{ isMulti ? 'Tables' : 'Columns' }}</span>
        </div>
        <div class="step-divider"></div>
        <div class="step" :class="{ active: step === 3, completed: step > 3 }">
          <div class="step-num">{{ hasStep2 ? '3' : '2' }}</div>
          <span>Execution</span>
        </div>
      </div>
    </div>

    <div class="wizard-content">
      <!-- Step 1: Format -->
      <div v-if="step === 1" class="step-pane anim-fade-in">
        <h3 class="mb-4">Select Output Format</h3>

        <div v-if="isMulti" class="multi-notice mb-4">
          <FileSpreadsheet :size="16" class="text-accent" />
          <span>Multi-table export is only available in Excel format. Each table will be saved as a separate sheet.</span>
        </div>

        <div class="format-grid">
           <div v-if="!isMulti" class="format-card" :class="{ active: outputFormat === 'csv' }" @click="outputFormat = 'csv'">
              <div class="format-icon"><FileText /></div>
              <div class="format-info">
                 <div class="format-name">CSV</div>
                 <div class="format-desc">Comma Separated Values</div>
              </div>
           </div>
           <div class="format-card" :class="{ active: outputFormat === 'excel', locked: isMulti }" @click="!isMulti && (outputFormat = 'excel')">
              <div class="format-icon"><FileSpreadsheet /></div>
              <div class="format-info">
                 <div class="format-name">Excel</div>
                 <div class="format-desc">{{ isMulti ? 'Each table = one sheet (.xlsx)' : 'Microsoft Excel (.xlsx)' }}</div>
              </div>
           </div>
           <div v-if="!isMulti" class="format-card" :class="{ active: outputFormat === 'sql' }" @click="outputFormat = 'sql'">
              <div class="format-icon"><FileCode /></div>
              <div class="format-info">
                 <div class="format-name">SQL</div>
                 <div class="format-desc">INSERT Statements</div>
              </div>
           </div>
        </div>

        <div class="form-group mt-8">
           <label>Save To</label>
           <div class="flex gap-2">
              <input v-model="outputPath" readonly placeholder="Select destination..." class="input-primary flex-1" @click="browseSavePath" />
              <button class="button-secondary" @click="browseSavePath">Browse</button>
           </div>
           <p v-if="error" class="text-xs text-error mt-1">{{ error }}</p>
        </div>
      </div>

      <!-- Step 2: Tables (multi-table export) -->
      <div v-if="step === 2 && isMulti" class="step-pane anim-fade-in">
         <div class="flex-between mb-4">
            <h3 class="m-0">Tables to Export</h3>
            <span class="text-xs text-secondary">Each table will become a separate sheet</span>
         </div>
         <div class="column-list scrollbar-thin">
            <div v-for="table in sourceTables" :key="table" class="column-item">
               <TableIcon :size="14" class="opacity-40" />
               <span>{{ table }}</span>
            </div>
         </div>
         <p class="text-xs text-secondary mt-4">{{ sourceTables.length }} table{{ sourceTables.length !== 1 ? 's' : '' }} will be exported to <span class="font-mono">{{ outputPath }}</span></p>
      </div>

      <!-- Step 2: Columns (single-table export) -->
      <div v-if="step === 2 && !isMulti" class="step-pane anim-fade-in">
         <div class="flex-between mb-4">
            <h3 class="m-0">Select Columns</h3>
            <div class="flex gap-2">
               <button class="button-secondary py-1 px-3 text-xs" @click="selectedColumns = [...availableColumns]">Select All</button>
               <button class="button-secondary py-1 px-3 text-xs" @click="selectedColumns = []">Clear</button>
            </div>
         </div>

         <div v-if="loadingColumns" class="flex-center p-8">
            <Loader2 class="spin" />
         </div>
         <div v-else class="column-list scrollbar-thin">
            <label v-for="col in availableColumns" :key="col" class="column-item">
               <input type="checkbox" :checked="selectedColumns.includes(col)" @change="toggleColumn(col)" />
               <TableIcon :size="14" class="opacity-40" />
               <span>{{ col }}</span>
            </label>
         </div>
      </div>

      <!-- Step 3: Finish/Confirm -->
      <div v-if="step === 3" class="step-pane anim-fade-in flex-center flex-col text-center">
         <template v-if="finished">
            <div class="success-circle mb-4">
               <Check :size="48" class="text-success" />
            </div>
            <h2>Export Completed</h2>
            <p class="text-secondary mt-2">Data exported successfully to:<br/><span class="text-xs font-mono">{{ outputPath }}</span></p>
         </template>
         <template v-else>
            <Download :size="48" class="text-accent mb-4 opacity-50" />
            <h2>Ready to Export</h2>
            <p class="text-secondary mt-2">
              Source: <span class="text-primary font-bold">
                {{ isMulti
                  ? `${sourceTables.length} table${sourceTables.length !== 1 ? 's' : ''}`
                  : isCurrent ? 'Current Result Set'
                  : sourceType === 'table' ? sourceName
                  : 'Custom Query' }}
              </span>
            </p>
            <p class="text-secondary">Format: <span class="text-primary font-bold">{{ outputFormat.toUpperCase() }}</span></p>
            <p class="text-secondary">Destination: <span class="text-primary text-xs font-mono">{{ outputPath }}</span></p>

            <div v-if="error" class="error-msg mt-4 p-3 bg-error bg-opacity-10 border border-error rounded text-error text-xs">
               {{ error }}
            </div>
         </template>
      </div>
    </div>

    <div class="wizard-footer">
      <div class="flex-between w-full">
        <div>
           <button v-if="step > 1 && !finished" class="button-secondary" @click="step--">
              <ChevronLeft :size="16" /> Back
           </button>
        </div>
        <div class="flex gap-3">
           <button v-if="step < 3" class="button-primary" @click="nextStep">
              Next <ChevronRight :size="16" />
           </button>
           <button v-if="step === 3 && !finished" class="button-primary" :disabled="isWorking" @click="startExport">
              <Loader2 v-if="isWorking" class="spin mr-2" :size="16" />
              {{ isWorking ? 'Exporting...' : 'Start Export' }}
           </button>
           <button v-if="finished" class="button-primary" @click="() => getCurrentWindow().close()">
              Close
           </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.export-wizard {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--bg-primary);
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
  max-width: 500px;
  margin: 0 auto;
}

.multi-notice {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  background: rgba(59, 130, 246, 0.08);
  border: 1px solid rgba(59, 130, 246, 0.25);
  border-radius: 8px;
  font-size: 0.82rem;
  color: var(--text-secondary);
}

.format-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 12px;
}

.format-card {
  display: flex;
  align-items: center;
  gap: 15px;
  padding: 15px;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.2s;
}

.format-card:hover:not(.locked) { border-color: var(--accent-primary); background: var(--glass-border); }
.format-card.active { border-color: var(--accent-primary); background: rgba(59, 130, 246, 0.1); }
.format-card.locked { cursor: default; opacity: 0.9; }

.format-icon {
  width: 40px;
  height: 40px;
  border-radius: 8px;
  background: var(--bg-tertiary);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--accent-primary);
}

.format-name { font-weight: 600; font-size: 0.9rem; }
.format-desc { font-size: 0.75rem; color: var(--text-secondary); }

.column-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 300px;
  overflow-y: auto;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background: var(--bg-secondary);
}

.column-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  cursor: pointer;
  font-size: 0.85rem;
  transition: background 0.2s;
}

.column-item:hover { background: var(--glass-border); }
.column-item input { cursor: pointer; }

.wizard-footer {
  padding: 20px 40px;
  background: var(--bg-tertiary);
  border-top: 1px solid var(--border-color);
}

.success-circle {
  width: 80px; height: 80px;
  border-radius: 50%;
  background: rgba(16, 185, 129, 0.1);
  display: flex; align-items: center; justify-content: center;
  margin: 0 auto;
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
