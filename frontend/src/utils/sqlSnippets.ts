import type { DbType } from '../types';

export interface Snippet {
  id: string;
  name: string;
  description: string;
  category: 'datetime' | 'timezone' | 'json' | 'strings' | 'conditionals' | 'window';
  dialects: Partial<Record<DbType, { code: string; example: string }>>;
}

export const SNIPPETS: Snippet[] = [
  // --- Date & Time ---
  {
    id: 'dt_cast_date',
    name: 'Timestamp to Date',
    description: 'Trích xuất phần ngày (YYYY-MM-DD) từ giá trị timestamp/datetime.',
    category: 'datetime',
    dialects: {
      postgreSQL: { code: 'CAST(${1:column} AS DATE)', example: 'CAST(created_at AS DATE) -- hoặc created_at::date' },
      mySQL: { code: 'DATE(${1:column})', example: 'DATE(created_at)' },
      sqlite: { code: 'DATE(${1:column})', example: 'DATE(created_at)' },
      sqlServer: { code: 'CAST(${1:column} AS DATE)', example: 'CAST(created_at AS DATE)' },
      oracle: { code: 'TRUNC(${1:column})', example: 'TRUNC(created_at)' },
      mongoDB: { code: '{\n  $dateToString: {\n    format: "%Y-%m-%d",\n    date: "$${1:column}"\n  }\n}', example: '{\n  $dateToString: {\n    format: "%Y-%m-%d",\n    date: "$createdAt"\n  }\n}' }
    }
  },
  {
    id: 'dt_format_str',
    name: 'Format Timestamp to String',
    description: 'Chuyển đổi timestamp thành chuỗi ký tự theo định dạng tùy chỉnh.',
    category: 'datetime',
    dialects: {
      postgreSQL: { code: "TO_CHAR(${1:column}, '${2:YYYY-MM-DD HH24:MI:SS}')", example: "TO_CHAR(created_at, 'YYYY-MM-DD HH24:MI:SS')" },
      mySQL: { code: "DATE_FORMAT(${1:column}, '${2:%Y-%m-%d %H:%i:%s}')", example: "DATE_FORMAT(created_at, '%Y-%m-%d %H:%i:%s')" },
      sqlite: { code: "STRFTIME('${2:%Y-%m-%d %H:%M:%S}', ${1:column})", example: "STRFTIME('%Y-%m-%d %H:%M:%S', created_at)" },
      sqlServer: { code: "CONVERT(VARCHAR(${2:19}), ${1:column}, ${3:120})", example: "CONVERT(VARCHAR(19), created_at, 120)" },
      oracle: { code: "TO_CHAR(${1:column}, '${2:YYYY-MM-DD HH24:MI:SS}')", example: "TO_CHAR(created_at, 'YYYY-MM-DD HH24:MI:SS')" },
      mongoDB: { code: '{\n  $dateToString: {\n    format: "${2:%Y-%m-%d %H:%M:%S}",\n    date: "$${1:column}"\n  }\n}', example: '{\n  $dateToString: {\n    format: "%Y-%m-%d %H:%M:%S",\n    date: "$createdAt"\n  }\n}' }
    }
  },
  {
    id: 'dt_epoch_to_ts',
    name: 'Unix Epoch to Timestamp',
    description: 'Đổi số giây/mili giây Unix Epoch thành kiểu DateTime/Timestamp.',
    category: 'datetime',
    dialects: {
      postgreSQL: { code: 'TO_TIMESTAMP(${1:epoch_seconds_col})', example: 'TO_TIMESTAMP(created_at_sec) -- hoặc TO_TIMESTAMP(created_at_ms / 1000.0)' },
      mySQL: { code: 'FROM_UNIXTIME(${1:epoch_seconds_col})', example: 'FROM_UNIXTIME(created_at_sec) -- hoặc FROM_UNIXTIME(created_at_ms / 1000)' },
      sqlite: { code: "DATETIME(${1:epoch_seconds_col}, 'unixepoch')", example: "DATETIME(created_at, 'unixepoch')" },
      sqlServer: { code: "DATEADD(second, ${1:epoch_seconds_col}, '1970-01-01')", example: "DATEADD(second, created_at, '1970-01-01')" },
      oracle: { code: "TO_TIMESTAMP('1970-01-01 00:00:00','YYYY-MM-DD HH24:MI:SS') + NUMTODSINTERVAL(${1:epoch_seconds_col}, 'SECOND')", example: "TO_TIMESTAMP('1970-01-01 00:00:00','YYYY-MM-DD HH24:MI:SS') + NUMTODSINTERVAL(created_at, 'SECOND')" },
      mongoDB: { code: '{\n  $toDate: "$${1:epoch_seconds_col}"\n}', example: '{\n  $toDate: "$createdAt"\n}' }
    }
  },
  {
    id: 'dt_ts_to_epoch',
    name: 'Timestamp to Unix Epoch',
    description: 'Chuyển đổi kiểu DateTime/Timestamp sang số nguyên Unix Epoch (giây).',
    category: 'datetime',
    dialects: {
      postgreSQL: { code: 'EXTRACT(EPOCH FROM ${1:column})', example: 'EXTRACT(EPOCH FROM created_at)' },
      mySQL: { code: 'UNIX_TIMESTAMP(${1:column})', example: 'UNIX_TIMESTAMP(created_at)' },
      sqlite: { code: "STRFTIME('%s', ${1:column})", example: "STRFTIME('%s', created_at)" },
      sqlServer: { code: "DATEDIFF(second, '1970-01-01', ${1:column})", example: "DATEDIFF(second, '1970-01-01', created_at)" },
      oracle: { code: "(CAST(${1:column} AS DATE) - TO_DATE('1970-01-01','YYYY-MM-DD')) * 86400", example: "(CAST(created_at AS DATE) - TO_DATE('1970-01-01','YYYY-MM-DD')) * 86400" },
      mongoDB: { code: '{\n  $toLong: "$${1:column}"\n}', example: '{\n  $toLong: "$createdAt"\n}' }
    }
  },
  {
    id: 'dt_current',
    name: 'Get Current Date & Time',
    description: 'Lấy thời gian hiện tại của hệ thống database.',
    category: 'datetime',
    dialects: {
      postgreSQL: { code: 'CURRENT_TIMESTAMP', example: 'CURRENT_TIMESTAMP -- hoặc NOW()' },
      mySQL: { code: 'NOW()', example: 'NOW()' },
      sqlite: { code: "DATETIME('now')", example: "DATETIME('now') -- hoặc CURRENT_TIMESTAMP" },
      sqlServer: { code: 'GETDATE()', example: 'GETDATE() -- hoặc CURRENT_TIMESTAMP' },
      oracle: { code: 'CURRENT_TIMESTAMP', example: 'CURRENT_TIMESTAMP -- hoặc SYSDATE' },
      mongoDB: { code: '$$NOW', example: '$$NOW' }
    }
  },
  {
    id: 'dt_add_interval',
    name: 'Add Interval to DateTime',
    description: 'Cộng một khoảng thời gian (ngày, giờ, phút) vào timestamp.',
    category: 'datetime',
    dialects: {
      postgreSQL: { code: "${1:column} + INTERVAL '${2:7 days}'", example: "created_at + INTERVAL '7 days'" },
      mySQL: { code: "DATE_ADD(${1:column}, INTERVAL ${2:7} ${3:DAY})", example: "DATE_ADD(created_at, INTERVAL 7 DAY)" },
      sqlite: { code: "DATETIME(${1:column}, '+${2:7} ${3:days}')", example: "DATETIME(created_at, '+7 days')" },
      sqlServer: { code: "DATEADD(${3:day}, ${2:7}, ${1:column})", example: "DATEADD(day, 7, created_at)" },
      oracle: { code: "${1:column} + INTERVAL '${2:7}' ${3:DAY}", example: "created_at + INTERVAL '7' DAY" },
      mongoDB: { code: '{\n  $dateAdd: {\n    startDate: "$${1:column}",\n    unit: "${3:day}",\n    amount: ${2:7}\n  }\n}', example: '{\n  $dateAdd: {\n    startDate: "$createdAt",\n    unit: "day",\n    amount: 7\n  }\n}' }
    }
  },
  {
    id: 'tz_convert',
    name: 'Convert Timezone',
    description: 'Chuyển đổi múi giờ của giá trị timestamp sang múi giờ mong muốn.',
    category: 'timezone',
    dialects: {
      postgreSQL: { code: "${1:column} AT TIME ZONE '${2:UTC}' AT TIME ZONE '${3:Asia/Ho_Chi_Minh}'", example: "created_at AT TIME ZONE 'UTC' AT TIME ZONE 'Asia/Ho_Chi_Minh'" },
      mySQL: { code: "CONVERT_TZ(${1:column}, '${2:UTC}', '${3:Asia/Ho_Chi_Minh}')", example: "CONVERT_TZ(created_at, 'UTC', 'Asia/Ho_Chi_Minh')" },
      sqlite: { code: "DATETIME(${1:column}, 'localtime') -- Chỉ đổi từ UTC sang múi giờ máy khách local", example: "DATETIME(created_at, 'localtime')" },
      sqlServer: { code: "${1:column} AT TIME ZONE '${2:UTC}' AT TIME ZONE '${3:SE Asia Standard Time}'", example: "created_at AT TIME ZONE 'UTC' AT TIME ZONE 'SE Asia Standard Time'" },
      oracle: { code: "FROM_TZ(CAST(${1:column} AS TIMESTAMP), '${2:UTC}') AT TIME ZONE '${3:Asia/Ho_Chi_Minh}'", example: "FROM_TZ(CAST(created_at AS TIMESTAMP), 'UTC') AT TIME ZONE 'Asia/Ho_Chi_Minh'" },
      mongoDB: { code: '{\n  $dateToString: {\n    format: "%Y-%m-%d %H:%M:%S",\n    date: "$${1:column}",\n    timezone: "${3:Asia/Ho_Chi_Minh}"\n  }\n}', example: '{\n  $dateToString: {\n    format: "%Y-%m-%d %H:%M:%S",\n    date: "$createdAt",\n    timezone: "Asia/Ho_Chi_Minh"\n  }\n}' }
    }
  },
  {
    id: 'json_extract',
    name: 'Extract JSON Value',
    description: 'Trích xuất giá trị từ khóa hoặc đường dẫn của cột JSON.',
    category: 'json',
    dialects: {
      postgreSQL: { code: "${1:column}->>'${2:key}'", example: "attributes->>'user_id'" },
      mySQL: { code: "JSON_UNQUOTE(JSON_EXTRACT(${1:column}, '$.${2:key}'))", example: "attributes->>'$.user_id'" },
      sqlite: { code: "JSON_EXTRACT(${1:column}, '$.${2:key}')", example: "JSON_EXTRACT(attributes, '$.user_id')" },
      sqlServer: { code: "JSON_VALUE(${1:column}, '$.${2:key}')", example: "JSON_VALUE(attributes, '$.user_id')" },
      oracle: { code: "JSON_VALUE(${1:column}, '$.${2:key}')", example: "JSON_VALUE(attributes, '$.user_id')" },
      mongoDB: { code: '"$${1:column}.${2:key}"', example: '"$attributes.user_id"' }
    }
  },
  {
    id: 'cond_coalesce',
    name: 'Coalesce (Handle Null)',
    description: 'Trả về giá trị mặc định đầu tiên nếu cột bị NULL.',
    category: 'conditionals',
    dialects: {
      postgreSQL: { code: 'COALESCE(${1:column}, ${2:default_value})', example: 'COALESCE(status, \'unknown\')' },
      mySQL: { code: 'IFNULL(${1:column}, ${2:default_value})', example: 'IFNULL(status, \'unknown\') -- hoặc COALESCE' },
      sqlite: { code: 'COALESCE(${1:column}, ${2:default_value})', example: 'COALESCE(status, \'unknown\') -- hoặc IFNULL' },
      sqlServer: { code: 'ISNULL(${1:column}, ${2:default_value})', example: 'ISNULL(status, \'unknown\') -- hoặc COALESCE' },
      oracle: { code: 'NVL(${1:column}, ${2:default_value})', example: 'NVL(status, \'unknown\')' },
      mongoDB: { code: '{\n  $ifNull: ["$${1:column}", ${2:default_value}]\n}', example: '{\n  $ifNull: ["$status", "unknown"]\n}' }
    }
  },
  {
    id: 'cond_case',
    name: 'CASE WHEN Statement',
    description: 'Câu lệnh rẽ nhánh điều kiện logic.',
    category: 'conditionals',
    dialects: {
      postgreSQL: { code: 'CASE WHEN ${1:condition} THEN ${2:value1} ELSE ${3:value2} END', example: 'CASE WHEN age >= 18 THEN \'Adult\' ELSE \'Minor\' END' },
      mySQL: { code: 'CASE WHEN ${1:condition} THEN ${2:value1} ELSE ${3:value2} END', example: 'CASE WHEN age >= 18 THEN \'Adult\' ELSE \'Minor\' END' },
      sqlite: { code: 'CASE WHEN ${1:condition} THEN ${2:value1} ELSE ${3:value2} END', example: 'CASE WHEN age >= 18 THEN \'Adult\' ELSE \'Minor\' END' },
      sqlServer: { code: 'CASE WHEN ${1:condition} THEN ${2:value1} ELSE ${3:value2} END', example: 'CASE WHEN age >= 18 THEN \'Adult\' ELSE \'Minor\' END' },
      oracle: { code: 'CASE WHEN ${1:condition} THEN ${2:value1} ELSE ${3:value2} END', example: 'CASE WHEN age >= 18 THEN \'Adult\' ELSE \'Minor\' END' },
      mongoDB: { code: '{\n  $cond: {\n    if: { ${1:condition} },\n    then: ${2:value1},\n    else: ${3:value2}\n  }\n}', example: '{\n  $cond: {\n    if: { $gte: ["$age", 18] },\n    then: "Adult",\n    else: "Minor"\n  }\n}' }
    }
  },
  {
    id: 'win_row_number',
    name: 'Row Number (Window)',
    description: 'Đánh số thứ tự tăng dần cho các dòng trong nhóm phân vùng.',
    category: 'window',
    dialects: {
      postgreSQL: { code: 'ROW_NUMBER() OVER (PARTITION BY ${1:partition_col} ORDER BY ${2:sort_col})', example: 'ROW_NUMBER() OVER (PARTITION BY group_id ORDER BY created_at DESC)' },
      mySQL: { code: 'ROW_NUMBER() OVER (PARTITION BY ${1:partition_col} ORDER BY ${2:sort_col})', example: 'ROW_NUMBER() OVER (PARTITION BY group_id ORDER BY created_at DESC)' },
      sqlite: { code: 'ROW_NUMBER() OVER (PARTITION BY ${1:partition_col} ORDER BY ${2:sort_col})', example: 'ROW_NUMBER() OVER (PARTITION BY group_id ORDER BY created_at DESC)' },
      sqlServer: { code: 'ROW_NUMBER() OVER (PARTITION BY ${1:partition_col} ORDER BY ${2:sort_col})', example: 'ROW_NUMBER() OVER (PARTITION BY group_id ORDER BY created_at DESC)' },
      oracle: { code: 'ROW_NUMBER() OVER (PARTITION BY ${1:partition_col} ORDER BY ${2:sort_col})', example: 'ROW_NUMBER() OVER (PARTITION BY group_id ORDER BY created_at DESC)' },
      mongoDB: { code: '{\n  $setWindowFields: {\n    partitionBy: "$${1:partition_col}",\n    sortBy: { ${2:sort_col}: -1 },\n    output: {\n      rowNumber: { $documentNumber: {} }\n    }\n  }\n}', example: '{\n  $setWindowFields: {\n    partitionBy: "$group_id",\n    sortBy: { created_at: -1 },\n    output: {\n      rowNumber: { $documentNumber: {} }\n    }\n  }\n}' }
    }
  }
];
