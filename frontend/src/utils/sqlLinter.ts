import * as monaco from 'monaco-editor';
import { extractAliasMap, extractReferencedTableNames } from './sqlCompletion';

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

// SQL keywords, functions, types to ignore when checking column names
const IGNORE_WORDS = new Set([
  'SELECT', 'FROM', 'WHERE', 'GROUP', 'ORDER', 'BY', 'LIMIT', 'OFFSET',
  'JOIN', 'LEFT', 'RIGHT', 'INNER', 'OUTER', 'FULL', 'CROSS', 'ON', 'AS', 'AND', 'OR', 'NOT', 'IN', 'IS', 'NULL', 'TRUE', 'FALSE',
  'INSERT', 'INTO', 'VALUES', 'UPDATE', 'SET', 'DELETE', 'TRUNCATE', 'DROP', 'CREATE', 'ALTER', 'TABLE', 'DATABASE',
  'SCHEMA', 'VIEW', 'FUNCTION', 'PROCEDURE', 'TRIGGER', 'INDEX', 'PRIMARY', 'KEY', 'FOREIGN', 'REFERENCES', 'UNIQUE',
  'CASE', 'WHEN', 'THEN', 'ELSE', 'END', 'UNION', 'ALL', 'INTERSECT', 'EXCEPT', 'EXISTS', 'LIKE', 'ILIKE', 'BETWEEN',
  'HAVING', 'DISTINCT', 'WITH', 'RECURSIVE', 'CTE', 'USING', 'NATURAL', 'ASC', 'DESC',
  // Functions
  'COUNT', 'SUM', 'AVG', 'MIN', 'MAX', 'CONCAT', 'COALESCE', 'NOW', 'DATE', 'YEAR', 'MONTH', 'DAY', 'CAST', 'CONVERT',
  // Types
  'INT', 'INTEGER', 'VARCHAR', 'TEXT', 'DATE', 'DATETIME', 'TIMESTAMP', 'BOOLEAN', 'NUMBER'
]);

interface Token {
  text: string;
  type: 'word' | 'dot' | 'string' | 'other';
  startLine: number;
  startCol: number;
  endLine: number;
  endCol: number;
}

function tokenize(sql: string): Token[] {
  const tokens: Token[] = [];
  const lines = sql.split('\n');
  
  let inBlockComment = false;
  
  lines.forEach((line, lineIdx) => {
    const lineNum = lineIdx + 1;
    let i = 0;
    
    while (i < line.length) {
      // 1. Block comment end check
      if (inBlockComment) {
        if (line.slice(i, i + 2) === '*/') {
          inBlockComment = false;
          i += 2;
        } else {
          i++;
        }
        continue;
      }
      
      // 2. Block comment start check
      if (line.slice(i, i + 2) === '/*') {
        inBlockComment = true;
        i += 2;
        continue;
      }
      
      // 3. Line comment check
      if (line.slice(i, i + 2) === '--') {
        break; // skip rest of line
      }
      
      const char = line[i];
      if (char === undefined) {
        break;
      }
      
      // 4. White space skip
      if (/\s/.test(char)) {
        i++;
        continue;
      }
      
      // 5. String literal check
      if (char === "'" || char === '"' || char === '`') {
        const quoteChar = char;
        const startCol = i + 1;
        i++; // skip quote
        let strVal = '';
        while (i < line.length) {
          const innerChar = line[i];
          if (innerChar === undefined || innerChar === quoteChar) {
            break;
          }
          if (innerChar === '\\' && i + 1 < line.length) {
            const nextChar = line[i + 1];
            if (nextChar !== undefined) {
              strVal += nextChar;
            }
            i += 2;
          } else {
            strVal += innerChar;
            i++;
          }
        }
        i++; // skip closing quote
        tokens.push({
          text: strVal,
          type: 'string',
          startLine: lineNum,
          startCol,
          endLine: lineNum,
          endCol: i
        });
        continue;
      }
      
      // 6. Dot check
      if (char === '.') {
        tokens.push({
          text: '.',
          type: 'dot',
          startLine: lineNum,
          startCol: i + 1,
          endLine: lineNum,
          endCol: i + 2
        });
        i++;
        continue;
      }
      
      // 7. Word/Identifier check
      if (/[a-zA-Z0-9_]/.test(char)) {
        const startCol = i + 1;
        let word = '';
        while (i < line.length) {
          const innerChar = line[i];
          if (innerChar === undefined || !/[a-zA-Z0-9_]/.test(innerChar)) {
            break;
          }
          word += innerChar;
          i++;
        }
        tokens.push({
          text: word,
          type: 'word',
          startLine: lineNum,
          startCol,
          endLine: lineNum,
          endCol: i
        });
        continue;
      }
      
      // 8. Other operators/characters
      tokens.push({
        text: char,
        type: 'other',
        startLine: lineNum,
        startCol: i + 1,
        endLine: lineNum,
        endCol: i + 1
      });
      i++;
    }
  });
  
  return tokens;
}

