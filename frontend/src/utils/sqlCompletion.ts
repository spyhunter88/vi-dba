import * as monaco from 'monaco-editor';
import type { DbType } from '../types';
import { SNIPPETS } from './sqlSnippets';

// ColumnInfo shape as stored in the schema cache
interface CachedColumn {
  name: string;
  dataType: string;
}

interface CachedObject {
  name: string;
  objectType: string;
  catalog?: string;
  schema?: string;
  columns?: CachedColumn[];
}

// SQL keywords + clause words
const SQL_KEYWORDS = [
  'SELECT', 'FROM', 'WHERE', 'GROUP BY', 'ORDER BY', 'LIMIT', 'OFFSET',
  'JOIN', 'LEFT JOIN', 'RIGHT JOIN', 'INNER JOIN', 'FULL JOIN', 'CROSS JOIN',
  'LEFT OUTER JOIN', 'RIGHT OUTER JOIN', 'FULL OUTER JOIN',
  'ON', 'AS', 'AND', 'OR', 'NOT', 'IN', 'NOT IN',
  'IS NULL', 'IS NOT NULL', 'IS TRUE', 'IS FALSE',
  'INSERT INTO', 'VALUES', 'UPDATE', 'SET', 'DELETE FROM',
  'TRUNCATE', 'DROP', 'DROP TABLE', 'DROP VIEW', 'DROP INDEX',
  'CREATE', 'CREATE TABLE', 'CREATE VIEW', 'CREATE INDEX', 'CREATE UNIQUE INDEX',
  'ALTER', 'ALTER TABLE', 'ADD COLUMN', 'DROP COLUMN', 'MODIFY COLUMN', 'RENAME COLUMN',
  'TABLE', 'DATABASE', 'SCHEMA', 'VIEW',
  'FUNCTION', 'PROCEDURE', 'TRIGGER', 'INDEX',
  'PRIMARY KEY', 'FOREIGN KEY', 'REFERENCES', 'UNIQUE', 'CHECK', 'DEFAULT',
  'CASE', 'WHEN', 'THEN', 'ELSE', 'END',
  'UNION', 'UNION ALL', 'INTERSECT', 'EXCEPT',
  'EXISTS', 'NOT EXISTS',
  'LIKE', 'NOT LIKE', 'ILIKE', 'BETWEEN', 'NOT BETWEEN',
  'HAVING', 'DISTINCT', 'ALL', 'ANY', 'SOME',
  'WITH', 'RECURSIVE', 'CTE',
  'EXPLAIN', 'ANALYZE', 'DESCRIBE', 'SHOW',
  'BEGIN', 'COMMIT', 'ROLLBACK', 'TRANSACTION', 'SAVEPOINT',
  'GRANT', 'REVOKE', 'LOCK', 'UNLOCK',
  'IF', 'IF NOT EXISTS', 'IF EXISTS',
  'AUTO_INCREMENT', 'AUTOINCREMENT', 'SERIAL',
  'NULL', 'TRUE', 'FALSE',
  'ASC', 'DESC', 'NULLS FIRST', 'NULLS LAST',
  'USING', 'NATURAL JOIN', 'STRAIGHT_JOIN',
];

