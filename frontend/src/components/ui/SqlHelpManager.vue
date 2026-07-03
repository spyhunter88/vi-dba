<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { 
  BookOpen, X, Search, Copy, Check, ChevronDown, ChevronRight, HelpCircle, Sparkles, Database 
} from 'lucide-vue-next';
import type { TableDefinition, TableColumn, DbType } from '../../types';
import { extractReferencedTableNames } from '../../utils/sqlCompletion';
import { SNIPPETS } from '../../utils/sqlSnippets';

const props = defineProps<{
  show: boolean;
  tabId: string;
  dbType: DbType;
  connectionId: string | null;
  catalog?: string;
  schema?: string;
  queryText: string;
}>();

const emit = defineEmits(['close', 'insert']);

// Navigation & Search
const activeTab = ref<'snippets' | 'context'>('snippets');
const searchIndex = ref('');
const selectedCategory = ref<string>('all');
const selectedDialect = ref<DbType>(props.dbType);

// Reference Context State
const referencedTables = ref<string[]>([]);
const tableDefinitions = ref<Record<string, TableDefinition>>({});
const loadingTables = ref<Record<string, boolean>>({});
const collapsedTables = ref<Record<string, boolean>>({});
const copiedSnippetId = ref<string | null>(null);

// Category mappings
const categories = [
  { id: 'all', name: 'Tất cả' },
  { id: 'datetime', name: 'Ngày & Giờ' },
  { id: 'timezone', name: 'Múi giờ' },
  { id: 'json', name: 'JSON' },
  { id: 'conditionals', name: 'Điều kiện' },
  { id: 'window', name: 'Window Fn' }
];

// Dialect mapping names
const dialectNames: Record<DbType, string> = {
  postgreSQL: 'PostgreSQL',
  mySQL: 'MySQL',
  sqlServer: 'SQL Server',
  sqlite: 'SQLite',
  oracle: 'Oracle',
  mongoDB: 'MongoDB'
};

// Computed list of filtered snippets
const filteredSnippets = computed(() => {
  return SNIPPETS.filter(s => {
    // Filter by category
    if (selectedCategory.value !== 'all' && s.category !== selectedCategory.value) {
      return false;
    }
    
    // Filter by dialect availability
    const dialect = s.dialects[selectedDialect.value];
    if (!dialect) {
      return false;
    }

    // Filter by search string
    const query = searchIndex.value.toLowerCase().trim();
    if (!query) return true;

    const code = dialect.code.toLowerCase();
    const example = dialect.example.toLowerCase();
    
    return s.name.toLowerCase().includes(query) || 
           s.description.toLowerCase().includes(query) || 
           code.includes(query) ||
           example.includes(query);
  });
});

// Update selected dialect when prop change
watch(() => props.dbType, (newVal) => {
  if (newVal) {
    selectedDialect.value = newVal;
  }
});

// Watch query text to parse table references
watch(() => props.queryText, (newText) => {
  if (activeTab.value === 'context' && props.show) {
    updateReferencedTables(newText);
  }
}, { immediate: true });

// Load tables when tab switches to context
watch(activeTab, (newTab) => {
  if (newTab === 'context') {
    updateReferencedTables(props.queryText);
  }
});

watch(() => props.show, (newVal) => {
  if (newVal && activeTab.value === 'context') {
    updateReferencedTables(props.queryText);
  }
});

function updateReferencedTables(sqlText: string) {
  if (!props.connectionId) {
    referencedTables.value = [];
    return;
  }
  
  const tables = Array.from(extractReferencedTableNames(sqlText));
  referencedTables.value = tables;
  
  // Fetch metadata for new tables
  tables.forEach(table => {
    if (!tableDefinitions.value[table] && !loadingTables.value[table]) {
      fetchTableDefinition(table);
    }
  });
}

async function fetchTableDefinition(tableName: string) {
  if (!props.connectionId) return;
  
  loadingTables.value[tableName] = true;
  try {
    const def = await invoke<TableDefinition>('get_table_definition', {
      id: props.connectionId,
      tableName,
      catalog: props.catalog || null,
      schema: props.schema || null
    });
    tableDefinitions.value[tableName] = def;
    if (collapsedTables.value[tableName] === undefined) {
      collapsedTables.value[tableName] = false;
    }
  } catch (err) {
    console.error(`Failed to fetch table definition for ${tableName}:`, err);
  } finally {
    loadingTables.value[tableName] = false;
  }
}