export function parseAndLintSql(
  model: monaco.editor.ITextModel,
  cachedSchema: CachedObject[]
): monaco.editor.IMarkerData[] {
  const markers: monaco.editor.IMarkerData[] = [];
  if (!cachedSchema || cachedSchema.length === 0) {
    return markers;
  }
  
  const sqlText = model.getValue();
  const tokens = tokenize(sqlText);
  const referencedTables = extractReferencedTableNames(sqlText);
  const aliasMap = extractAliasMap(sqlText);
  
  // Create lowercase mapping of cached schema tables for lookup
  const schemaTables = new Map<string, CachedObject>();
  cachedSchema.forEach(obj => {
    if (obj.objectType === 'table' || obj.objectType === 'view') {
      schemaTables.set(obj.name.toLowerCase(), obj);
    }
  });
  
  // 1. Lint Table Names
  tokens.forEach((token, idx) => {
    if (token.type === 'word') {
      const prevToken = idx > 0 ? tokens[idx - 1] : null;
      if (prevToken && prevToken.type === 'word') {
        const prevTextUpper = prevToken.text.toUpperCase();
        if (['FROM', 'JOIN', 'UPDATE', 'INTO'].includes(prevTextUpper)) {
          const tableName = token.text.toLowerCase();
          if (!schemaTables.has(tableName)) {
            markers.push({
              severity: monaco.MarkerSeverity.Warning,
              message: `Bảng hoặc View '${token.text}' không tồn tại trong database schema hiện tại.`,
              startLineNumber: token.startLine,
              startColumn: token.startCol,
              endLineNumber: token.endLine,
              endColumn: token.endCol,
              source: 'SQL Schema Linter'
            });
          }
        }
      }
    }
  });
  
  // 2. Lint Column Names
  tokens.forEach((token, idx) => {
    if (token.type === 'word') {
      const nextToken = idx + 1 < tokens.length ? tokens[idx + 1] : null;
      const nextNextToken = idx + 2 < tokens.length ? tokens[idx + 2] : null;
      
      // Pattern: prefix . column
      if (nextToken && nextToken.type === 'dot' && nextNextToken && nextNextToken.type === 'word') {
        const prefix = token.text.toLowerCase();
        const columnName = nextNextToken.text.toLowerCase();
        
        const resolvedTableName = aliasMap.get(prefix) ?? prefix;
        const tableObj = schemaTables.get(resolvedTableName);
        
        if (tableObj) {
          if (tableObj.columns && tableObj.columns.length > 0) {
            const hasColumn = tableObj.columns.some(col => col.name.toLowerCase() === columnName);
            if (!hasColumn) {
              markers.push({
                severity: monaco.MarkerSeverity.Warning,
                message: `Cột '${nextNextToken.text}' không tồn tại trong bảng '${tableObj.name}'.`,
                startLineNumber: nextNextToken.startLine,
                startColumn: nextNextToken.startCol,
                endLineNumber: nextNextToken.endLine,
                endColumn: nextNextToken.endCol,
                source: 'SQL Schema Linter'
              });
            }
          }
        }
      } 
      // Single word column check
      else {
        const prevToken = idx > 0 ? tokens[idx - 1] : null;
        if (prevToken && prevToken.type === 'dot') {
          return;
        }
        
        const tokenTextUpper = token.text.toUpperCase();
        if (!IGNORE_WORDS.has(tokenTextUpper) && isNaN(Number(token.text))) {
          if (aliasMap.has(token.text.toLowerCase()) || referencedTables.has(token.text.toLowerCase())) {
            return;
          }
          
          if (referencedTables.size > 0) {
            let foundValidTableForColumn = false;
            let tablesCheckedCount = 0;
            
            referencedTables.forEach(refTableName => {
              const tableObj = schemaTables.get(refTableName);
              if (tableObj && tableObj.columns && tableObj.columns.length > 0) {
                tablesCheckedCount++;
                const hasColumn = tableObj.columns.some(col => col.name.toLowerCase() === token.text.toLowerCase());
                if (hasColumn) {
                  foundValidTableForColumn = true;
                }
              }
            });
            
            if (tablesCheckedCount > 0 && !foundValidTableForColumn) {
              markers.push({
                severity: monaco.MarkerSeverity.Warning,
                message: `Cột '${token.text}' không tìm thấy trong bất kỳ bảng nào đang được tham chiếu.`,
                startLineNumber: token.startLine,
                startColumn: token.startCol,
                endLineNumber: token.endLine,
                endColumn: token.endCol,
                source: 'SQL Schema Linter'
              });
            }
          }
        }
      }
    }
  });
  
  return markers;
}
