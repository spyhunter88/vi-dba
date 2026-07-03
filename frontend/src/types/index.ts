export type DbType = 'mySQL' | 'postgreSQL' | 'sqlServer' | 'sqlite' | 'oracle' | 'mongoDB';

export interface ConnectionConfig {
  id: string;
  name: string;
  dbType: DbType;
  host: string;
  port: number;
  user: string;
  password?: string;
  database?: string;
  sslMode?: string;
  serverVersion?: string;
  group?: string;       // project / product label
  environment?: string; // staging label (dev, test, beta, prod, …)
}

export interface ConnectResult {
  serverVersion?: string;
}

export interface QueryResult {
  columns: string[];
  columnTypes: string[];
  rows: any[];
  affectedRows: number;
  executionTimeMs: number;
  primaryKeys: string[];
  tableName?: string;
}

export interface ColumnInfo {
  name: string;
  dataType: string;
}

export interface DbObject {
  name: string;
  objectType: 'database' | 'schema' | 'table' | 'view' | 'function' | 'procedure' | 'category';
  schema?: string;
  catalog?: string;
  parent?: string;
  columns?: ColumnInfo[];
}

export type TabType = 'sql_query' | 'ai_sql' | 'table_data' | 'table_structure' | 'table_list' | 'routine_list' | 'script_list' | 'routine_editor' | 'view_editor' | 'ai_routine_editor';

export interface ScriptInfo {
  name: string;
  path: string;
  database?: string;
  schema?: string;
  createdAt: string;
  modifiedAt: string;
}

export interface Tab {
  id: string;
  title: string;
  type: TabType;
  connectionId: string;
  database?: string;
  schema?: string;
  content?: string; // For SQL query
  filePath?: string; // For script path
  metadata?: any; // For table data (e.g. table name)
  isDirty?: boolean;
  showDetail?: boolean;
  aiPromptOpen?: boolean;
  aiPromptContent?: string;
  aiLoading?: boolean;
  pagination?: {
    page: number;
    pageSize: number;
    total?: number;
  };
  isDetached?: boolean;
}

export interface TableColumn {
  name: string;
  dataType: string;
  isNullable: boolean;
  isPrimaryKey: boolean;
  isAutoIncrement: boolean;
  defaultValue: string | null;
  comment: string | null;
  length: string | null;
  collation?: string | null;
}

export interface TableDefinition {
  name: string;
  columns: TableColumn[];
  catalog?: string;
  schema?: string;
  comment: string | null;
  collation?: string | null;
}

export interface RoutineDefinition {
  name: string;
  routineType: string;
  definition: string;
  catalog?: string;
  schema?: string;
}

export interface ViewDefinition {
  name: string;
  definition: string;
  catalog?: string;
  schema?: string;
}

export interface QueryHistoryEntry {
  id: string;
  query: string;
  timestamp: string;
  connectionId: string;
  database?: string;
  schema?: string;
  durationMs: number;
  status: 'success' | 'error';
  affectedRows: number;
  scriptId?: string;
  errorMessage?: string;
}

export interface TabState {
  id: string;
  title: string;
  tabType: string;
  connectionId: string;
  database?: string;
  schema?: string;
  content?: string;
  filePath?: string;
  metadata?: any;
  pagination?: any;
  cursorPosition?: any;
}

export interface SessionState {
  tabs: TabState[];
  activeTabId?: string;
}