// Helpers for Timestamp Columns detection
function isTimestampColumn(col: TableColumn): boolean {
  const nameLower = col.name.toLowerCase();
  const typeLower = col.dataType.toLowerCase();
  const commentLower = (col.comment || '').toLowerCase();
  
  // Skip actual DateTime types
  if (/date|time|timestamp/i.test(typeLower)) return false;
  
  const suggestions = ['stamp', 'epoch', 'unix', 'millis', 'mili', 'created_at', 'updated_at', 'time_int', 'date_int'];
  return suggestions.some(s => nameLower.includes(s) || commentLower.includes(s));
}

function getTimestampConversion(col: TableColumn): string {
  const name = col.name;
  const comment = (col.comment || '').toLowerCase();
  const isMs = /ms|millis|mili/i.test(comment) || /ms|millis/i.test(name.toLowerCase());
  const dialect = selectedDialect.value;

  switch (dialect) {
    case 'postgreSQL':
      return isMs ? `TO_TIMESTAMP(\${1:${name}} / 1000.0)` : `TO_TIMESTAMP(\${1:${name}})`;
    case 'mySQL':
      return isMs ? `FROM_UNIXTIME(\${1:${name}} / 1000)` : `FROM_UNIXTIME(\${1:${name}})`;
    case 'sqlite':
      return isMs ? `DATETIME(\${1:${name}} / 1000, 'unixepoch')` : `DATETIME(\${1:${name}}, 'unixepoch')`;
    case 'sqlServer':
      return isMs ? `DATEADD(millisecond, \${1:${name}}, '1970-01-01')` : `DATEADD(second, \${1:${name}}, '1970-01-01')`;
    case 'oracle':
      return isMs
        ? `TO_TIMESTAMP('1970-01-01 00:00:00','YYYY-MM-DD HH24:MI:SS') + NUMTODSINTERVAL(\${1:${name}} / 1000, 'SECOND')`
        : `TO_TIMESTAMP('1970-01-01 00:00:00','YYYY-MM-DD HH24:MI:SS') + NUMTODSINTERVAL(\${1:${name}}, 'SECOND')`;
    case 'mongoDB':
      return `{\n  $toDate: "$${name}"\n}`;
    default:
      return `\${1:${name}}`;
  }
}

function handleInsert(code: string) {
  emit('insert', code);
}

function handleCopy(code: string, id: string) {
  navigator.clipboard.writeText(code).then(() => {
    copiedSnippetId.value = id;
    setTimeout(() => {
      if (copiedSnippetId.value === id) {
        copiedSnippetId.value = null;
      }
    }, 2000);
  });
}

function toggleCollapse(tableName: string) {
  collapsedTables.value[tableName] = !collapsedTables.value[tableName];
}
</script>

