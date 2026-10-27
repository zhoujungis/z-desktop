-- Z Desktop v0.1 数据库初始化
-- 对应设计文档 #28 Database Schema，并补充 Conversation / Project / Settings / Approval Rules

CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    completed_at INTEGER
);

CREATE TABLE IF NOT EXISTS task_steps (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    step_index INTEGER NOT NULL,
    tool TEXT,
    input TEXT,
    output TEXT,
    status TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_task_steps_task ON task_steps(task_id, step_index);

CREATE TABLE IF NOT EXISTS events (
    id TEXT PRIMARY KEY,
    task_id TEXT,
    type TEXT NOT NULL,
    data TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_events_task ON events(task_id, created_at);

CREATE TABLE IF NOT EXISTS memories (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    content TEXT NOT NULL,
    source TEXT,
    created_at INTEGER NOT NULL
);

-- 会话记录（设计文档 #27：Memory System 保存 Conversation）
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    task_id TEXT,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_messages_task ON messages(task_id, created_at);

-- 项目系统（设计文档 #29 Project System）
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    description TEXT,
    technology TEXT,
    git_url TEXT,
    is_active INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

-- 设置键值存储
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- 审批规则（Approval UI 的 "Always" 会写入这里）
CREATE TABLE IF NOT EXISTS approval_rules (
    id TEXT PRIMARY KEY,
    tool TEXT NOT NULL,
    pattern TEXT NOT NULL,
    decision TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

-- FTS5 全文检索（设计文档 #27：SQLite + FTS5）
CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
    content,
    content='memories',
    content_rowid='rowid'
);

CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts(rowid, content) VALUES (new.rowid, new.content);
END;

CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, content)
    VALUES ('delete', old.rowid, old.content);
END;

CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, content)
    VALUES ('delete', old.rowid, old.content);
    INSERT INTO memories_fts(rowid, content) VALUES (new.rowid, new.content);
END;
