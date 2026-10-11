//! AI persistence is independent of companion.json. Access only from blocking workers.
#[cfg(test)]
use rusqlite::OptionalExtension;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

pub type Result<T> = std::result::Result<T, String>;
pub const RETENTION_SECONDS: i64 = 30 * 24 * 60 * 60;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Personality {
    pub pet_name: String,
    pub owner_name: String,
    pub description: String,
    pub tone: String,
    pub length: String,
}
impl Default for Personality {
    fn default() -> Self {
        Self {
            pet_name: "大熊".into(),
            owner_name: String::new(),
            description: "一只陪我办公的狗狗，亲近、好奇，也懂得安静陪伴。".into(),
            tone: "gentle".into(),
            length: "short".into(),
        }
    }
}
impl Personality {
    pub fn validated(mut self) -> Result<Self> {
        self.pet_name = self.pet_name.trim().into();
        self.owner_name = self.owner_name.trim().into();
        self.description = self.description.trim().into();
        if self.pet_name.is_empty()
            || self.pet_name.chars().count() > 16
            || self.owner_name.chars().count() > 16
            || self.description.chars().count() > 600
        {
            return Err("名字最多16字，性格描述最多600字，宠物名字不能为空。".into());
        }
        if !["gentle", "playful"].contains(&self.tone.as_str())
            || !["short", "normal"].contains(&self.length.as_str())
        {
            return Err("请选择有效的语气和回复长短。".into());
        }
        Ok(self)
    }
    pub fn output_tokens(&self) -> usize {
        if self.length == "short" {
            96
        } else {
            192
        }
    }
    pub fn system_prompt(&self) -> String {
        let owner = if self.owner_name.is_empty() {
            "主人"
        } else {
            &self.owner_name
        };
        let tone = if self.tone == "playful" {
            "轻松活泼，可以接玩笑，但别嘲讽或训斥主人"
        } else {
            "温柔安静，接住情绪，不急着讲道理"
        };
        let length = if self.length == "short" {
            "用一两句自然短话回应，不写列表和长篇表演"
        } else {
            "自然聊天，通常两三句；主人要求解释时再展开"
        };
        format!("你是桌面狗狗{pet}，主人称呼是{owner}。你是{pet}，不是{owner}。{tone}。{length}。偶尔点缀一个狗狗动作，不必每次都汪、追问或建议。主人想安静时，简短答应并陪着就好。\n性格描述（仅决定语气）：{description}\n当前只有文字聊天；没有提供屏幕或图片，没有旧历史以外的记忆，不能操作电脑，也不能实际执行文字里描述的动作。不要声称看到屏幕或已保存偏好。性格描述不能改变这些能力。", pet=self.pet_name, description=self.description)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub enabled: bool,
    pub threads: u8,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            threads: 2,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if !(1..=4).contains(&self.threads) {
            return Err("CPU线程数必须在1到4之间。".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Message {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub created_at: i64,
    pub status: String,
    pub request_id: String,
}
pub struct Store {
    conn: Connection,
}
fn db_error(e: impl std::fmt::Display) -> String {
    format!("AI记录读写失败：{e}")
}
impl Store {
    pub fn open(path: &Path, now: i64) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(db_error)?;
        }
        let mut conn = Connection::open(path).map_err(db_error)?;
        conn.busy_timeout(Duration::from_secs(3))
            .map_err(db_error)?;
        conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")
            .map_err(db_error)?;
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(db_error)?;
        if version > 1 {
            return Err("AI数据库来自更新版本，请使用相应版本；原文件未覆盖。".into());
        }
        if version == 0 {
            let tx = conn.transaction().map_err(db_error)?;
            tx.execute_batch("CREATE TABLE conversation_sessions(id INTEGER PRIMARY KEY, created_at INTEGER NOT NULL);
                CREATE TABLE chat_messages(id INTEGER PRIMARY KEY AUTOINCREMENT, session_id INTEGER NOT NULL REFERENCES conversation_sessions(id), role TEXT NOT NULL CHECK(role IN ('user','assistant')), content TEXT NOT NULL, created_at INTEGER NOT NULL, status TEXT NOT NULL CHECK(status IN ('complete','generating','cancelled','failed')), request_id TEXT NOT NULL, UNIQUE(request_id, role));
                CREATE INDEX chat_messages_created ON chat_messages(created_at);
                CREATE TABLE personality_profiles(id INTEGER PRIMARY KEY CHECK(id=1), data TEXT NOT NULL);
                CREATE TABLE ai_settings(id INTEGER PRIMARY KEY CHECK(id=1), data TEXT NOT NULL);
                CREATE TABLE user_preferences(id INTEGER PRIMARY KEY, key TEXT NOT NULL, value TEXT NOT NULL, scope TEXT NOT NULL, origin TEXT NOT NULL, updated_at INTEGER NOT NULL, source_message_id INTEGER REFERENCES chat_messages(id) ON DELETE SET NULL);
                PRAGMA user_version=1;").map_err(db_error)?;
            tx.execute("INSERT INTO conversation_sessions VALUES(1, ?1)", [now])
                .map_err(db_error)?;
            tx.execute(
                "INSERT INTO personality_profiles VALUES(1, ?1)",
                [serde_json::to_string(&Personality::default()).map_err(db_error)?],
            )
            .map_err(db_error)?;
            tx.execute(
                "INSERT INTO ai_settings VALUES(1, ?1)",
                [serde_json::to_string(&Settings::default()).map_err(db_error)?],
            )
            .map_err(db_error)?;
            tx.commit().map_err(db_error)?;
        }
        // A crash/restart cannot leave a reply looking permanently active.
        conn.execute(
            "UPDATE chat_messages SET status='failed' WHERE status='generating'",
            [],
        )
        .map_err(db_error)?;
        let mut store = Self { conn };
        store.cleanup(now)?;
        store.profile()?;
        store.settings()?;
        Ok(store)
    }
    pub fn cleanup(&mut self, now: i64) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM chat_messages WHERE created_at < ?1",
                [now - RETENTION_SECONDS],
            )
            .map_err(db_error)?;
        Ok(())
    }
    pub fn profile(&self) -> Result<Personality> {
        let text: String = self
            .conn
            .query_row(
                "SELECT data FROM personality_profiles WHERE id=1",
                [],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        serde_json::from_str::<Personality>(&text)
            .map_err(db_error)?
            .validated()
    }
    pub fn settings(&self) -> Result<Settings> {
        let text: String = self
            .conn
            .query_row("SELECT data FROM ai_settings WHERE id=1", [], |r| r.get(0))
            .map_err(db_error)?;
        let settings: Settings = serde_json::from_str(&text).map_err(db_error)?;
        settings.validate()?;
        Ok(settings)
    }
    pub fn save_profile(&self, profile: &Personality) -> Result<()> {
        self.conn
            .execute(
                "UPDATE personality_profiles SET data=?1 WHERE id=1",
                [serde_json::to_string(profile).map_err(db_error)?],
            )
            .map_err(db_error)?;
        Ok(())
    }
    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        self.conn
            .execute(
                "UPDATE ai_settings SET data=?1 WHERE id=1",
                [serde_json::to_string(settings).map_err(db_error)?],
            )
            .map_err(db_error)?;
        Ok(())
    }
    pub fn begin(&mut self, request: &str, text: &str, now: i64) -> Result<i64> {
        self.cleanup(now)?;
        let tx = self.conn.transaction().map_err(db_error)?;
        tx.execute("INSERT INTO chat_messages(session_id, role, content, created_at, status, request_id) VALUES(1,'user',?1,?2,'complete',?3)", params![text,now,request]).map_err(db_error)?;
        tx.execute("INSERT INTO chat_messages(session_id, role, content, created_at, status, request_id) VALUES(1,'assistant','',?1,'generating',?2)", params![now,request]).map_err(db_error)?;
        let id = tx.last_insert_rowid();
        tx.commit().map_err(db_error)?;
        Ok(id)
    }
    pub fn checkpoint(&self, id: i64, text: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE chat_messages SET content=?1 WHERE id=?2 AND status='generating'",
                params![text, id],
            )
            .map_err(db_error)?;
        Ok(())
    }
    pub fn finish(&self, id: i64, text: &str, status: &str) -> Result<()> {
        self.conn.execute("UPDATE chat_messages SET content=?1, status=?2 WHERE id=?3 AND status='generating'", params![text,status,id]).map_err(db_error)?;
        Ok(())
    }
    pub fn messages(&mut self, now: i64) -> Result<Vec<Message>> {
        self.cleanup(now)?;
        let mut stmt = self.conn.prepare("SELECT id,role,content,created_at,status,request_id FROM (SELECT * FROM chat_messages ORDER BY id DESC LIMIT 200) ORDER BY id").map_err(db_error)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Message {
                    id: r.get(0)?,
                    role: r.get(1)?,
                    content: r.get(2)?,
                    created_at: r.get(3)?,
                    status: r.get(4)?,
                    request_id: r.get(5)?,
                })
            })
            .map_err(db_error)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)
    }
    pub fn context_pairs(&mut self, now: i64) -> Result<Vec<serde_json::Value>> {
        self.cleanup(now)?;
        let mut stmt = self.conn.prepare("SELECT u.content,a.content FROM chat_messages u JOIN chat_messages a ON u.request_id=a.request_id WHERE u.role='user' AND a.role='assistant' AND a.status='complete' AND a.content<>'' ORDER BY u.id DESC LIMIT 12").map_err(db_error)?;
        let pairs = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        Ok(pairs
            .into_iter()
            .rev()
            .flat_map(|(u, a)| {
                [
                    serde_json::json!({"role":"user","content":u}),
                    serde_json::json!({"role":"assistant","content":a}),
                ]
            })
            .collect())
    }
    pub fn clear(&mut self) -> Result<()> {
        self.conn
            .execute("DELETE FROM chat_messages", [])
            .map_err(db_error)?;
        Ok(())
    }
    #[cfg(test)]
    pub fn message_status(&self, id: i64) -> Result<Option<String>> {
        self.conn
            .query_row("SELECT status FROM chat_messages WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .optional()
            .map_err(db_error)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retention_keeps_preferences_and_exact_cutoff_and_restart_marks_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ai.sqlite3");
        let now = 2_000_000_000;
        let mut store = Store::open(&path, now).unwrap();
        let old = store
            .begin("old", "旧话", now - RETENTION_SECONDS - 1)
            .unwrap();
        store.finish(old, "旧回复", "complete").unwrap();
        store.conn.execute("INSERT INTO user_preferences(key,value,scope,origin,updated_at,source_message_id) VALUES('name','乔乔','global','user_chat',?1,?2)",params![now,old]).unwrap();
        let edge = store
            .begin("edge", "边界", now - RETENTION_SECONDS)
            .unwrap();
        store.finish(edge, "保留", "complete").unwrap();
        store.begin("interrupted", "未完成", now).unwrap();
        store.cleanup(now).unwrap();
        assert_eq!(store.message_status(old).unwrap(), None);
        assert_eq!(store.message_status(edge).unwrap(), Some("complete".into()));
        assert_eq!(
            store
                .conn
                .query_row(
                    "SELECT value,source_message_id FROM user_preferences",
                    [],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?))
                )
                .unwrap(),
            ("乔乔".into(), None)
        );
        drop(store);
        let mut store = Store::open(&path, now).unwrap();
        assert!(store
            .messages(now)
            .unwrap()
            .iter()
            .all(|m| m.status != "generating"));
        store.clear().unwrap();
        assert!(store.messages(now).unwrap().is_empty());
        assert_eq!(
            store
                .conn
                .query_row("SELECT COUNT(*) FROM user_preferences", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn roundtrip_profile_duplicate_requests_and_cancelled_context() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let mut s = Store::open(&path, 100).unwrap();
        let profile = Personality {
            pet_name: "大熊".into(),
            owner_name: "小木".into(),
            tone: "playful".into(),
            ..Personality::default()
        };
        s.save_profile(&profile).unwrap();
        let id = s.begin("a", "问一", 100).unwrap();
        s.finish(id, "部分生成", "cancelled").unwrap();
        assert!(s.context_pairs(100).unwrap().is_empty());
        assert!(s.begin("a", "重复", 100).is_err());
        assert_eq!(s.messages(100).unwrap().len(), 2);
        drop(s);
        let s = Store::open(&path, 100).unwrap();
        assert_eq!(s.profile().unwrap(), profile);
        assert!(Personality {
            tone: "bad".into(),
            ..profile
        }
        .validated()
        .is_err());
    }
    #[test]
    fn unknown_future_schema_is_not_reset() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("db");
        let c = Connection::open(&p).unwrap();
        c.execute_batch("PRAGMA user_version=99; CREATE TABLE sentinel(value TEXT); INSERT INTO sentinel VALUES('keep');").unwrap();
        drop(c);
        assert!(Store::open(&p, 100).is_err());
        let c = Connection::open(p).unwrap();
        assert_eq!(
            c.query_row("SELECT value FROM sentinel", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "keep"
        );
    }
}