<template>
  <div v-if="show" class="help-sidebar">
    <!-- Header -->
    <div class="sidebar-header">
      <div class="flex-center gap-1.5">
        <BookOpen :size="13" class="text-accent" style="opacity: 0.65;" />
        <span class="sidebar-title">SQL Help Center</span>
      </div>
      <button class="close-btn" @click="$emit('close')" title="Close Help">
        <X :size="13" />
      </button>
    </div>

    <!-- Mode Selector Dropdown -->
    <div class="dialect-selector-bar">
      <span class="label">Dialect:</span>
      <select v-model="selectedDialect" class="dialect-select">
        <option v-for="(name, value) in dialectNames" :key="value" :value="value">
          {{ name }}
        </option>
      </select>
    </div>

    <!-- Navigation Tabs -->
    <div class="tabs-header">
      <button 
        class="tab-btn" 
        :class="{ active: activeTab === 'snippets' }"
        @click="activeTab = 'snippets'"
      >
        Cú pháp
      </button>
      <button 
        class="tab-btn" 
        :class="{ active: activeTab === 'context' }"
        @click="activeTab = 'context'"
      >
        Ngữ cảnh
        <span v-if="referencedTables.length > 0" class="badge">
          {{ referencedTables.length }}
        </span>
      </button>
    </div>

    <!-- Main Sidebar Content Area -->
    <div class="sidebar-content">
      <!-- SNIPPETS VIEW -->
      <div v-if="activeTab === 'snippets'" class="snippets-tab">
        <div class="search-box">
          <Search :size="12" class="search-icon" />
          <input 
            v-model="searchIndex" 
            type="text" 
            placeholder="Tìm kiếm cú pháp..." 
            class="search-input"
          />
          <button v-if="searchIndex" class="clear-search" @click="searchIndex = ''">
            <X :size="10" />
          </button>
        </div>

        <!-- Categories tags -->
        <div class="category-scroll">
          <button 
            v-for="cat in categories" 
            :key="cat.id"
            class="category-tag"
            :class="{ active: selectedCategory === cat.id }"
            @click="selectedCategory = cat.id"
          >
            {{ cat.name }}
          </button>
        </div>

        <!-- List of snippets -->
        <div v-if="filteredSnippets.length === 0" class="state-box empty">
          <HelpCircle :size="20" class="state-icon" />
          <span class="state-text">Không tìm thấy mẫu phù hợp</span>
        </div>
        <div v-else class="snippets-list">
          <div 
            v-for="snippet in filteredSnippets" 
            :key="snippet.id" 
            class="snippet-item"
          >
            <div class="item-header">
              <span class="snippet-title">{{ snippet.name }}</span>
              <div class="action-buttons">
                <button 
                  class="action-btn" 
                  title="Copy"
                  @click="handleCopy(snippet.dialects[selectedDialect]?.code || '', snippet.id)"
                >
                  <Check v-if="copiedSnippetId === snippet.id" :size="10" class="text-success" />
                  <Copy v-else :size="10" />
                </button>
                <button 
                  class="action-btn insert" 
                  title="Insert into editor"
                  @click="handleInsert(snippet.dialects[selectedDialect]?.code || '')"
                >
                  Chèn
                </button>
              </div>
            </div>
            
            <p class="snippet-desc">{{ snippet.description }}</p>
            
            <div class="code-preview-box">
              <div class="code-label">Cú pháp:</div>
              <pre class="code-pre"><code>{{ snippet.dialects[selectedDialect]?.code }}</code></pre>
              
              <div class="code-label" style="margin-top: 4px;">Ví dụ:</div>
              <pre class="code-pre example"><code>{{ snippet.dialects[selectedDialect]?.example }}</code></pre>
            </div>
          </div>
        </div>
      </div>

      <!-- SCHEMA TABLE CONTEXT VIEW -->
      <div v-else-if="activeTab === 'context'" class="context-tab">
        <div v-if="!connectionId" class="state-box">
          <Database :size="20" class="state-icon" />
          <span class="state-text">Chưa kết nối database</span>
        </div>
        
        <div v-else-if="referencedTables.length === 0" class="state-box info">
          <Sparkles :size="20" class="state-icon text-accent" />
          <span class="state-text">Nhập tên bảng trong câu query (ví dụ: <code>FROM table_name</code>) để xem mô tả.</span>
        </div>

        <div v-else class="tables-list">
          <div 
            v-for="table in referencedTables" 
            :key="table" 
            class="table-context-card"
          >
            <!-- Table Header info -->
            <div class="table-card-header" @click="toggleCollapse(table)">
              <div class="flex-center gap-1.5 overflow-hidden">
                <component 
                  :is="collapsedTables[table] ? ChevronRight : ChevronDown" 
                  :size="12" 
                  class="collapse-toggle-icon"
                />
                <span class="table-name" :title="table">{{ table }}</span>
              </div>
              <div v-if="loadingTables[table]" class="spinner-xs"></div>
            </div>

            <!-- Expandable Table details -->
            <div v-if="!collapsedTables[table]" class="table-card-body">
              <!-- Loading/Empty state -->
              <div v-if="loadingTables[table]" class="loading-placeholder">
                Đang tải schema...
              </div>
              <div v-else-if="!tableDefinitions[table] || !tableDefinitions[table].columns.length" class="loading-placeholder error">
                Không tìm thấy thông tin bảng
              </div>
              
              <div v-else>
                <!-- Table Comment description -->
                <div v-if="tableDefinitions[table].comment" class="table-comment">
                  📝 {{ tableDefinitions[table].comment }}
                </div>
                
                <!-- Columns list -->
                <div class="columns-table">
                  <div 
                    v-for="col in tableDefinitions[table].columns" 
                    :key="col.name" 
                    class="column-row"
                    :class="{ 'timestamp-highlight': isTimestampColumn(col) }"
                  >
                    <div class="col-meta">
                      <div class="col-name-type">
                        <span 
                          class="col-name" 
                          title="Click để chèn tên cột"
                          @click="handleInsert(col.name)"
                        >
                          {{ col.name }}
                        </span>
                        <span class="col-type">{{ col.dataType }}</span>
                      </div>
                      
                      <!-- Comment/Description -->
                      <p v-if="col.comment" class="col-comment">
                        {{ col.comment }}
                      </p>
                    </div>

                    <!-- Actions -->
                    <div class="col-actions">
                      <!-- Timestamp conversion helper -->
                      <button 
                        v-if="isTimestampColumn(col)" 
                        class="convert-btn" 
                        title="Phát hiện epoch/timestamp! Click để chèn cú pháp convert sang DateTime"
                        @click="handleInsert(getTimestampConversion(col))"
                      >
                        ⚡ Convert
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.help-sidebar {
  width: 280px;
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
  opacity: 0.65;
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

/* Dialect Selector Bar */
.dialect-selector-bar {
  padding: 6px 10px;
  display: flex;
  align-items: center;
  gap: 8px;
  background: rgba(0, 0, 0, 0.15);
  border-bottom: 1px solid var(--border-color);
  font-size: 11px;
}

.dialect-selector-bar .label {
  opacity: 0.5;
}

.dialect-select {
  flex: 1;
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: 4px;
  color: var(--text-primary);
  padding: 2px 6px;
  font-size: 11px;
  outline: none;
  cursor: pointer;
}

/* Tab Headers */
.tabs-header {
  display: flex;
  height: 30px;
  border-bottom: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.05);
}

