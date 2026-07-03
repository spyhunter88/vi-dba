import { computed, ref, nextTick, onMounted } from 'vue';
import type { Ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { useConnectionStore } from '../../../stores/connections';
import { useTabStore } from '../../../stores/tabs';
import { useQueryStore } from '../../../stores/query';
import { useUiStore } from '../../../stores/ui';

export function useTableList(tabId: Ref<string>) {
  const connectionStore = useConnectionStore();
  const tabStore = useTabStore();
  const queryStore = useQueryStore();
  const uiStore = useUiStore();

  const result = computed(() => queryStore.queryResults[tabId.value]);
  const displayRows = computed(() => result.value?.results?.[0]?.rows || []);
  const displayColumns = computed(() => result.value?.results?.[0]?.columns || []);

  const searchQuery = ref('');
  const selectedRows = ref<any[]>([]);
  const menuVisible = ref(false);
  const menuPos = ref({ x: 0, y: 0 });
  const contextRow = ref<any>(null);

  const filteredRows = computed(() => {
    if (!searchQuery.value) return displayRows.value;
    const q = searchQuery.value.toLowerCase();
    return displayRows.value.filter((row: any) =>
      String(row['Name'] || '').toLowerCase().includes(q)
    );
  });

  const tab = computed(() => tabStore.tabs.find((t: any) => t.id === tabId.value));
  const connection = computed(() => connectionStore.connections.find((c: any) => c.id === tab.value?.connectionId));
  const connectionName = computed(() => connection.value?.name || 'No Connection');
  const dbType = computed(() => connection.value?.dbType);

  const hasSelection = computed(() => selectedRows.value.length > 0);
  const isSingleSelection = computed(() => selectedRows.value.length === 1);

  function formatValue(col: string, val: any): string {
    if (val === null || val === undefined) return '';
    if (col === 'Data Length') {
      const n = Number(val);
      if (!isNaN(n)) {
        if (n > 1024 * 1024) return (n / (1024 * 1024)).toFixed(2) + ' MB';
        if (n > 1024) return (n / 1024).toFixed(2) + ' KB';
        return n + ' B';
      }
    }
    if (/created|modified|updated/i.test(col)) {
      try {
        const d = new Date(val);
        if (!isNaN(d.getTime())) return d.toLocaleString();
      } catch { /* ignore */ }
    }
    return val;
  }

  function selectRow(row: any, event?: MouseEvent) {
    if (event?.ctrlKey || event?.metaKey) {
      const idx = selectedRows.value.indexOf(row);
      if (idx >= 0) {
        selectedRows.value = selectedRows.value.filter((r) => r !== row);
      } else {
        selectedRows.value = [...selectedRows.value, row];
      }
    } else {
      selectedRows.value = [row];
    }
  }

  function handleRowDoubleClick(row: any) {
    const tableName = row['Name'];
    if (!tableName || !tab.value?.connectionId) return;
    tabStore.addTab({
      id: `table-${tab.value.connectionId}-${tableName}`,
      title: tableName,
      type: 'table_data',
      connectionId: tab.value.connectionId,
      database: tab.value.metadata?.catalog,
      schema: tab.value.metadata?.schema,
      metadata: { tableName, catalog: tab.value.metadata?.catalog, schema: tab.value.metadata?.schema }
    });
  }

  function handleCreateTable() {
    if (tab.value?.connectionId) {
      tabStore.createTable(tab.value.connectionId, tab.value.metadata?.catalog, tab.value.metadata?.schema);
    }
  }

  function handleEditTable(row?: any) {
    const target = row || selectedRows.value[0];
    const tableName = target?.['Name'];
    if (tab.value?.connectionId && tableName) {
      tabStore.editTable(tab.value.connectionId, tableName, tab.value.metadata?.catalog, tab.value.metadata?.schema);
    }
  }

  function showContextMenu(e: MouseEvent, row: any) {
    e.preventDefault();
    contextRow.value = row;
    if (!selectedRows.value.includes(row)) {
      selectedRows.value = [row];
    }
    menuPos.value = { x: e.clientX, y: e.clientY };
    menuVisible.value = true;
    window.addEventListener('click', closeMenu, { once: true });
  }

  function closeMenu() {
    menuVisible.value = false;
    contextRow.value = null;
  }

  function _openTableTab(tableName: string) {
    if (!tab.value?.connectionId) return null;
    const tabId2 = `table-${tab.value.connectionId}-${tableName}`;
    tabStore.addTab({
      id: tabId2,
      title: tableName,
      type: 'table_data',
      connectionId: tab.value.connectionId,
      database: tab.value.metadata?.catalog,
      schema: tab.value.metadata?.schema,
      metadata: { tableName, catalog: tab.value.metadata?.catalog, schema: tab.value.metadata?.schema },
      pagination: { page: 1, pageSize: 1000 }
    });
    return tabId2;
  }

  function handleViewFirstPage() {
    if (!contextRow.value) return;
    const id = _openTableTab(contextRow.value['Name']);
    if (id) queryStore.goToFirstPage(id);
    closeMenu();
  }

  async function handleViewLastPage() {
    if (!contextRow.value) return;
    const id = _openTableTab(contextRow.value['Name']);
    if (id) { await nextTick(); queryStore.goToLastPage(id); }
    closeMenu();
  }

  function handleViewStructure() {
    if (!contextRow.value || !tab.value?.connectionId) return;
    tabStore.editTable(tab.value.connectionId, contextRow.value['Name'], tab.value.metadata?.catalog, tab.value.metadata?.schema);
    closeMenu();
  }

  // Context menu handlers — operate on the right-clicked row (contextRow)
  async function handleEmptyTable() {
    if (!contextRow.value || !tab.value?.connectionId) return;
    const tableName = contextRow.value['Name'];
    const connId = tab.value.connectionId;
    const qualified = queryStore.getQualifiedName(connId, tableName, tab.value.metadata?.catalog, tab.value.metadata?.schema);
    uiStore.showConfirm('Empty Table', `Are you sure you want to EMPTY table "${tableName}"? This will delete ALL rows.`, 'warning', async () => {
      try {
        await queryStore.executeRawQuery(connId, `DELETE FROM ${qualified}`, `Table "${tableName}" emptied.`);
        await connectionStore.refreshObjects(connId);
        queryStore.loadTableList(tabId.value);
      } catch { /* error shown by executeRawQuery */ }
    });
    closeMenu();
  }

  async function handleTruncateTable() {
    if (!contextRow.value || !tab.value?.connectionId) return;
    const tableName = contextRow.value['Name'];
    const connId = tab.value.connectionId;
    const qualified = queryStore.getQualifiedName(connId, tableName, tab.value.metadata?.catalog, tab.value.metadata?.schema);
    uiStore.showConfirm('Truncate Table', `Are you sure you want to TRUNCATE table "${tableName}"?`, 'warning', async () => {
      try {
        await queryStore.executeRawQuery(connId, `TRUNCATE TABLE ${qualified}`, `Table "${tableName}" truncated.`);
        await connectionStore.refreshObjects(connId);
        queryStore.loadTableList(tabId.value);
      } catch { /* error shown */ }
    });
    closeMenu();
  }

  async function handleDropTable() {
    if (!contextRow.value || !tab.value?.connectionId) return;
    const tableName = contextRow.value['Name'];
    const connId = tab.value.connectionId;
    const qualified = queryStore.getQualifiedName(connId, tableName, tab.value.metadata?.catalog, tab.value.metadata?.schema);
    uiStore.showConfirm('Drop Table', `Are you sure you want to DROP table <strong class="confirm-name">${tableName}</strong>? THIS CANNOT BE UNDONE.`, 'danger', async () => {
      try {
        await queryStore.executeRawQuery(connId, `DROP TABLE ${qualified}`, `Table "${tableName}" dropped.`);
        await connectionStore.refreshObjects(connId);
        queryStore.loadTableList(tabId.value);
      } catch { /* error shown */ }
    });
    closeMenu();
  }

  function handleImportContext() {
    if (!contextRow.value || !tab.value?.connectionId) return;
    invoke('open_import', {
      connectionId: tab.value.connectionId,
      catalog: tab.value.metadata?.catalog,
      schema: tab.value.metadata?.schema,
      tableName: contextRow.value['Name']
    });
    closeMenu();
  }

  function handleExportContext() {
    if (!contextRow.value || !tab.value?.connectionId) return;
    invoke('open_export', {
      connectionId: tab.value.connectionId,
      sourceType: 'table',
      sourceName: contextRow.value['Name'],
      catalog: tab.value.metadata?.catalog,
      schema: tab.value.metadata?.schema
    });
    closeMenu();
  }

  function handleGenerateQuery(type: 'SELECT' | 'INSERT' | 'UPDATE' | 'DELETE') {
    if (!contextRow.value || !tab.value?.connectionId) return;
    queryStore.generateTableQuery(
      tab.value.connectionId,
      contextRow.value['Name'],
      type,
      tab.value.metadata?.catalog,
      tab.value.metadata?.schema
    );
    closeMenu();
  }

  // Toolbar handlers — operate on all selectedRows (multi-table)
  function handleImportSelected() {
    if (selectedRows.value.length !== 1 || !tab.value?.connectionId) return;
    invoke('open_import', {
      connectionId: tab.value.connectionId,
      catalog: tab.value.metadata?.catalog,
      schema: tab.value.metadata?.schema,
      tableName: selectedRows.value[0]['Name']
    });
  }

  function handleExportSelected() {
    if (!selectedRows.value.length || !tab.value?.connectionId) return;
    const connId = tab.value.connectionId;
    const catalog = tab.value.metadata?.catalog;
    const schema = tab.value.metadata?.schema;
    if (selectedRows.value.length === 1) {
      invoke('open_export', {
        connectionId: connId,
        sourceType: 'table',
        sourceName: selectedRows.value[0]['Name'],
        catalog,
        schema
      });
    } else {
      invoke('open_export_multi', {
        connectionId: connId,
        sourceTables: selectedRows.value.map((r: any) => r['Name'] as string),
        catalog,
        schema
      });
    }
  }

  async function handleEmptySelected() {
    if (!selectedRows.value.length || !tab.value?.connectionId) return;
    const names = selectedRows.value.map((r: any) => r['Name'] as string);
    const connId = tab.value.connectionId;
    const catalog = tab.value.metadata?.catalog;
    const schema = tab.value.metadata?.schema;
    const label = names.length === 1 ? `"${names[0]}"` : `${names.length} tables`;
    uiStore.showConfirm('Empty Tables', `Delete ALL rows from ${label}?`, 'warning', async () => {
      try {
        for (const name of names) {
          const qualified = queryStore.getQualifiedName(connId, name, catalog, schema);
          await queryStore.executeRawQuery(connId, `DELETE FROM ${qualified}`);
        }
        uiStore.showToast(`${names.length} table(s) emptied.`, 'success');
        await connectionStore.refreshObjects(connId);
        queryStore.loadTableList(tabId.value);
      } catch { /* error shown */ }
    });
  }

  async function handleTruncateSelected() {
    if (!selectedRows.value.length || !tab.value?.connectionId) return;
    const names = selectedRows.value.map((r: any) => r['Name'] as string);
    const connId = tab.value.connectionId;
    const catalog = tab.value.metadata?.catalog;
    const schema = tab.value.metadata?.schema;
    const label = names.length === 1 ? `"${names[0]}"` : `${names.length} tables`;
    uiStore.showConfirm('Truncate Tables', `Truncate ${label}?`, 'warning', async () => {
      try {
        for (const name of names) {
          const qualified = queryStore.getQualifiedName(connId, name, catalog, schema);
          await queryStore.executeRawQuery(connId, `TRUNCATE TABLE ${qualified}`);
        }
        uiStore.showToast(`${names.length} table(s) truncated.`, 'success');
        await connectionStore.refreshObjects(connId);
        queryStore.loadTableList(tabId.value);
      } catch { /* error shown */ }
    });
  }

  async function handleDropSelected() {
    if (!selectedRows.value.length || !tab.value?.connectionId) return;
    const names = selectedRows.value.map((r: any) => r['Name'] as string);
    const connId = tab.value.connectionId;
    const catalog = tab.value.metadata?.catalog;
    const schema = tab.value.metadata?.schema;
    const nameHtml = names.length === 1
      ? `<strong class="confirm-name">${names[0]}</strong>`
      : `<strong class="confirm-name">${names.length} tables</strong><br><span class="confirm-name-list">${names.join(', ')}</span>`;
    uiStore.showConfirm('Drop Tables', `Drop ${nameHtml}? THIS CANNOT BE UNDONE.`, 'danger', async () => {
      try {
        for (const name of names) {
          const qualified = queryStore.getQualifiedName(connId, name, catalog, schema);
          await queryStore.executeRawQuery(connId, `DROP TABLE ${qualified}`);
        }
        uiStore.showToast(`${names.length} table(s) dropped.`, 'success');
        await connectionStore.refreshObjects(connId);
        queryStore.loadTableList(tabId.value);
      } catch { /* error shown */ }
    });
  }

  function reload() {
    selectedRows.value = [];
    queryStore.loadTableList(tabId.value);
  }

  onMounted(reload);

  return {
    result, displayRows, displayColumns, searchQuery, filteredRows,
    selectedRows, hasSelection, isSingleSelection,
    menuVisible, menuPos, contextRow,
    tab, connectionName, dbType,
    formatValue,
    selectRow, handleRowDoubleClick, handleCreateTable, handleEditTable,
    showContextMenu, closeMenu,
    handleViewFirstPage, handleViewLastPage, handleViewStructure,
    handleEmptyTable, handleTruncateTable, handleDropTable,
    handleImportContext, handleExportContext, handleGenerateQuery,
    handleImportSelected, handleExportSelected,
    handleEmptySelected, handleTruncateSelected, handleDropSelected,
    reload
  };
}
