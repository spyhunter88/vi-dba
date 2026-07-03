<script setup lang="ts">
import { toRef } from 'vue';
import { Scaling, AlertCircle, Plus, Edit, Table, RefreshCw, Search, MessageSquare, Upload, Download, Scissors, Eraser, Trash2 } from 'lucide-vue-next';
import { useTableList } from './useTableList';
import TableContextMenu from './TableContextMenu.vue';

const props = defineProps<{ tabId: string }>();
const {
  result, filteredRows, searchQuery, selectedRows, hasSelection, isSingleSelection,
  menuVisible, menuPos, contextRow,
  tab, connectionName,
  selectRow, handleRowDoubleClick, handleCreateTable, handleEditTable,
  showContextMenu,
  handleViewFirstPage, handleViewLastPage, handleViewStructure,
  handleEmptyTable, handleTruncateTable, handleDropTable,
  handleImportContext, handleExportContext, handleGenerateQuery,
  handleImportSelected, handleExportSelected,
  handleEmptySelected, handleTruncateSelected, handleDropSelected,
  reload
} = useTableList(toRef(props, 'tabId'));
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
                <th class="col-num">Rows</th>
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
                <td class="col-num text-muted">{{ row['Rows'] ?? '' }}</td>
                <td>
                  <div v-if="row['Comment']" class="comment-cell">
                    <MessageSquare :size="11" class="comment-icon" />
                    <span class="name-text">{{ row['Comment'] }}</span>
                  </div>
                </td>
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

.comment-cell {
  display: flex;
  align-items: center;
  gap: 5px;
  color: var(--text-secondary);
  font-size: 0.75rem;
  opacity: 0.75;
  overflow: hidden;
}
.comment-icon { opacity: 0.5; flex-shrink: 0; }
.col-num { text-align: right; width: 70px; }
</style>