// Common SQL built-in functions
const SQL_FUNCTIONS = [
  // Aggregate
  'COUNT', 'SUM', 'AVG', 'MIN', 'MAX', 'GROUP_CONCAT', 'STRING_AGG',
  'STDDEV', 'VARIANCE', 'ARRAY_AGG', 'JSON_AGG',
  // String
  'CONCAT', 'CONCAT_WS', 'LENGTH', 'CHAR_LENGTH', 'UPPER', 'LOWER',
  'TRIM', 'LTRIM', 'RTRIM', 'SUBSTRING', 'SUBSTR', 'LEFT', 'RIGHT',
  'REPLACE', 'REGEXP_REPLACE', 'REGEXP_MATCH', 'POSITION', 'LOCATE',
  'LPAD', 'RPAD', 'REPEAT', 'REVERSE', 'FORMAT', 'CHAR', 'ASCII',
  'INSTR', 'FIELD', 'FIND_IN_SET', 'QUOTE', 'SPACE',
  // Numeric
  'ABS', 'CEIL', 'CEILING', 'FLOOR', 'ROUND', 'TRUNCATE', 'MOD',
  'POWER', 'POW', 'SQRT', 'LOG', 'LOG2', 'LOG10', 'EXP',
  'SIGN', 'PI', 'RAND', 'RANDOM',
  // Date/Time
  'NOW', 'CURDATE', 'CURRENT_DATE', 'CURTIME', 'CURRENT_TIME',
  'CURRENT_TIMESTAMP', 'SYSDATE', 'LOCALTIME', 'LOCALTIMESTAMP',
  'DATE', 'TIME', 'DATETIME', 'TIMESTAMP',
  'DATE_FORMAT', 'TO_CHAR', 'TO_DATE',
  'DATE_ADD', 'DATE_SUB', 'DATEADD', 'DATEDIFF', 'TIMEDIFF',
  'YEAR', 'MONTH', 'DAY', 'HOUR', 'MINUTE', 'SECOND',
  'WEEK', 'WEEKDAY', 'DAYOFWEEK', 'DAYOFMONTH', 'DAYOFYEAR',
  'QUARTER', 'EXTRACT', 'DATE_PART',
  'UNIX_TIMESTAMP', 'FROM_UNIXTIME', 'EPOCH',
  // Conditional
  'IF', 'IIF', 'IFNULL', 'NULLIF', 'COALESCE', 'ISNULL',
  'CASE', 'NVL', 'DECODE',
  // Type conversion
  'CAST', 'CONVERT', 'TRY_CAST',
  // JSON
  'JSON_OBJECT', 'JSON_ARRAY', 'JSON_EXTRACT', 'JSON_VALUE',
  'JSON_QUERY', 'JSON_SET', 'JSON_REMOVE', 'JSON_CONTAINS',
  'JSON_LENGTH', 'JSON_TYPE', 'JSON_KEYS', 'JSON_ARRAYAGG',
  // Window
  'ROW_NUMBER', 'RANK', 'DENSE_RANK', 'NTILE', 'LAG', 'LEAD',
  'FIRST_VALUE', 'LAST_VALUE', 'NTH_VALUE',
  'OVER', 'PARTITION BY', 'ROWS BETWEEN', 'RANGE BETWEEN',
  'UNBOUNDED PRECEDING', 'CURRENT ROW', 'UNBOUNDED FOLLOWING',
  // Misc
  'DISTINCT', 'LAST_INSERT_ID', 'ROW_COUNT', 'FOUND_ROWS',
  'DATABASE', 'SCHEMA', 'USER', 'VERSION',
  'UUID', 'UUID_SHORT', 'MD5', 'SHA1', 'SHA2',
  'COMPRESS', 'UNCOMPRESS',
];

// Common data types
const SQL_DATA_TYPES = [
  'INT', 'INTEGER', 'TINYINT', 'SMALLINT', 'MEDIUMINT', 'BIGINT',
  'UNSIGNED', 'SIGNED',
  'FLOAT', 'DOUBLE', 'DECIMAL', 'NUMERIC', 'REAL',
  'CHAR', 'VARCHAR', 'TINYTEXT', 'TEXT', 'MEDIUMTEXT', 'LONGTEXT',
  'BINARY', 'VARBINARY', 'TINYBLOB', 'BLOB', 'MEDIUMBLOB', 'LONGBLOB',
  'DATE', 'TIME', 'DATETIME', 'TIMESTAMP', 'YEAR',
  'BOOLEAN', 'BOOL', 'BIT',
  'ENUM', 'SET',
  'JSON', 'JSONB',
  'UUID', 'SERIAL', 'BIGSERIAL', 'SMALLSERIAL',
  'BYTEA', 'ARRAY',
];