.tab-btn {
  flex: 1;
  border: none;
  background: transparent;
  font-size: 11px;
  font-weight: 500;
  color: var(--text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  border-bottom: 2px solid transparent;
  transition: all 0.15s;
}

.tab-btn.active {
  color: var(--accent-primary);
  border-bottom-color: var(--accent-primary);
  background: var(--bg-secondary);
}

.tab-btn .badge {
  font-size: 9px;
  background: var(--accent-primary);
  color: #fff;
  padding: 0px 5px;
  border-radius: 10px;
  font-weight: 700;
}

/* Content Area */
.sidebar-content {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
  display: flex;
  flex-direction: column;
}

/* Search Box */
.search-box {
  position: relative;
  margin-bottom: 6px;
}

.search-icon {
  position: absolute;
  left: 8px;
  top: 50%;
  transform: translateY(-50%);
  opacity: 0.4;
}

.search-input {
  width: 100%;
  padding: 5px 22px 5px 24px;
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: 4px;
  color: var(--text-primary);
  font-size: 11px;
  outline: none;
}

.clear-search {
  position: absolute;
  right: 6px;
  top: 50%;
  transform: translateY(-50%);
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  opacity: 0.5;
}

.clear-search:hover {
  opacity: 1;
}

/* Categories Tags */
.category-scroll {
  display: flex;
  gap: 4px;
  overflow-x: auto;
  padding-bottom: 6px;
  margin-bottom: 8px;
  border-bottom: 1px solid rgba(var(--border-color-rgb), 0.5);
  scrollbar-width: none; /* Hide scrollbar for clean aesthetic */
}

.category-scroll::-webkit-scrollbar {
  display: none;
}

.category-tag {
  flex-shrink: 0;
  border: 1px solid var(--border-color);
  background: transparent;
  padding: 2px 8px;
  border-radius: 10px;
  font-size: 9px;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all 0.12s;
}

.category-tag:hover {
  background: var(--glass-border);
  color: var(--text-primary);
}

.category-tag.active {
  background: var(--accent-primary);
  border-color: var(--accent-primary);
  color: #fff;
}

/* Snippets list */
.snippets-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.snippet-item {
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background: var(--bg-primary);
  padding: 6px 8px;
  transition: border-color 0.15s;
}

.snippet-item:hover {
  border-color: rgba(var(--accent-primary-rgb), 0.3);
}

.item-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 4px;
}

.snippet-title {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-primary);
}

.action-buttons {
  display: flex;
  align-items: center;
  gap: 3px;
}

.action-btn {
  background: var(--glass-border);
  border: 1px solid transparent;
  color: var(--text-secondary);
  border-radius: 3px;
  padding: 1px 4px;
  font-size: 9px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  height: 16px;
}

.action-btn:hover {
  color: var(--text-primary);
  background: rgba(var(--accent-primary-rgb), 0.15);
}

.action-btn.insert {
  background: var(--accent-primary);
  color: #fff;
  font-weight: 500;
}

.action-btn.insert:hover {
  filter: brightness(1.1);
}

.snippet-desc {
  font-size: 10px;
  color: var(--text-secondary);
  margin: 3px 0 6px 0;
  line-height: 1.35;
}

/* Pre Code Preview */
.code-preview-box {
  background: rgba(0, 0, 0, 0.2);
  border-radius: 4px;
  padding: 4px 6px;
}

