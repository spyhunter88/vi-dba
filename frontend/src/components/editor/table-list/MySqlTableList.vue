<script setup lang="ts">
import { toRef } from 'vue';
import { Scaling, AlertCircle, Plus, Edit, Table, RefreshCw, Search, Upload, Download, Scissors, Eraser, Trash2 } from 'lucide-vue-next';
import { useTableList } from './useTableList';
import TableContextMenu from './TableContextMenu.vue';

const props = defineProps<{ tabId: string }>();
const {
  result, filteredRows, searchQuery, selectedRows, hasSelection, isSingleSelection,
  menuVisible, menuPos, contextRow,
  tab, connectionName,
  formatValue,
  selectRow, handleRowDoubleClick, handleCreateTable, handleEditTable,
  showContextMenu,
  handleViewFirstPage, handleViewLastPage, handleViewStructure,
  handleEmptyTable, handleTruncateTable, handleDropTable,
  handleImportContext, handleExportContext, handleGenerateQuery,
  handleImportSelected, handleExportSelected,
  handleEmptySelected, handleTruncateSelected, handleDropSelected,
  reload
} = useTableList(toRef(props, 'tabId'));

const engineColors: Record<string, string> = {
  InnoDB: '#3b82f6',
  MyISAM: '#f59e0b',
  MEMORY: '#8b5cf6',
  CSV: '#10b981',
  ARCHIVE: '#6b7280',
  BLACKHOLE: '#1f2937',
};

function engineColor(engine: string) {
  return engineColors[engine] || 'var(--text-secondary)';
}
</script>

<template>
  <div class="table-list-view">
    <div v-if="result?.loading" class="state-center gap-2 text-secondary">
      <Scaling :size="20" class="spin" />
      Fetching table metadata…
    </div>

    <template v-else>
      <div v-if="result?.error" class="state-center flex-col gap-3">
        <AlertCircle :size="40" class="text-error" />
        <div class="error-msg">{{ result.error }}</div>
      </div>

      <div v-else class="list-wrap">
        <div class="tbl-toolbar">
          <div class="tbl-toolbar-buttons">
            <div class="conn-info tbl-btn-group mr-3">
              <Scaling :size="14" class="text-accent" />
              <div class="flex flex-col">
                <span class="text-xs font-bold text-accent leading-tight">{{ connectionName }}</span>
                <span v-if="tab?.metadata?.catalog || tab?.metadata?.schema" class="text-9px opacity-60 text-accent leading-none">
                  {{ tab.metadata.catalog || '' }}{{ tab.metadata.catalog && tab.metadata.schema ? '.' : '' }}{{ tab.metadata.schema || '' }}
                </span>
              </div>
            </div>
            <button class="tbl-btn" title="Refresh" @click="reload"><RefreshCw :size="15" /></button>
            <button class="tbl-btn" title="Create Table" @click="handleCreateTable"><Plus :size="15" /></button>
            <button class="tbl-btn" title="Edit Table" :disabled="!isSingleSelection" @click="handleEditTable()"><Edit :size="15" /></button>
            <button class="tbl-btn" title="Import" :disabled="!isSingleSelection" @click="handleImportSelected"><Upload :size="15" /></button>
            <button class="tbl-btn" title="Export" :disabled="!hasSelection" @click="handleExportSelected"><Download :size="15" /></button>
            <span class="tbl-btn-sep"></span>
            <button class="tbl-btn warn" title="Truncate" :disabled="!hasSelection" @click="handleTruncateSelected"><Scissors :size="15" /></button>
            <button class="tbl-btn warn" title="Empty Table" :disabled="!hasSelection" @click="handleEmptySelected"><Eraser :size="15" /></button>
            <button class="tbl-btn danger" title="Drop Table" :disabled="!hasSelection" @click="handleDropSelected"><Trash2 :size="15" /></button>
          </div>
          <div class="search-wrap">
            <Search class="search-icon" :size="13" />
            <input v-model="searchQuery" type="text" placeholder="Filter tables…" class="search-input" />
          </div>
        </div>

        <div class="grid-scroll">
          <table class="data-grid">
            <thead>
              <tr>
                <th>Name</th>
                <th>Engine</th>
                <th class="col-num">Rows</th>
                <th class="col-num">Size</th>
                <th>Modified</th>
                <th>Collation</th>
                <th>Comment</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="(row, idx) in filteredRows"
                :key="idx"
                :class="{ selected: selectedRows.includes(row), 'context-row': contextRow === row }"
                @click="(e) => selectRow(row, e)"
                @dblclick="handleRowDoubleClick(row)"
                @contextmenu="showContextMenu($event, row)"
              >
                <td>
                  <div class="name-cell">
                    <Table :size="13" class="text-accent flex-shrink-0" />
                    <span class="name-text">{{ row['Name'] }}</span>
                  </div>
                </td>
                <td>
                  <span v-if="row['Engine']" class="engine-badge" :style="{ borderColor: engineColor(row['Engine']), color: engineColor(row['Engine']) }">
                    {{ row['Engine'] }}
                  </span>
                </td>
                <td class="col-num text-muted">{{ row['Rows'] != null ? Number(row['Rows']).toLocaleString() : '' }}</td>
                <td class="col-num text-muted">{{ formatValue('Data Length', row['Data Length']) }}</td>
                <td class="text-muted">{{ formatValue('Modified At', row['Modified At']) }}</td>
                <td class="text-muted collation-col">{{ row['Collation'] || '' }}</td>
                <td class="text-muted" style="max-width: 200px; font-size: 0.72rem;">{{ row['Comment'] || '' }}</td>
              </tr>
            </tbody>
          </table>
        </div>

        <div class="tbl-footer">
          {{ filteredRows.length }} tables{{ searchQuery ? ` (filtered)` : '' }}{{ selectedRows.length > 0 ? ` · ${selectedRows.length} selected` : '' }}
        </div>
      </div>
    </template>

    <TableContextMenu
      :visible="menuVisible"
      :x="menuPos.x"
      :y="menuPos.y"
      @generate-query="handleGenerateQuery"
      @view-first="handleViewFirstPage"
      @view-last="handleViewLastPage"
      @view-structure="handleViewStructure"
      @edit-table="() => handleEditTable(contextRow)"
      @import="handleImportContext"
      @export="handleExportContext"
      @empty="handleEmptyTable"
      @truncate="handleTruncateTable"
      @drop="handleDropTable"
    />
  </div>
</template>

<style scoped>
@import './table-list-shared.css';

.engine-badge {
  font-size: 0.65rem;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 10px;
  border: 1px solid;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  white-space: nowrap;
}

.col-num { text-align: right; width: 80px; }
.collation-col { font-size: 0.72rem; font-family: var(--font-mono); }
</style>