/**
 * Scan the entire document text and build a map of alias → table name.
 * Handles patterns: FROM tbl [AS] alias, JOIN tbl [AS] alias
 */
export function extractAliasMap(fullText: string): Map<string, string> {
  const aliasMap = new Map<string, string>();
  // Match: (FROM|JOIN ...) tableName [AS] alias — stops before ON/WHERE/SET/,/newline
  const re = /(?:FROM|JOIN|STRAIGHT_JOIN|NATURAL\s+JOIN)\s+`?(\w+)`?\s+(?:AS\s+)?`?(\w+)`?(?=\s|$|,|ON\b|WHERE\b|SET\b)/gi;
  let m: RegExpExecArray | null;
  const kwSet = new Set(['ON', 'WHERE', 'SET', 'AND', 'OR', 'NOT', 'LEFT', 'RIGHT', 'INNER', 'OUTER', 'FULL', 'CROSS', 'JOIN', 'GROUP', 'ORDER', 'HAVING', 'LIMIT', 'UNION', 'SELECT']);
  while ((m = re.exec(fullText)) !== null) {
    const tableName = m[1];
    const alias = m[2];
    if (tableName && alias && !kwSet.has(alias.toUpperCase())) {
      aliasMap.set(alias.toLowerCase(), tableName.toLowerCase());
    }
  }
  return aliasMap;
}

/**
 * Find all table/view names referenced in the query (FROM, JOIN, UPDATE, INTO).
 */
export function extractReferencedTableNames(fullText: string): Set<string> {
  const names = new Set<string>();
  const re = /(?:FROM|JOIN|STRAIGHT_JOIN|NATURAL\s+JOIN|UPDATE|INTO)\s+`?(\w+)`?/gi;
  let m: RegExpExecArray | null;
  while ((m = re.exec(fullText)) !== null) {
    if (m[1]) {
      names.add(m[1].toLowerCase());
    }
  }
  return names;
}

/**
 * The main completion function — call this from `provideCompletionItems`.
 * Pass the full model text so alias/reference extraction works across lines.
 */