.code-label {
  font-size: 8px;
  text-transform: uppercase;
  color: var(--accent-primary);
  font-weight: 600;
  opacity: 0.8;
  margin-bottom: 2px;
}

.code-pre {
  margin: 0;
  font-family: 'Fira Code', 'JetBrains Mono', monospace;
  font-size: 10px;
  color: #a78bfa;
  white-space: pre-wrap;
  word-break: break-all;
}

.code-pre.example {
  color: #93c5fd;
  opacity: 0.85;
}

/* Context view */
.tables-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.table-context-card {
  border: 1px solid var(--border-color);
  border-radius: 6px;
  overflow: hidden;
  background: var(--bg-primary);
}

.table-card-header {
  padding: 6px 8px;
  background: rgba(0, 0, 0, 0.1);
  display: flex;
  align-items: center;
  justify-content: space-between;
  cursor: pointer;
  user-select: none;
  font-size: 11px;
}

.table-card-header:hover {
  background: rgba(0, 0, 0, 0.18);
}

.table-name {
  font-weight: 600;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.collapse-toggle-icon {
  opacity: 0.5;
}

.table-card-body {
  border-top: 1px solid var(--border-color);
  padding: 6px;
  background: var(--bg-secondary);
}

.loading-placeholder {
  font-size: 10px;
  color: var(--text-secondary);
  text-align: center;
  padding: 10px 0;
}

.loading-placeholder.error {
  color: #f87171;
}

.table-comment {
  font-size: 10px;
  color: var(--text-secondary);
  background: rgba(0, 0, 0, 0.08);
  padding: 4px 6px;
  border-radius: 4px;
  margin-bottom: 6px;
  border-left: 2px solid var(--accent-primary);
  line-height: 1.35;
}

/* Columns table rendering */
.columns-table {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.column-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  padding: 4px 6px;
  border-radius: 4px;
  border: 1px solid transparent;
  background: var(--bg-primary);
  transition: all 0.15s;
}

.column-row:hover {
  border-color: rgba(var(--accent-primary-rgb), 0.2);
  background: rgba(var(--accent-primary-rgb), 0.02);
}

.column-row.timestamp-highlight {
  border-color: rgba(245, 158, 11, 0.3); /* Amber border */
  background: rgba(245, 158, 11, 0.05); /* Amber background */
}

.col-meta {
  flex: 1;
  min-width: 0;
  margin-right: 6px;
}

.col-name-type {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 4px;
}

.col-name {
  font-family: 'Fira Code', 'JetBrains Mono', monospace;
  font-size: 10px;
  font-weight: 600;
  color: var(--text-primary);
  cursor: pointer;
  text-decoration: underline dotted rgba(var(--text-primary-rgb), 0.3);
}

.col-name:hover {
  color: var(--accent-primary);
  text-decoration: underline;
}

.col-type {
  font-size: 8px;
  padding: 0px 4px;
  background: var(--glass-border);
  color: var(--text-secondary);
  border-radius: 3px;
  font-weight: 500;
}

.col-comment {
  font-size: 9px;
  color: var(--text-secondary);
  margin: 2px 0 0 0;
  line-height: 1.25;
}

.col-actions {
  flex-shrink: 0;
  display: flex;
  align-items: center;
}

.convert-btn {
  border: 1px solid #f59e0b;
  background: rgba(245, 158, 11, 0.15);
  color: #f59e0b;
  font-size: 8px;
  font-weight: 700;
  padding: 1px 4px;
  border-radius: 3px;
  cursor: pointer;
  transition: all 0.12s;
  text-transform: uppercase;
}

.convert-btn:hover {
  background: #f59e0b;
  color: #fff;
  box-shadow: 0 0 4px rgba(245, 158, 11, 0.4);
}

/* Common states */
.state-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  gap: 8px;
  padding: 24px 10px;
}

.state-box.info {
  background: rgba(var(--accent-primary-rgb), 0.02);
  border: 1px dashed var(--border-color);
  border-radius: 6px;
  margin-top: 10px;
}

.state-icon {
  opacity: 0.5;
}

.state-text {
  font-size: 10.5px;
  color: var(--text-secondary);
  line-height: 1.4;
}

.state-text code {
  font-family: monospace;
  background: var(--glass-border);
  padding: 1px 3px;
  border-radius: 3px;
}

.text-success {
  color: #10b981;
}

.text-accent {
  color: var(--accent-primary);
}

.spinner-xs {
  width: 12px;
  height: 12px;
  border: 2px solid var(--glass-border);
  border-top-color: var(--accent-primary);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin { to { transform: rotate(360deg); } }
</style>
