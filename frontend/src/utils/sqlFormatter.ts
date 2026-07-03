import * as monaco from 'monaco-editor';
import { format } from 'sql-formatter';
import type { DbType } from '../types';
import { useTabStore } from '../stores/tabs';
import { useConnectionStore } from '../stores/connections';

export function getFormatterLanguage(dbType: DbType): string {
  switch (dbType) {
    case 'mySQL':
      return 'mysql';
    case 'postgreSQL':
      return 'postgresql';
    case 'sqlServer':
      return 'transactsql';
    case 'sqlite':
      return 'sqlite';
    case 'oracle':
      return 'plsql';
    default:
      return 'sql';
  }
}

export function formatSql(sql: string, dbType: DbType): string {
  try {
    return format(sql, {
      language: getFormatterLanguage(dbType) as any,
      tabWidth: 2,
      useTabs: false,
      keywordCase: 'upper',
    });
  } catch (err) {
    console.error('SQL Formatting failed, falling back to original SQL:', err);
    return sql;
  }
}

let formattingProviderRegistered = false;

export function registerGlobalFormattingProvider() {
  if (formattingProviderRegistered) return;
  formattingProviderRegistered = true;

  monaco.languages.registerDocumentFormattingEditProvider('sql', {
    provideDocumentFormattingEdits(model) {
      try {
        const tabStore = useTabStore();
        const connectionStore = useConnectionStore();
        
        // Find the active tab to extract the appropriate dbType
        const activeTab = tabStore.tabs.find(t => t.id === tabStore.activeTabId);
        if (!activeTab) return [];

        const connection = connectionStore.connections.find(c => c.id === activeTab.connectionId);
        const dbType = connection?.dbType || 'postgreSQL';

        const formatted = formatSql(model.getValue(), dbType);
        return [
          {
            range: model.getFullModelRange(),
            text: formatted,
          },
        ];
      } catch (err) {
        console.error('Failed to format document in global formatting provider:', err);
        return [];
      }
    }
  });
}
