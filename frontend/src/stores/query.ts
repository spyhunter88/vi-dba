import { defineStore } from 'pinia';
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { QueryHistoryEntry, ScriptInfo } from '../types';
import { useConnectionStore } from './connections';
import { useTabStore } from './tabs';
import { useUiStore } from './ui';

export const useQueryStore = defineStore('query', () => {
  const connectionStore = useConnectionStore();
  const tabStore = useTabStore();
  const uiStore = useUiStore();

  const queryResults = ref<Record<string, { loading: boolean, results: any[], error: string | null, messages: string[], totalTimeMs?: number, execId?: string, startTime?: number, elapsedMs?: number }>>({});

  const elapsedTimers: Record<string, number> = {};

  function startElapsedTimer(tabId: string) {
    stopElapsedTimer(tabId);
    elapsedTimers[tabId] = window.setInterval(() => {
      const r = queryResults.value[tabId];
      if (r && r.loading && r.startTime !== undefined) {
        r.elapsedMs = Date.now() - r.startTime;
      } else {
        stopElapsedTimer(tabId);
      }
    }, 100);
  }

  function stopElapsedTimer(tabId: string) {
    if (elapsedTimers[tabId]) {
      clearInterval(elapsedTimers[tabId]);
      delete elapsedTimers[tabId];
    }
  }

  async function cancelQuery(tabId: string) {
    const state = queryResults.value[tabId];
    if (!state?.loading || !state.execId) return;
    try {
      await invoke('cancel_query', { execId: state.execId });
    } catch (e) {
      console.error('Failed to cancel query:', e);
    }
  }

  function addTabMessage(tabId: string, message: string) {
    if (!queryResults.value[tabId]) {
      queryResults.value[tabId] = { loading: false, results: [], error: null, messages: [] };
    }
    const timeStr = new Date().toLocaleTimeString([], { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' });
    queryResults.value[tabId].messages.push(`[${timeStr}] ${message}`);
  }

  async function executeQueryInTab(tabId: string, selection?: string) {
    console.log(`[queryStore] executeQueryInTab called for ${tabId}. Selection: ${!!selection}`);
    const tab = tabStore.tabs.find(t => t.id === tabId);

    if (!tab) {
      console.warn(`[queryStore] executeQueryInTab abort: tab ${tabId} not found.`);
      return;
    }

    if (!tab.content && tab.type === 'sql_query') {
      console.warn(`[queryStore] executeQueryInTab abort: query tab ${tab.title} has no content.`);
      return;
    }

    if (!tab.connectionId) {
      console.warn(`[queryStore] executeQueryInTab abort: tab ${tab.title} has no connectionId.`);
      return;
    }

    if (queryResults.value[tabId]?.loading) {
      console.log(`[queryStore] executeQueryInTab skip: ${tabId} is already loading.`);
      return;
    }

    let queryToRun = selection || tab.content || '';

    const conn = connectionStore.connections.find(c => c.id === tab.connectionId);
    // Database context priority (per docs/overview.md):
    //   1. tab.database  — set when tab is opened from a specific database node in the sidebar
    //   2. conn.database — the connection's default database (configured at connection setup)
    //   3. null          — let the RDBMS use its own default (driver-specific behaviour)
    const dbContext = tab.database || conn?.database || null;
    // Schema context: tab.schema is seeded from the sidebar's activeSchema when the tab is
    // created; no connection-level schema fallback exists (ConnectionConfig has no schema field).
    const schemaContext = tab.schema || null;

    if (tab.type === 'table_data' && tab.metadata?.tableName) {
      const pagination = tab.pagination || { page: 1, pageSize: 1000 };
      const offset = (pagination.page - 1) * pagination.pageSize;
      const qualifiedName = getQualifiedName(tab.connectionId, tab.metadata.tableName, dbContext || undefined, schemaContext || undefined);
      queryToRun = `SELECT * FROM ${qualifiedName} LIMIT ${pagination.pageSize} OFFSET ${offset}`;
      console.log(`[queryStore] Table data query for "${tab.metadata.tableName}": ${queryToRun} (Context: db=${dbContext}, sc=${schemaContext})`);

      // Proactively fetch total rows if missing
      if (pagination.total === undefined) {
        fetchTotalRows(tabId);
      }
    }

    if (!queryToRun.trim()) {
      console.warn(`[queryStore] Skipping execution for tab ${tabId}: query is empty.`);
      return;
    }

    try {
      console.log(`[queryStore] Executing in tab ${tabId} on ${tab.connectionId}... (Context: db=${dbContext}, sc=${schemaContext})`);

      if (!connectionStore.connectedIds.has(tab.connectionId)) {
        await connectionStore.connect(tab.connectionId);
      }

      console.log(`[queryStore] Setting loading: true for tab ${tabId}`);
      const execId = `${tabId}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
      const startTime = Date.now();
      queryResults.value[tabId] = {
        loading: true,
        results: [],
        error: null,
        messages: queryResults.value[tabId]?.messages || [],
        execId,
        startTime,
        elapsedMs: 0
      };
      startElapsedTimer(tabId);

      const statements = queryToRun.split(';').map(s => s.trim()).filter(s => s.length > 0);
      const allResults: any[] = [];
      let totalAffected = 0;

      for (const stmt of statements) {
        const res = await invoke<any>('execute_query', {
          id: tab.connectionId,
          query: stmt,
          tableName: tab.metadata?.tableName || null,
          database: dbContext,
          schema: schemaContext,
          execId
        });
        res.query = stmt;
        allResults.push(res);
        totalAffected += (res.affectedRows || 0);
        console.log(`[queryStore] Statement executed. Rows: ${res.rows?.length || 0}, Affected: ${res.affectedRows || 0}`);
      }

      stopElapsedTimer(tabId);
      const durationMs = Date.now() - startTime;
      let totalRows = totalAffected;
      // If it's only SELECT statements, or includes SELECT, sum up the rows
      if (allResults.length > 0) {
        const selectRows = allResults.reduce((acc, r) => acc + (r.rows?.length || 0), 0);
        if (selectRows > 0) totalRows = selectRows;
      }

      queryResults.value[tabId] = {
        loading: false,
        results: allResults,
        error: null,
        messages: queryResults.value[tabId]?.messages || [],
        totalTimeMs: durationMs,
        elapsedMs: durationMs
      };

      await addQueryHistory({
        id: Math.random().toString(36).substring(2, 9),
        query: queryToRun,
        timestamp: new Date().toISOString(),
        connectionId: tab.connectionId,
        database: tab.database,
        schema: tab.schema,
        durationMs,
        status: 'success',
        affectedRows: totalRows,
        scriptId: tab.id,
      });
    } catch (e: any) {
      stopElapsedTimer(tabId);
      queryResults.value[tabId] = { loading: false, results: [], error: e.toString(), messages: queryResults.value[tabId]?.messages || [] };
      await addQueryHistory({
        id: Math.random().toString(36).substring(2, 9),
        query: queryToRun,
        timestamp: new Date().toISOString(),
        connectionId: tab.connectionId,
        database: tab.database,
        schema: tab.schema,
        durationMs: 0,
        status: 'error',
        affectedRows: 0,
        errorMessage: e.toString(),
        scriptId: tab.id,
      });
    }
  }

  async function addQueryHistory(entry: QueryHistoryEntry) {
    try {
      await invoke('add_query_history', { entry });
    } catch (e) {
      console.error('Failed to log query history:', e);
    }
  }

  async function getQueryHistory() {
    try {
      return await invoke<QueryHistoryEntry[]>('get_query_history');
    } catch (e) {
      console.error('Failed to get query history:', e);
      return [];
    }
  }

  async function clearQueryHistory() {
    try {
      await invoke('clear_query_history');
    } catch (e) {
      console.error('Failed to clear query history:', e);
    }
  }

  function quoteIdentifier(connectionId: string, name: string): string {
    const conn = connectionStore.connections.find(c => c.id === connectionId);
    if (!name) return '';
    if (!conn) return `"${name}"`;

    const dbType = conn.dbType;
    if (dbType === 'mySQL') {
      return `\`${name}\``;
    } else if (dbType === 'sqlServer') {
      return `[${name}]`;
    } else {
      return `"${name}"`;
    }
  }

  function getQualifiedName(connectionId: string, name: string, catalog?: string, schema?: string): string {
    const parts: string[] = [];
    if (catalog) parts.push(quoteIdentifier(connectionId, catalog));
    if (schema) parts.push(quoteIdentifier(connectionId, schema));
    parts.push(quoteIdentifier(connectionId, name));
    return parts.join('.');
  }

  async function executeRawQuery(connectionId: string, query: string, successToast?: string, database?: string, schema?: string) {
    try {
      await invoke('execute_query', {
        id: connectionId,
        query,
        tableName: null,
        database: database || null,
        schema: schema || null
      });
      if (successToast) {
        uiStore.showToast(successToast);
      }
    } catch (e: any) {
      console.error('Failed to execute raw query:', e);
      let errorMsg = e.toString();
      if (typeof e === 'object' && e.message) errorMsg = e.message;
      uiStore.showToast(errorMsg, 'error');
      throw e;
    }
  }

  async function changePage(tabId: string, page: number) {
    const tab = tabStore.tabs.find(t => t.id === tabId);
    if (!tab || !tab.pagination) return;
    tab.pagination.page = page;
    await executeQueryInTab(tabId);
  }

  async function goToFirstPage(tabId: string) {
    await changePage(tabId, 1);
  }

  async function goToLastPage(tabId: string) {
    const tab = tabStore.tabs.find(t => t.id === tabId);
    if (!tab || !tab.pagination || !tab.pagination.total) {
      // If total is missing, fetch it first
      await fetchTotalRows(tabId);
    }

    if (tab?.pagination?.total) {
      const lastPage = Math.ceil(tab.pagination.total / tab.pagination.pageSize);
      await changePage(tabId, Math.max(1, lastPage));
    }
  }

  async function fetchTotalRows(tabId: string) {
    const tab = tabStore.tabs.find(t => t.id === tabId);
    if (!tab || tab.type !== 'table_data' || !tab.metadata?.tableName) return;

    const conn = connectionStore.connections.find(c => c.id === tab.connectionId);
    const dbContext = tab.database || conn?.database || null;
    const schemaContext = tab.schema || null;
    const qualifiedName = getQualifiedName(tab.connectionId, tab.metadata.tableName, dbContext || undefined, schemaContext || undefined);

    try {
      const res = await invoke<any>('execute_query', {
        id: tab.connectionId,
        query: `SELECT COUNT(*) as total FROM ${qualifiedName}`,
        tableName: null,
        database: dbContext,
        schema: schemaContext
      });

      if (res && res.rows && res.rows.length > 0) {
        const total = parseInt(res.rows[0].total || res.rows[0].TOTAL || 0, 10);
        if (tab.pagination) {
          tab.pagination.total = total;
        } else {
          tab.pagination = { page: 1, pageSize: 1000, total };
        }
      }
    } catch (e) {
      console.error('Failed to fetch total rows:', e);
    }
  }

  async function updateCellValue(tabId: string, row: any, column: string, newValue: any) {
    const tab = tabStore.tabs.find(t => t.id === tabId);
    const result = queryResults.value[tabId]?.results?.[0];

    if (!tab || !result || !result.primaryKeys || result.primaryKeys.length === 0 || !result.tableName) {
      console.warn('Cannot update cell: missing metadata');
      return;
    }

    const pks: Record<string, any> = {};
    result.primaryKeys.forEach((pk: string) => {
      pks[pk] = row[pk];
    });

    try {
      const res = await invoke<any>('update_row', {
        id: tab.connectionId,
        tableName: result.tableName,
        pks,
        column,
        value: newValue,
        catalog: tab.database || null,
        schema: tab.schema || null
      });
      row[column] = newValue;
      uiStore.showToast('Cell updated successfully');
      if (res && res.query) {
        addTabMessage(tabId, res.query);
      }
    } catch (e: any) {
      console.error('Failed to update cell:', e);
      uiStore.showToast(e.toString() || 'Failed to update cell', 'error');
      throw e;
    }
  }

  async function insertRow(tabId: string, data: Record<string, any>) {
    const tab = tabStore.tabs.find(t => t.id === tabId);
    const result = queryResults.value[tabId]?.results?.[0];
    const connId = tab?.connectionId;
    const tableName = result?.tableName;

    if (!connId || !tableName) return;

    const filteredData: Record<string, any> = {};
    Object.entries(data).forEach(([key, value]) => {
      if (value !== '' && value !== null && value !== undefined) {
        filteredData[key] = value;
      }
    });

    try {
      const res = await invoke<any>('insert_row', {
        id: connId,
        tableName,
        data: filteredData,
        catalog: tab?.database || null,
        schema: tab?.schema || null
      });
      await executeQueryInTab(tabId);
      uiStore.showToast('Row inserted successfully');
      if (res && res.query) {
        addTabMessage(tabId, res.query);
      }
    } catch (e: any) {
      console.error('Failed to insert row:', e);
      uiStore.showToast(e.toString() || 'Failed to insert row', 'error');
      throw e;
    }
  }

  async function loadTableList(tabId: string) {
    const tab = tabStore.tabs.find(t => t.id === tabId);
    if (!tab || tab.type !== 'table_list') return;

    const conn = connectionStore.connections.find(c => c.id === tab.connectionId);
    const catalog = tab.database || tab.metadata?.catalog || conn?.database || null;
    const schema = tab.schema || tab.metadata?.schema || null;

    try {
      if (!connectionStore.connectedIds.has(tab.connectionId)) {
        await connectionStore.connect(tab.connectionId);
      }
      queryResults.value[tabId] = { loading: true, results: [], error: null, messages: [] };
      const result = await invoke('get_table_list', {
        id: tab.connectionId,
        catalog,
        schema
      });
      queryResults.value[tabId] = { loading: false, results: [result], error: null, messages: queryResults.value[tabId]?.messages || [] };
    } catch (e: any) {
      queryResults.value[tabId] = { loading: false, results: [], error: e.toString(), messages: queryResults.value[tabId]?.messages || [] };
    }
  }

  async function loadRoutineList(tabId: string) {
    const tab = tabStore.tabs.find(t => t.id === tabId);
    if (!tab || tab.type !== 'routine_list') return;

    const conn = connectionStore.connections.find(c => c.id === tab.connectionId);
    const catalog = tab.database || tab.metadata?.catalog || conn?.database || null;
    const schema = tab.schema || tab.metadata?.schema || null;

    try {
      if (!connectionStore.connectedIds.has(tab.connectionId)) {
        await connectionStore.connect(tab.connectionId);
      }
      queryResults.value[tabId] = { loading: true, results: [], error: null, messages: [] };
      const result = await invoke('get_routine_list', {
        id: tab.connectionId,
        catalog,
        schema
      });
      queryResults.value[tabId] = { loading: false, results: [result], error: null, messages: queryResults.value[tabId]?.messages || [] };
    } catch (e: any) {
      queryResults.value[tabId] = { loading: false, results: [], error: e.toString(), messages: queryResults.value[tabId]?.messages || [] };
    }
  }

  async function loadScripts(connectionId: string, database?: string, schema?: string) {
    try {
      return await invoke<ScriptInfo[]>('list_scripts', { connectionId, database, schema });
    } catch (e) {
      console.error('Failed to load scripts:', e);
      throw e;
    }
  }

  async function generateTableQuery(connectionId: string, tableName: string, queryType: 'SELECT' | 'INSERT' | 'UPDATE' | 'DELETE', catalog?: string, schema?: string) {
    try {
      const def: any = await invoke('get_table_definition', { id: connectionId, tableName, catalog, schema });
      const qualifiedName = getQualifiedName(connectionId, tableName, catalog, schema);
      let content = '';

      let where = 'id = ?';
      if (def && def.columns && def.columns.some((c: any) => c.isPrimaryKey)) {
        where = def.columns
          .filter((c: any) => c.isPrimaryKey)
          .map((c: any) => `${quoteIdentifier(connectionId, c.name)} = ?`)
          .join(' AND ');
      }

      if (queryType === 'SELECT') {
        content = `SELECT * FROM ${qualifiedName} \nWHERE ${where};`;
      } else if (queryType === 'INSERT') {
        const cols = def.columns.map((c: any) => quoteIdentifier(connectionId, c.name)).join(', ');
        const placeholders = def.columns.map(() => '?').join(', ');
        content = `INSERT INTO ${qualifiedName} (${cols}) \nVALUES (${placeholders});`;
      } else if (queryType === 'UPDATE') {
        content = `UPDATE ${qualifiedName} \nSET \nWHERE ${where};`;
      } else if (queryType === 'DELETE') {
        content = `DELETE FROM ${qualifiedName} \nWHERE ${where};`;
      }

      tabStore.addTab({
        id: `query-${connectionId}-${Date.now()}`,
        title: `${queryType}: ${tableName}`,
        type: 'sql_query',
        connectionId,
        database: catalog,
        schema,
        content,
        isDirty: false
      });
    } catch (e: any) {
      console.error('Failed to generate table query:', e);
      uiStore.showToast('Failed to generate query: ' + e.toString(), 'error');
    }
  }

  return {
    queryResults,
    addTabMessage,
    executeQueryInTab,
    cancelQuery,
    addQueryHistory,
    getQueryHistory,
    clearQueryHistory,
    quoteIdentifier,
    getQualifiedName,
    executeRawQuery,
    changePage,
    updateCellValue,
    insertRow,
    loadTableList,
    loadRoutineList,
    loadScripts,
    generateTableQuery,
    goToFirstPage,
    goToLastPage,
    fetchTotalRows
  };
});
