# 数据库设计

> 对应《Z_Desktop_v0.1 开发设计文档》#27-#29 节。Schema 定义：`database/migrations/0001_init.sql`，由 `src-tauri/src/database/migrations.rs` 在启动时执行。

## 1. 概览

- 引擎：SQLite（rusqlite `bundled`，编译期内置，**自带 FTS5**）。
- 模式：WAL（读写并发）；文件：`{app_data_dir}/zdesktop.db`。
- 主键统一 TEXT（UUID v4）；时间统一 INTEGER（Unix 毫秒）。
- 迁移机制：`migrations.rs` 顺序执行内嵌 SQL，幂等（`IF NOT EXISTS`），版本记录在 `schema_migrations` 表；
  SQL 执行与版本写入在同一事务内，中途失败不会留下半成品 schema。

## 2. 表结构（8 张业务表 + FTS5 虚拟表）

### tasks — 任务
| 列 | 类型 | 说明 |
|---|---|---|
| id | TEXT PK | UUID |
| title | TEXT | 任务标题（用户输入摘要） |
| status | TEXT | pending/running/waiting_approval/paused/completed/failed/cancelled |
| created_at / updated_at | INTEGER | 毫秒时间戳 |
| completed_at | INTEGER? | 终态时间 |

### task_steps — 任务步骤（Task Timeline，设计文档 #32）
| 列 | 类型 | 说明 |
|---|---|---|
| id | TEXT PK | |
| task_id | TEXT | 所属任务（索引：task_id + step_index） |
| step_index | INTEGER | 步骤序号 |
| tool / input / output / status | TEXT? | 工具名、参数 JSON、输出、状态 |
| created_at | INTEGER | |

### events — 事件流水（Activity 数据源）
| 列 | 类型 | 说明 |
|---|---|---|
| id | TEXT PK | |
| task_id | TEXT? | 可空（全局事件） |
| type | TEXT | tool_call / approval / status_change / error ... |
| data | TEXT? | JSON |
| created_at | INTEGER | 索引：task_id + created_at |

### memories — 长期记忆
| 列 | 类型 | 说明 |
|---|---|---|
| id | TEXT PK | |
| type | TEXT | fact / preference / note ... |
| content | TEXT | 正文 |
| source | TEXT? | 来源 |
| created_at | INTEGER | |

### messages — 会话消息（设计文档 #27 Memory System）
| 列 | 类型 | 说明 |
|---|---|---|
| id | TEXT PK | |
| task_id | TEXT? | |
| role | TEXT | user / assistant / system |
| content | TEXT | |
| created_at | INTEGER | 索引：task_id + created_at |

### projects — 项目系统（设计文档 #29）
| 列 | 类型 | 说明 |
|---|---|---|
| id | TEXT PK | |
| name / path | TEXT | 项目名与根路径 |
| description / technology / git_url | TEXT? | 元信息 |
| is_active | INTEGER | 当前活跃项目（单选） |
| created_at | INTEGER | |

### settings — 设置键值
key TEXT PK / value TEXT（JSON 字符串，对应 `AppSettings`）。

### approval_rules — 审批规则
id / tool / pattern（完整 input JSON 精确匹配）/ decision（'allow'）/ created_at。

### memories_fts — FTS5 虚拟表（设计文档 #27 SQLite + FTS5）
```sql
CREATE VIRTUAL TABLE memories_fts USING fts5(content, content='memories', content_rowid='rowid');
```
三个触发器（ai/ad/au）保持与 memories 表同步，插入/删除/更新自动同步索引。

## 3. 访问层

`database/models.rs` 提供 CRUD 函数（create_task / update_task_status / append_step / append_event / save_message / add_memory / search_memories / project 系列 / settings 读写），全部由 `commands.rs` 调用，前端不直接接触 SQL。

`search_memories` 会先把用户输入里的 FTS5 控制字符替换为空格，再按词加引号拼接
（`alpha:beta` → `"alpha" OR "beta"`），避免 `*`、`"`、`OR` 等输入让 MATCH 报错。

设置写入前会做归一化：`max_steps` 限制在 1–200，空 provider 回退 `openai`，保证持久化值与内存值一致。

## 4. 演进方向（v0.2+）

- 事件表分区或按任务归档，避免无限增长。
- memories 增加 embedding 列，做向量+全文混合检索。
- 增量迁移版本管理（当前为 `schema_migrations`，后续补充回滚与校验）。
