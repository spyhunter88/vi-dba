<script setup lang="ts">
import { ref, onMounted, computed, watch, toRaw } from 'vue';
import { useTabStore } from '../../stores/tabs';
import { useConnectionStore } from '../../stores/connections';
import { useUiStore } from '../../stores/ui';
import { Plus, Trash2, Save, X, ChevronDown, ChevronUp, Database, Key, Link } from 'lucide-vue-next';
import { invoke } from '@tauri-apps/api/core';
import type { TableDefinition, TableColumn } from '../../types';
import ConfirmDialog from '../ui/ConfirmDialog.vue';

interface TableIndex {
  name: string;
  indexType: string;
  method: string;
  columns: string[];
}
interface TableForeignKey {
  name: string;
  columns: string[];
  referencedTable: string;
  referencedColumns: string[];
  onUpdate: string;
  onDelete: string;
}
interface TableIndexInfo {
  indexes: TableIndex[];
  foreignKeys: TableForeignKey[];
}

const props = defineProps<{
  tabId: string;
}>();

const tabStore = useTabStore();
const connectionStore = useConnectionStore();
const uiStore = useUiStore();

const tab = computed(() => tabStore.tabs.find((t: any) => t.id === props.tabId));

const tableName = ref('');
const columns = ref<TableColumn[]>([]);
const catalog = ref<string | undefined>(undefined);
const schema = ref<string | undefined>(undefined);
const tableCollation = ref<string | null>(null);
const loading = ref(false);
const error = ref<string | null>(null);

// Active editor tab
const activeTab = ref<'columns' | 'indexes'>('columns');

// Index/FK data
const indexInfo = ref<TableIndexInfo | null>(null);
const indexLoading = ref(false);

// Remove-column confirm dialog
const removeDialogVisible = ref(false);
const removeTargetIdx = ref<number | null>(null);

// Track which column rows are new or dirty (by index into columns array)
type EditableColumn = TableColumn & { _isNew?: boolean; _isDirty?: boolean };

// Column resizing state
const columnWidths = ref<Record<string, number>>({
  drag: 40,
  name: 200,
  type: 120,
  length: 80,
  nullable: 60,
  pk: 60,
  ai: 60,
  default: 120,
  collation: 160,
  comment: 200,
  actions: 80
});

const resizing = ref<{ key: string; startWidth: number; startX: number } | null>(null);

function startResizing(e: MouseEvent, key: string) {
  resizing.value = {
    key,
    startWidth: columnWidths.value[key] || 100,
    startX: e.pageX
  };
  window.addEventListener('mousemove', handleMouseMove);
  window.addEventListener('mouseup', stopResizing);
  e.preventDefault();
}

function handleMouseMove(e: MouseEvent) {
  if (!resizing.value) return;
  const delta = e.pageX - resizing.value.startX;
  const newWidth = Math.max(40, resizing.value.startWidth + delta);
  columnWidths.value[resizing.value.key] = newWidth;
}

function stopResizing() {
  resizing.value = null;
  window.removeEventListener('mousemove', handleMouseMove);
  window.removeEventListener('mouseup', stopResizing);
}

const originalDefinition = ref<string>('');

const isNew = computed(() => !tab.value?.metadata?.tableName);

const commonTypes = [
  'INT', 'BIGINT', 'VARCHAR', 'TEXT', 'BOOLEAN', 'TIMESTAMP', 
  'DATE', 'CHAR', 'DECIMAL', 'FLOAT', 'DOUBLE', 'JSON', 'BLOB', 'SERIAL', 'BIGSERIAL'
];

const typeDefaults: Record<string, string | null> = {
  'VARCHAR': "255",
  'CHAR': "255",
  'INT': null,
  'INTEGER': null,
  'BIGINT': null,
  'TEXT': null,
  'DECIMAL': "10,2",
  'DATE': null,
  'DATETIME': null,
  'TIMESTAMP': null,
  'BOOLEAN': null,
  'JSON': null,
  'BLOB': null,
  'SERIAL': null,
  'BIGSERIAL': null
};

const previewSql = ref('');
const showPreview = ref(false);
const previewType = ref<'create' | 'alter'>('create');
const previewLoading = ref(false);