export function provideSqlCompletions(
  model: monaco.editor.ITextModel,
  position: monaco.Position,
  cachedSchema: CachedObject[],
  dbType: DbType = 'postgreSQL',
): monaco.languages.CompletionList {
  const word = model.getWordUntilPosition(position);
  const range: monaco.IRange = {
    startLineNumber: position.lineNumber,
    endLineNumber: position.lineNumber,
    startColumn: word.startColumn,
    endColumn: word.endColumn,
  };

  const lineContent = model.getLineContent(position.lineNumber);
  const textUntilPosition = lineContent.substring(0, position.column - 1);
  const fullText = model.getValue();

  const suggestions: monaco.languages.CompletionItem[] = [];

  // ── Dot-notation: resolve alias/table → suggest its columns ──
  if (textUntilPosition.endsWith('.')) {
    const dotMatch = textUntilPosition.match(/([a-zA-Z0-9_`]+)\.$/);
    if (dotMatch?.[1]) {
      const prefix = dotMatch[1].replace(/`/g, '').toLowerCase();
      const aliasMap = extractAliasMap(fullText);
      // Could be an alias or a direct table name
      const resolvedName = aliasMap.get(prefix) ?? prefix;
      const tableObj = cachedSchema.find(
        obj => obj.name.toLowerCase() === resolvedName &&
          (obj.objectType === 'table' || obj.objectType === 'view'),
      );
      if (tableObj?.columns) {
        tableObj.columns.forEach(col => {
          suggestions.push({
            label: col.name,
            kind: monaco.languages.CompletionItemKind.Field,
            detail: col.dataType,
            insertText: col.name,
            range,
          });
        });
      }
    }
    return { suggestions };
  }

  // ── Smart JOIN condition suggestion ──
  const textBeforeCursor = textUntilPosition.toUpperCase();
  if (textBeforeCursor.match(/\bJOIN\s+[a-zA-Z0-9_`]+\s+(?:AS\s+[a-zA-Z0-9_`]+\s+)?ON\s*$/) || textBeforeCursor.endsWith(' ON')) {
    const joinMatch = textUntilPosition.match(/(?:JOIN|LEFT\s+JOIN|RIGHT\s+JOIN|INNER\s+JOIN|FULL\s+JOIN|CROSS\s+JOIN)\s+`?([a-zA-Z0-9_]+)`?\s*(?:AS\s+)?`?([a-zA-Z0-9_]+)?`?\s+ON\s*$/i);
    if (joinMatch) {
      const match1 = joinMatch[1];
      if (match1) {
        const joinedTable = match1.toLowerCase();
        const joinedAlias = joinMatch[2] || match1;

        const referencedTables = Array.from(extractReferencedTableNames(fullText))
          .filter(name => name.toLowerCase() !== joinedTable);

        const aliasMap = extractAliasMap(fullText);
        const suggestionsForOn: monaco.languages.CompletionItem[] = [];

        const joinedObj = cachedSchema.find(obj => obj.name.toLowerCase() === joinedTable);
        if (joinedObj && joinedObj.columns) {
          const joinedCols = joinedObj.columns;
          referencedTables.forEach(refTableName => {
            const refObj = cachedSchema.find(obj => obj.name.toLowerCase() === refTableName);
            if (refObj && refObj.columns) {
              const refCols = refObj.columns;
              let refAlias = refTableName;
              for (const [alias, tbl] of aliasMap.entries()) {
                if (tbl === refTableName) {
                  refAlias = alias;
                  break;
                }
              }

              joinedCols.forEach(joinedCol => {
                refCols.forEach(refCol => {
                  const joinedColLower = joinedCol.name.toLowerCase();
                  const refColLower = refCol.name.toLowerCase();

                  let isMatch = false;
                  if (joinedColLower === refColLower && joinedColLower !== 'id') {
                    isMatch = true;
                  } else if (joinedColLower === `${refTableName}_id` && refColLower === 'id') {
                    isMatch = true;
                  } else if (refColLower === `${joinedTable}_id` && joinedColLower === 'id') {
                    isMatch = true;
                  } else if (refAlias !== refTableName && joinedColLower === `${refAlias.toLowerCase()}_id` && refColLower === 'id') {
                    isMatch = true;
                  } else if (joinedAlias !== joinedTable && refColLower === `${joinedAlias.toLowerCase()}_id` && joinedColLower === 'id') {
                    isMatch = true;
                  }

                  if (isMatch) {
                    const condition = `${joinedAlias}.${joinedCol.name} = ${refAlias}.${refCol.name}`;
                    suggestionsForOn.push({
                      label: `ON ${condition}`,
                      kind: monaco.languages.CompletionItemKind.Snippet,
                      detail: `Smart JOIN condition`,
                      insertText: condition,
                      range,
                      sortText: '0_join_' + condition,
                    });
                  }
                });
              });
            }
          });
        }

        if (suggestionsForOn.length > 0) {
          return { suggestions: suggestionsForOn };
        }
      }
    }
  }

  // ── General context ──

  // 1. SQL keywords
  SQL_KEYWORDS.forEach(kw => {
    suggestions.push({
      label: kw,
      kind: monaco.languages.CompletionItemKind.Keyword,
      insertText: kw,
      range,
      sortText: '1_' + kw,
    });
  });

  // 2. SQL functions (with parentheses snippet)
  SQL_FUNCTIONS.forEach(fn => {
    suggestions.push({
      label: fn,
      kind: monaco.languages.CompletionItemKind.Function,
      detail: 'function',
      insertText: fn + '($0)',
      insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
      range,
      sortText: '2_' + fn,
    });
  });

  // 3. SQL data types
  SQL_DATA_TYPES.forEach(dt => {
    suggestions.push({
      label: dt,
      kind: monaco.languages.CompletionItemKind.TypeParameter,
      detail: 'data type',
      insertText: dt,
      range,
      sortText: '3_' + dt,
    });
  });

  // 4. Tables and views from schema
  cachedSchema.forEach(obj => {
    if (obj.objectType === 'table' || obj.objectType === 'view') {
      suggestions.push({
        label: obj.name,
        kind: obj.objectType === 'table'
          ? monaco.languages.CompletionItemKind.Struct
          : monaco.languages.CompletionItemKind.Interface,
        detail: obj.objectType,
        insertText: obj.name,
        range,
        sortText: '4_' + obj.name,
      });
    }
  });

  // 5. Routines/procedures from schema
  cachedSchema.forEach(obj => {
    if (obj.objectType === 'procedure' || obj.objectType === 'function') {
      suggestions.push({
        label: obj.name,
        kind: monaco.languages.CompletionItemKind.Module,
        detail: obj.objectType,
        insertText: obj.name,
        range,
        sortText: '5_' + obj.name,
      });
    }
  });

  // 6. Columns — from tables referenced anywhere in the full query
  const referencedTableNames = extractReferencedTableNames(fullText);
  const aliasMap = extractAliasMap(fullText);

  // also resolve aliases → their real table names
  aliasMap.forEach(tableName => referencedTableNames.add(tableName));

  const addedColumns = new Set<string>(); // avoid duplicates across tables
  referencedTableNames.forEach(refName => {
    const tableObj = cachedSchema.find(
      obj => obj.name.toLowerCase() === refName &&
        (obj.objectType === 'table' || obj.objectType === 'view'),
    );
    if (tableObj?.columns) {
      tableObj.columns.forEach(col => {
        if (!addedColumns.has(col.name)) {
          addedColumns.add(col.name);
          suggestions.push({
            label: col.name,
            kind: monaco.languages.CompletionItemKind.Field,
            detail: `${tableObj.name} · ${col.dataType}`,
            insertText: col.name,
            range,
            sortText: '0_' + col.name, // float columns to the top
          });
        }
      });
    }
  });

  // ── Syntax Helper Snippets ──
  SNIPPETS.forEach(snippet => {
    const dialectInfo = snippet.dialects[dbType];
    if (dialectInfo) {
      suggestions.push({
        label: `snippet: ${snippet.name}`,
        kind: monaco.languages.CompletionItemKind.Snippet,
        detail: `[SQL Help] ${snippet.description}`,
        documentation: {
          value: `Dialect: ${dbType}\nCategory: ${snippet.category}\n\nSyntax:\n\`\`\`sql\n${dialectInfo.code}\n\`\`\`\n\nExample:\n\`\`\`sql\n${dialectInfo.example}\n\`\`\``
        },
        insertText: dialectInfo.code,
        insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
        range,
        sortText: '9_' + snippet.name, // float snippets down to keep default suggestions clean
      });
    }
  });

  return { suggestions };
}

/**
 * Create and register a completion provider for the given cachedSchema ref.
 * Returns a disposable — call `.dispose()` in onBeforeUnmount.
 */
export function registerSqlCompletionProvider(
  getCachedSchema: () => CachedObject[],
  getDbType: () => DbType = () => 'postgreSQL',
  isModelActive?: (model: monaco.editor.ITextModel) => boolean,
): monaco.IDisposable {
  return monaco.languages.registerCompletionItemProvider('sql', {
    triggerCharacters: ['.', ' ', '('],
    provideCompletionItems: (model, position) => {
      if (isModelActive && !isModelActive(model)) {
        return { suggestions: [] };
      }
      return provideSqlCompletions(model, position, getCachedSchema(), getDbType());
    },
  });
}