async function updatePreviewSql() {
  if (!tab.value || !tableName.value) return;
  
  previewLoading.value = true;
  try {
    const definition: TableDefinition = {
      name: tableName.value,
      columns: columns.value,
      catalog: catalog.value,
      schema: schema.value,
      comment: null
    };

    let oldDef = null;
    if (previewType.value === 'alter' && !isNew.value && originalDefinition.value) {
      oldDef = JSON.parse(originalDefinition.value);
    }

    const sql = await invoke<string>('generate_table_sql', {
      id: tab.value.connectionId,
      oldDefinition: oldDef,
      newDefinition: definition
    });
    previewSql.value = sql;
  } catch (e: any) {
    console.error('Failed to generate preview SQL:', e);
    previewSql.value = `-- Error generating preview: ${e}`;
  } finally {
    previewLoading.value = false;
  }
}

// Watch for changes to update preview
watch([tableName, columns, catalog, schema, previewType], () => {
  if (showPreview.value) {
    updatePreviewSql();
  }
}, { deep: true });

function togglePreview(type: 'create' | 'alter' = 'alter') {
  if (showPreview.value && previewType.value === type) {
    showPreview.value = false;
  } else {
    previewType.value = type;
    showPreview.value = true;
    updatePreviewSql();
  }
}

function onTypeChange(col: TableColumn) {
  const type = col.dataType.toUpperCase();
  if (type in typeDefaults) {
    col.length = typeDefaults[type] ?? null;
  } else {
    // If unknown type, let user decide or clear
    col.length = null;
  }
  
  // Auto-set AI for SERIAL types
  if (type === 'SERIAL' || type === 'BIGSERIAL') {
    col.isAutoIncrement = true;
    col.isPrimaryKey = true;
  }
}

onMounted(async () => {
  if (tab.value?.metadata?.tableName) {
    loading.value = true;
    try {
      const def = await invoke<TableDefinition>('get_table_definition', {
        id: tab.value.connectionId,
        tableName: tab.value.metadata.tableName,
        catalog: tab.value.metadata.catalog,
        schema: tab.value.metadata.schema
      });
      tableName.value = def.name;
      columns.value = def.columns.map(col => ({
        ...col,
        dataType: col.dataType.toUpperCase(),
        length: col.length ? col.length.toString() : null
      }));
      catalog.value = def.catalog;
      schema.value = def.schema;
      tableCollation.value = def.collation ?? null;

      loadIndexes();
      
      // Capture original state
      originalDefinition.value = JSON.stringify(toRaw({
        name: tableName.value,
        columns: columns.value.map(c => toRaw(c)),
        catalog: catalog.value,
        schema: schema.value
      }));
    } catch (e: any) {
      error.value = e.toString();
    } finally {
      loading.value = false;
    }
  } else {
    // New table default column
    tableName.value = 'new_table';
    catalog.value = tab.value?.metadata?.catalog || tab.value?.database || connectionStore.activeDatabase || undefined;
    schema.value = tab.value?.metadata?.schema || tab.value?.schema || connectionStore.activeSchema || undefined;
    
    // Add default ID column for new table
    (columns.value as EditableColumn[]).push({
      name: 'id',
      dataType: 'INT',
      length: null,
      isNullable: false,
      isPrimaryKey: true,
      isAutoIncrement: true,
      defaultValue: null,
      comment: null,
      collation: null,
      _isNew: true,
    });

    originalDefinition.value = JSON.stringify(toRaw({
      name: tableName.value,
      columns: columns.value.map(c => toRaw(c)),
      catalog: catalog.value,
      schema: schema.value
    }));
  }
});

watch([tableName, columns, catalog, schema], () => {
  if (originalDefinition.value && tab.value) {
    const current = JSON.stringify(toRaw({
      name: tableName.value,
      columns: columns.value.map(c => toRaw(c)),
      catalog: catalog.value,
      schema: schema.value
    }));
    const dirty = current !== originalDefinition.value;
    tab.value.isDirty = dirty;
  }
}, { deep: true });

function addColumn() {
  (columns.value as EditableColumn[]).push({
    name: `column_${columns.value.length + 1}`,
    dataType: 'VARCHAR',
    length: "255",
    isNullable: true,
    isPrimaryKey: false,
    isAutoIncrement: false,
    defaultValue: null,
    comment: null,
    collation: null,
    _isNew: true,
  });
}

function markDirty(col: EditableColumn) {
  if (!col._isNew) col._isDirty = true;
}

function requestRemoveColumn(index: number) {
  removeTargetIdx.value = index;
  removeDialogVisible.value = true;
}

function confirmRemoveColumn() {
  if (removeTargetIdx.value !== null) {
    columns.value.splice(removeTargetIdx.value, 1);
  }
  removeDialogVisible.value = false;
  removeTargetIdx.value = null;
}

async function loadIndexes() {
  if (!tab.value?.metadata?.tableName || !tab.value?.connectionId) return;
  indexLoading.value = true;
  try {
    const info = await invoke<TableIndexInfo>('get_table_indexes', {
      id: tab.value.connectionId,
      tableName: tab.value.metadata.tableName,
      catalog: tab.value.metadata.catalog,
      schema: tab.value.metadata.schema,
    });
    indexInfo.value = info;
  } catch {
    indexInfo.value = { indexes: [], foreignKeys: [] };
  } finally {
    indexLoading.value = false;
  }
}

function moveColumn(index: number, direction: 'up' | 'down') {
  if (direction === 'up' && index > 0) {
    const item = columns.value.splice(index, 1)[0];
    if (item) columns.value.splice(index - 1, 0, item);
  } else if (direction === 'down' && index < columns.value.length - 1) {
    const item = columns.value.splice(index, 1)[0];
    if (item) columns.value.splice(index + 1, 0, item);
  }
}

async function handleSave() {
  if (!tableName.value) {
    error.value = 'Table name is required';
    return;
  }
  if (columns.value.length === 0) {
    error.value = 'At least one column is required';
    return;
  }

  loading.value = true;
  error.value = null;

  try {
    const definition: TableDefinition = {
      name: tableName.value,
      columns: columns.value,
      catalog: catalog.value,
      schema: schema.value,
      comment: null
    };

    if (isNew.value) {
      await invoke('create_table', { 
        id: tab.value?.connectionId, 
        definition 
      });
      uiStore.showToast(`Table "${definition.name}" created successfully.`);
    } else {
      const oldDefinition = JSON.parse(originalDefinition.value!);
      await invoke('alter_table', {
        id: tab.value?.connectionId,
        oldDefinition,
        newDefinition: definition
      });
      uiStore.showToast(`Table "${definition.name}" altered successfully.`);
    }

    // Success - refresh objects in sidebar
    if (tab.value?.connectionId) {
      await connectionStore.refreshObjects(tab.value.connectionId);
    }

    // Update tab metadata if it was new, to transition to Edit mode
    if (tab.value && !tab.value.metadata?.tableName) {
      tab.value.title = `Edit: ${tableName.value}`;
      if (!tab.value.metadata) tab.value.metadata = {};
      tab.value.metadata.tableName = tableName.value;
    }

    // Re-fetch definition from DB to ensure UI is in sync with reality
    if (tab.value?.connectionId) {
      const updatedDef: TableDefinition = await invoke('get_table_definition', {
        id: tab.value.connectionId,
        tableName: tableName.value,
        catalog: catalog.value,
        schema: schema.value
      });

      // Update local state with fresh data
      tableName.value = updatedDef.name;
      columns.value = updatedDef.columns;
      catalog.value = updatedDef.catalog;
      schema.value = updatedDef.schema;

      // Reset dirty state by capturing new original state
      originalDefinition.value = JSON.stringify(toRaw({
        name: tableName.value,
        columns: columns.value.map(c => toRaw(c)),
        catalog: catalog.value,
        schema: schema.value
      }));
      
      if (tab.value) {
        tab.value.isDirty = false;
      }
    }
  } catch (e: any) {
    error.value = e.toString();
  } finally {
    loading.value = false;
  }
}

function handleClose() {
  tabStore.closeTab(props.tabId);
}

function hideTypeMenu(col: any) {
  setTimeout(() => col.showTypeMenu = false, 200);
}

function getColStyle(key: string) {
  return {
    width: columnWidths.value[key] + 'px',
    minWidth: columnWidths.value[key] + 'px',
    maxWidth: columnWidths.value[key] + 'px'
  };
}

const connectionName = computed(() => {
  const conn = connectionStore.connections.find((c: any) => c.id === tab.value?.connectionId);
  return conn?.name || 'No Connection';
});
</script>

<template>
  <div class="table-editor glass flex-column h-full">
    <div class="editor-header flex-between">
      <div class="header-left flex-center gap-4">
        <div class="table-name-input">
          <input v-model="tableName" type="text" placeholder="Table Name" />
        </div>
        <div class="connection-info flex-center gap-2">
          <Database :size="14" class="text-accent" />
          <div class="flex flex-col">
            <span class="text-xs font-bold text-accent leading-tight">{{ connectionName }}</span>
            <span v-if="catalog || schema" class="text-9px opacity-70 text-accent leading-none">
              {{ catalog || schema }}{{ catalog && schema ? '.' + schema : '' }}
            </span>
          </div>
        </div>
        <div v-if="tableCollation" class="collation-badge">
          <span class="collation-label">Collation</span>
          <span class="collation-value">{{ tableCollation }}</span>
        </div>
      </div>
      <div class="header-center flex-center gap-1">
        <button
          class="tab-btn"
          :class="{ active: activeTab === 'columns' }"
          @click="activeTab = 'columns'"
        >Columns</button>
        <button
          class="tab-btn"
          :class="{ active: activeTab === 'indexes' }"
          @click="activeTab = 'indexes'; if (!isNew) loadIndexes()"
        >Indexes &amp; Keys</button>
      </div>
      <div class="header-right flex-center gap-2">
        <button
          v-if="!isNew"
          class="button-secondary btn-xs"
          :class="{ active: showPreview && previewType === 'alter' }"
          @click="togglePreview('alter')"
        >
          SQL: Alter
        </button>
        <button
          class="button-secondary btn-xs"
          :class="{ active: showPreview && previewType === 'create' }"
          @click="togglePreview('create')"
        >
          SQL: Create
        </button>
        <button class="button-primary btn-xs" @click="handleSave" :disabled="loading">
          <Save :size="14" />
          Save
        </button>
        <button class="button-secondary btn-xs" @click="handleClose">
          <X :size="14" />
          Cancel
        </button>
      </div>
    </div>

    <div v-if="error" class="error-banner flex-between">
      <span>{{ error }}</span>
      <button class="icon-btn xs" @click="error = null"><X :size="14" /></button>
    </div>

    <div class="editor-content flex-1 h-full overflow-auto">
      <div v-if="loading" class="flex-center p-10">
        <div class="spinner"></div>
        <span class="ml-2">Processing...</span>
      </div>

      <!-- Columns tab -->
      <div v-if="activeTab === 'columns'" class="table-container">
        <table class="column-grid">
          <thead>
            <tr>
              <th :style="getColStyle('drag')"></th>
              <th :style="getColStyle('name')">Name<div class="resizer" @mousedown="startResizing($event, 'name')"></div></th>
              <th :style="getColStyle('type')">Type<div class="resizer" @mousedown="startResizing($event, 'type')"></div></th>
              <th :style="getColStyle('length')">Length<div class="resizer" @mousedown="startResizing($event, 'length')"></div></th>
              <th :style="getColStyle('nullable')">Nullable<div class="resizer" @mousedown="startResizing($event, 'nullable')"></div></th>
              <th :style="getColStyle('pk')">PK<div class="resizer" @mousedown="startResizing($event, 'pk')"></div></th>
              <th :style="getColStyle('ai')">AI<div class="resizer" @mousedown="startResizing($event, 'ai')"></div></th>
              <th :style="getColStyle('default')">Default<div class="resizer" @mousedown="startResizing($event, 'default')"></div></th>
              <th :style="getColStyle('collation')">Collation<div class="resizer" @mousedown="startResizing($event, 'collation')"></div></th>
              <th :style="getColStyle('comment')">Comment<div class="resizer" @mousedown="startResizing($event, 'comment')"></div></th>
              <th :style="getColStyle('actions')">Actions</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="(col, idx) in columns"
              :key="idx"
              :class="{
                'row-new': (col as EditableColumn)._isNew,
                'row-dirty': (col as EditableColumn)._isDirty && !(col as EditableColumn)._isNew
              }"
            >
              <td :style="getColStyle('drag')" class="drag-handle">
                <div class="flex-center gap-1">
                  <button class="icon-btn xs" @click="moveColumn(idx, 'up')" :disabled="idx === 0"><ChevronUp :size="10" /></button>
                  <button class="icon-btn xs" @click="moveColumn(idx, 'down')" :disabled="idx === columns.length - 1"><ChevronDown :size="10" /></button>
                </div>
              </td>
              <td :style="getColStyle('name')">
                <input v-model="col.name" type="text" class="grid-input" @input="markDirty(col as EditableColumn)" />
              </td>
              <td :style="getColStyle('type')">
                <div class="type-select-wrapper relative">
                  <input
                    v-model="col.dataType"
                    type="text"
                    class="grid-input"
                    placeholder="Type..."
                    @focus="(col as any).showTypeMenu = true"
                    @blur="hideTypeMenu(col)"
                    @change="onTypeChange(col); markDirty(col as EditableColumn)"
                  />
                  <div v-if="(col as any).showTypeMenu" class="type-menu-dropdown glass">
                    <div
                      v-for="t in commonTypes.filter(type => type.toLowerCase().includes(col.dataType.toLowerCase()))"
                      :key="t"
                      class="type-option"
                      @mousedown="col.dataType = t; onTypeChange(col); markDirty(col as EditableColumn)"
                    >{{ t }}</div>
                  </div>
                </div>
              </td>
              <td :style="getColStyle('length')">
                <input v-model="col.length" type="text" class="grid-input" placeholder="-" @input="markDirty(col as EditableColumn)" />
              </td>
              <td :style="getColStyle('nullable')" class="text-center">
                <input v-model="col.isNullable" type="checkbox" @change="markDirty(col as EditableColumn)" />
              </td>
              <td :style="getColStyle('pk')" class="text-center">
                <input v-model="col.isPrimaryKey" type="checkbox" @change="markDirty(col as EditableColumn)" />
              </td>
              <td :style="getColStyle('ai')" class="text-center">
                <input v-model="col.isAutoIncrement" type="checkbox" @change="markDirty(col as EditableColumn)" />
              </td>
              <td :style="getColStyle('default')">
                <input v-model="col.defaultValue" type="text" class="grid-input" placeholder="" @input="markDirty(col as EditableColumn)" />
              </td>
              <td :style="getColStyle('collation')" class="collation-cell">
                <span class="collation-text">{{ col.collation || '' }}</span>
              </td>
              <td :style="getColStyle('comment')">
                <input v-model="col.comment" type="text" class="grid-input" @input="markDirty(col as EditableColumn)" />
              </td>
              <td :style="getColStyle('actions')" class="text-center">
                <button class="icon-btn icon-btn-danger" @click="requestRemoveColumn(idx)">
                  <Trash2 :size="14" />
                </button>
              </td>
            </tr>
          </tbody>
        </table>

        <div class="add-column-bar">
          <button class="add-column-btn button-primary" @click="addColumn">
            <Plus :size="15" />
            Add Column
          </button>
        </div>
      </div>

      <!-- Indexes & Keys tab -->
      <div v-else-if="activeTab === 'indexes'" class="indexes-panel">
        <div v-if="isNew" class="state-center text-secondary p-8">
          Save the table first to manage indexes and foreign keys.
        </div>
        <template v-else>
          <div v-if="indexLoading" class="state-center gap-2 text-secondary p-6">
            <div class="spinner"></div> Loading…
          </div>
          <template v-else>
            <!-- Indexes -->
            <div class="index-section">
              <div class="index-section-title">
                <Key :size="14" />
                Indexes
              </div>
              <div v-if="!indexInfo?.indexes.length" class="index-empty">No indexes defined.</div>
              <table v-else class="index-grid">
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Type</th>
                    <th>Method</th>
                    <th>Columns</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="idx in indexInfo!.indexes" :key="idx.name"
                    :class="{ 'idx-primary': idx.indexType === 'PRIMARY', 'idx-unique': idx.indexType === 'UNIQUE' }">
                    <td class="idx-name">{{ idx.name }}</td>
                    <td><span class="idx-badge" :class="'badge-' + idx.indexType.toLowerCase()">{{ idx.indexType }}</span></td>
                    <td class="text-muted">{{ idx.method }}</td>
                    <td class="text-muted">{{ idx.columns.join(', ') }}</td>
                  </tr>
                </tbody>
              </table>
            </div>

            <!-- Foreign Keys -->
            <div class="index-section">
              <div class="index-section-title">
                <Link :size="14" />
                Foreign Keys
              </div>
              <div v-if="!indexInfo?.foreignKeys.length" class="index-empty">No foreign keys defined.</div>
              <table v-else class="index-grid">
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Columns</th>
                    <th>References</th>
                    <th>On Update</th>
                    <th>On Delete</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="fk in indexInfo!.foreignKeys" :key="fk.name">
                    <td class="idx-name">{{ fk.name }}</td>
                    <td class="text-muted">{{ fk.columns.join(', ') }}</td>
                    <td class="text-muted">
                      <span class="fk-ref">{{ fk.referencedTable }}</span>
                      <span class="text-secondary"> ({{ fk.referencedColumns.join(', ') }})</span>
                    </td>
                    <td class="text-muted">{{ fk.onUpdate }}</td>
                    <td class="text-muted">{{ fk.onDelete }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </template>
        </template>
      </div>

      <div v-if="showPreview" class="sql-preview-container">
        <div class="preview-header flex-between">
          <span class="text-xs font-bold text-secondary uppercase">SQL Preview</span>
          <div v-if="previewLoading" class="spinner-xs"></div>
        </div>
        <pre class="sql-code"><code>{{ previewSql }}</code></pre>
      </div>
    </div>
  </div>

  <ConfirmDialog
    :show="removeDialogVisible"
    type="danger"
    title="Remove Column"
    :message="removeTargetIdx !== null ? `Remove column '${columns[removeTargetIdx]?.name}'? This cannot be undone.` : ''"
    confirm-label="Remove"
    cancel-label="Cancel"
    @confirm="confirmRemoveColumn"
    @cancel="removeDialogVisible = false; removeTargetIdx = null"
  />
</template>

<style scoped>
.table-editor {
  background: var(--bg-primary);
}

.editor-header {
  padding: 8px 16px;
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
}

.table-name-input {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.table-name-input label {
  font-size: 0.7rem;
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
}

.table-name-input input {
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  padding: 6px 12px;
  border-radius: 4px;
  font-size: 0.9rem;
  width: 250px;
}

.table-name-input input:focus {
  border-color: var(--accent-primary);
  outline: none;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.editor-content {
  overflow: auto;
  position: relative;
  display: flex;
  flex-direction: column;
}

.table-container {
  overflow-x: auto;
  flex: 1;
}

.column-grid {
  width: max-content;
  min-width: 100%;
  border-collapse: collapse;
  font-size: 0.85rem;
  table-layout: fixed;
}

.column-grid th {
  background: var(--bg-tertiary);
  padding: 6px 10px;
  text-align: left;
  border-bottom: 2px solid var(--border-color);
  border-right: 1px solid var(--border-color);
  color: var(--text-secondary);
  font-weight: 600;
  position: sticky;
  top: 0;
  z-index: 10;
  white-space: nowrap;
}

.column-grid th .resizer {
  position: absolute;
  top: 0;
  right: 0;
  width: 4px;
  height: 100%;
  cursor: col-resize;
  z-index: 11;
}

.column-grid th .resizer:hover {
  background: var(--accent-primary);
}

.column-grid td {
  padding: 2px 4px;
  border-bottom: 1px solid var(--border-color);
  border-right: 1px solid var(--border-color);
}

.grid-input {
  width: 100%;
  background: transparent;
  border: 1px solid transparent;
  color: var(--text-primary);
  padding: 2px 6px;
  border-radius: 4px;
}

.grid-input:hover {
  background: var(--glass-border);
}

.grid-input:focus {
  background: var(--bg-secondary);
  border-color: var(--accent-primary);
  outline: none;
}

.button-secondary.active {
  background: var(--accent-primary);
  color: white;
  border-color: var(--accent-primary);
}

.column-grid td.flex-center {
  display: flex;
  align-items: center;
  justify-content: center;
}

.drag-handle {
  width: 50px;
  vertical-align: middle;
  text-align: center;
}
.text-xs { font-size: 0.75rem; }
.ml-2 { margin-left: 8px; }
.p-4 { padding: 16px; }
.p-10 { padding: 40px; }

.error-banner {
  background: rgba(239, 68, 68, 0.1);
  color: #ef4444;
  padding: 8px 20px;
  border-bottom: 1px solid rgba(239, 68, 68, 0.2);
  font-size: 0.85rem;
}

.toolbar-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  background: transparent;
  border: 1px dashed var(--border-color);
  color: var(--text-secondary);
  padding: 8px 16px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  width: 100%;
  justify-content: center;
}

.toolbar-btn:hover {
  background: var(--glass-border);
  color: var(--text-primary);
  border-color: var(--accent-primary);
}

.spinner {
  width: 20px;
  height: 20px;
  border: 2px solid var(--glass-border);
  border-top-color: var(--accent-primary);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.sql-preview-container {
  margin: 16px;
  padding: 12px;
  background: #0d1117;
  border-radius: 6px;
  border: 1px solid var(--border-color);
}

.preview-header {
  margin-bottom: 8px;
  padding-bottom: 8px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}

.sql-code {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-all;
  color: #c9d1d9;
  font-family: var(--font-mono);
  font-size: 0.8rem;
}

.spinner-xs {
  width: 12px;
  height: 12px;
  border: 1.5px solid rgba(255, 255, 255, 0.1);
  border-top-color: var(--accent-primary);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

.type-menu-dropdown {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  max-height: 200px;
  overflow-y: auto;
  z-index: 100;
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 4px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
}

.type-option {
  padding: 6px 10px;
  cursor: pointer;
  font-size: 0.8rem;
}

.type-option:hover {
  background: var(--accent-primary);
  color: white;
}

/* Tab switcher */
.header-center {
  flex: 1;
  justify-content: center;
}

.tab-btn {
  background: transparent;
  border: 1px solid var(--border-color);
  color: var(--text-secondary);
  padding: 4px 14px;
  border-radius: 4px;
  font-size: 0.78rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.15s;
}

.tab-btn + .tab-btn {
  margin-left: 4px;
}

.tab-btn.active {
  background: var(--accent-primary);
  color: white;
  border-color: var(--accent-primary);
}

/* Add Column bar */
.add-column-bar {
  padding: 12px 16px;
  border-top: 1px solid var(--border-color);
  background: var(--bg-secondary);
}

.add-column-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 0.82rem;
  font-weight: 600;
  padding: 7px 18px;
  border-radius: 6px;
  cursor: pointer;
  background: var(--accent-primary);
  color: white;
  border: none;
  box-shadow: 0 2px 8px rgba(59, 130, 246, 0.35);
  transition: all 0.15s;
}

.add-column-btn:hover {
  opacity: 0.9;
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(59, 130, 246, 0.45);
}

/* New / dirty row highlight */
.row-new td {
  background: rgba(34, 197, 94, 0.08) !important;
}
.row-new td:first-child {
  border-left: 3px solid #22c55e;
}
.row-dirty td {
  background: rgba(234, 179, 8, 0.08) !important;
}
.row-dirty td:first-child {
  border-left: 3px solid #eab308;
}

/* Indexes panel */
.indexes-panel {
  flex: 1;
  overflow: auto;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.index-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.index-section-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 0.72rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--text-secondary);
  padding-bottom: 6px;
  border-bottom: 1px solid var(--border-color);
}

.index-empty {
  font-size: 0.8rem;
  color: var(--text-secondary);
  padding: 8px 0;
  font-style: italic;
}

.index-grid {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.82rem;
}

.index-grid th {
  background: var(--bg-tertiary);
  padding: 6px 10px;
  text-align: left;
  border-bottom: 2px solid var(--border-color);
  color: var(--text-secondary);
  font-weight: 600;
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.index-grid td {
  padding: 6px 10px;
  border-bottom: 1px solid var(--border-color);
}

.idx-name {
  font-family: var(--font-mono);
  font-size: 0.78rem;
  color: var(--text-primary);
}

.idx-badge {
  font-size: 0.65rem;
  font-weight: 700;
  padding: 2px 7px;
  border-radius: 10px;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.badge-primary { background: rgba(59, 130, 246, 0.15); color: #3b82f6; }
.badge-unique  { background: rgba(139, 92, 246, 0.15); color: #8b5cf6; }
.badge-index   { background: rgba(107, 114, 128, 0.15); color: #9ca3af; }

.fk-ref {
  font-family: var(--font-mono);
  font-size: 0.78rem;
  color: var(--accent-primary);
}

.idx-primary td { background: rgba(59, 130, 246, 0.04); }
.idx-unique td  { background: rgba(139, 92, 246, 0.04); }

.p-8 { padding: 32px; }
.state-center { display: flex; align-items: center; justify-content: center; }

.collation-badge {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: 4px;
}

.collation-label {
  font-size: 0.65rem;
  font-weight: 600;
  text-transform: uppercase;
  color: var(--text-secondary);
  letter-spacing: 0.04em;
}

.collation-value {
  font-size: 0.72rem;
  color: var(--accent-primary);
  font-family: var(--font-mono);
}

.collation-cell {
  vertical-align: middle;
  padding: 4px 8px;
}

.collation-text {
  font-size: 0.72rem;
  color: var(--text-secondary);
  font-family: var(--font-mono);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  display: block;
}
</style>
