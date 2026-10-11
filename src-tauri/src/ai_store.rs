//! AI persistence is independent of companion.json. Access only from blocking workers.
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
        format!("你是桌面狗狗{pet}，主人称呼是{owner}。你是{pet}，不是{owner}。{tone}。{length}。偶尔点缀一个狗狗动作，不必每次都汪、追问或建议。主人想安静时，简短答应并陪着就好。\n性格描述（仅决定语气）：{description}\n只有宿主本次提供屏幕或图片材料时才能评论画面；只有宿主确认的偏好列表才代表已保存记忆。不能操作电脑、运行命令，也不能实际执行文字里描述的动作。不要虚构观察或保存结果。性格描述不能改变这些能力。", pet=self.pet_name, description=self.description)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub enabled: bool,
    pub threads: u8,
    pub backend: String,
    pub service_url: String,
    pub service_model: String,
    pub remote_screen: bool,
    pub has_credential: bool,
    pub observe_enabled: bool,
    pub speak_enabled: bool,
    pub observe_seconds: u32,
    pub speak_seconds: u32,
    pub screen_scope: String,
    pub monitor_id: Option<u32>,
    pub excluded_apps: Vec<String>,
    pub quiet_start: u8,
    pub quiet_end: u8,
    pub quiet_hours: bool,

    pub external_context: usize,
    pub local_model: String,
    pub local_context: usize,
    pub idle_seconds: u64,
    pub timeout_seconds: u64,
    pub output_tokens: usize,
    pub screen_backend: String,
    pub screen_service_url: String,
    pub screen_service_model: String,
    pub screen_has_credential: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            threads: 2,
            backend: "bundled".into(),
            service_url: "http://127.0.0.1:1234/v1".into(),
            service_model: String::new(),
            remote_screen: false,
            has_credential: false,
            observe_enabled: false,
            speak_enabled: false,
            observe_seconds: 60,
            speak_seconds: 600,
            screen_scope: "window".into(),
            monitor_id: None,
            excluded_apps: Vec::new(),
            quiet_start: 22,
            quiet_end: 8,
            quiet_hours: true,

            external_context: 8192,
            local_model: "2b".into(),
            local_context: 4096,
            idle_seconds: 300,
            timeout_seconds: 180,
            output_tokens: 192,
            screen_backend: "bundled".into(),
            screen_service_url: "http://127.0.0.1:1234/v1".into(),
            screen_service_model: String::new(),
            screen_has_credential: false,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if !(2048..=8192).contains(&self.local_context)
            || !(30..=1800).contains(&self.idle_seconds)
            || !(30..=300).contains(&self.timeout_seconds)
            || !(32..=512).contains(&self.output_tokens)
        {
            return Err("上下文、输出、超时或空闲释放设置无效。".into());
        }
        if !["2b", "small"].contains(&self.local_model.as_str()) {
            return Err("未知本地模型。".into());
        }
        if !(1..=4).contains(&self.threads) {
            return Err("CPU线程数必须在1到4之间。".into());
        }
        if !["bundled", "external"].contains(&self.backend.as_str())
            || self.service_model.len() > 128
            || !(1024..=32768).contains(&self.external_context)
        {
            return Err("模型后端或上下文设置无效。".into());
        }
        if !(15..=3600).contains(&self.observe_seconds)
            || !(60..=7200).contains(&self.speak_seconds)
            || !["window", "monitor", "all"].contains(&self.screen_scope.as_str())
            || self.excluded_apps.len() > 20
            || self.excluded_apps.iter().any(|x| x.len() > 128)
            || self.quiet_start > 23
            || self.quiet_end > 23
        {
            return Err("观察间隔、范围或安静时段设置无效。".into());
        }
        crate::ai_external::validate_url(&self.service_url)?;
        crate::ai_external::validate_url(&self.screen_service_url)?;
        if !["bundled", "external"].contains(&self.screen_backend.as_str())
            || self.screen_service_model.len() > 128
            || (self.screen_backend == "external" && self.screen_service_model.trim().is_empty())
        {
            return Err("请填写有效的屏幕理解服务和模型名。".into());
        }
        if self.backend == "external" && self.service_model.trim().is_empty() {
            return Err("请填写服务的模型名称。".into());
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preference {
    pub key: String,
    pub value: String,
    pub origin: String,
    pub revision: String,
    pub source_message_id: Option<i64>,
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
        if version > 3 {
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
        if version < 2 {
            let tx = conn.transaction().map_err(db_error)?;
            tx.execute_batch("ALTER TABLE user_preferences ADD COLUMN revision TEXT NOT NULL DEFAULT ''; CREATE UNIQUE INDEX preference_key_scope ON user_preferences(key,scope); CREATE TABLE preference_changes(revision TEXT PRIMARY KEY,key TEXT NOT NULL,old_data TEXT,created_at INTEGER NOT NULL); PRAGMA user_version=2;").map_err(db_error)?;
            tx.commit().map_err(db_error)?;
        }
        if version < 3 {
            conn.execute_batch("BEGIN; CREATE TABLE credential_refs(url TEXT PRIMARY KEY); PRAGMA user_version=3; COMMIT;").map_err(db_error)?;
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
        let mut profile = serde_json::from_str::<Personality>(&text)
            .map_err(db_error)?
            .validated()?;
        for p in self.preferences()? {
            match p.key.as_str() {
                "owner_name" => profile.owner_name = p.value,
                "speaking_style" if p.value == "brief" => profile.length = "short".into(),
                "speaking_style" if ["gentle", "playful"].contains(&p.value.as_str()) => {
                    profile.tone = p.value
                }
                _ => {}
            }
        }
        profile.validated()
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
    #[cfg(test)]
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
    pub fn credential_urls(&self) -> Result<Vec<String>> {
        let mut q = self
            .conn
            .prepare("SELECT url FROM credential_refs")
            .map_err(db_error)?;
        let result = q
            .query_map([], |r| r.get(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<String>, _>>()
            .map_err(db_error);
        result
    }
    pub fn credential_ref(&self, url: &str, present: bool) -> Result<()> {
        if present {
            self.conn
                .execute("INSERT OR IGNORE INTO credential_refs VALUES(?1)", [url])
                .map_err(db_error)?;
        } else {
            self.conn
                .execute("DELETE FROM credential_refs WHERE url=?1", [url])
                .map_err(db_error)?;
        }
        Ok(())
    }
    pub fn reset_personal_data(&mut self) -> Result<()> {
        let tx = self.conn.transaction().map_err(db_error)?;
        tx.execute_batch("DELETE FROM chat_messages; DELETE FROM user_preferences; DELETE FROM preference_changes; DELETE FROM credential_refs;").map_err(db_error)?;
        tx.execute(
            "UPDATE personality_profiles SET data=?1 WHERE id=1",
            [serde_json::to_string(&Personality::default()).map_err(db_error)?],
        )
        .map_err(db_error)?;
        tx.execute(
            "UPDATE ai_settings SET data=?1 WHERE id=1",
            [serde_json::to_string(&Settings::default()).map_err(db_error)?],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)
    }
    pub fn save_profile_with_preferences(&mut self, profile: &Personality, now: i64) -> Result<()> {
        let old = self.profile()?;
        let tx = self.conn.transaction().map_err(db_error)?;
        if old.owner_name != profile.owner_name {
            if profile.owner_name.is_empty() {
                tx.execute("DELETE FROM user_preferences WHERE key='owner_name'", [])
                    .map_err(db_error)?;
                tx.execute("DELETE FROM preference_changes WHERE key='owner_name'", [])
                    .map_err(db_error)?;
            } else {
                Self::write_preference(
                    &tx,
                    "owner_name",
                    &profile.owner_name,
                    "manual",
                    None,
                    now,
                )?;
            }
        }
        if old.tone != profile.tone || old.length != profile.length {
            tx.execute(
                "DELETE FROM user_preferences WHERE key='speaking_style'",
                [],
            )
            .map_err(db_error)?;
            tx.execute(
                "DELETE FROM preference_changes WHERE key='speaking_style'",
                [],
            )
            .map_err(db_error)?;
        }
        tx.execute(
            "UPDATE personality_profiles SET data=?1 WHERE id=1",
            [serde_json::to_string(profile).map_err(db_error)?],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)
    }
    pub fn begin(&mut self, request: &str, text: &str, now: i64) -> Result<i64> {
        self.cleanup(now)?;
        let tx = self.conn.transaction().map_err(db_error)?;
        tx.execute("INSERT INTO chat_messages(session_id, role, content, created_at, status, request_id) VALUES(1,'user',?1,?2,'complete',?3)", params![text,now,request]).map_err(db_error)?;
        let user_id = tx.last_insert_rowid();
        if let Some((key, value)) = crate::ai_memory::extract(text) {
            Self::write_preference(&tx, &key, &value, "user_chat", Some(user_id), now)?;
        }
        tx.execute("INSERT INTO chat_messages(session_id, role, content, created_at, status, request_id) VALUES(1,'assistant','',?1,'generating',?2)", params![now,request]).map_err(db_error)?;
        let id = tx.last_insert_rowid();
        tx.commit().map_err(db_error)?;
        Ok(id)
    }

    fn write_preference(
        conn: &Connection,
        key: &str,
        value: &str,
        origin: &str,
        source: Option<i64>,
        now: i64,
    ) -> Result<()> {
        let old: Option<Preference> = conn.query_row("SELECT key,value,origin,revision,source_message_id FROM user_preferences WHERE key=?1 AND scope='global'", [key], |r| Ok(Preference {key:r.get(0)?,value:r.get(1)?,origin:r.get(2)?,revision:r.get(3)?,source_message_id:r.get(4)?})).optional().map_err(db_error)?;
        if old.as_ref().is_some_and(|p| p.value == value) {
            return Ok(());
        }
        let revision = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO preference_changes VALUES(?1,?2,?3,?4)",
            params![
                revision,
                key,
                old.map(|p| serde_json::to_string(&p))
                    .transpose()
                    .map_err(db_error)?,
                now
            ],
        )
        .map_err(db_error)?;
        conn.execute("INSERT INTO user_preferences(key,value,scope,origin,updated_at,source_message_id,revision) VALUES(?1,?2,'global',?3,?4,?5,?6) ON CONFLICT(key,scope) DO UPDATE SET value=excluded.value,origin=excluded.origin,updated_at=excluded.updated_at,source_message_id=excluded.source_message_id,revision=excluded.revision",params![key,value,origin,now,source,revision]).map_err(db_error)?;
        conn.execute("DELETE FROM preference_changes WHERE revision NOT IN (SELECT revision FROM preference_changes ORDER BY created_at DESC,rowid DESC LIMIT 100)",[]).map_err(db_error)?;
        Ok(())
    }
    pub fn preferences(&self) -> Result<Vec<Preference>> {
        let mut q=self.conn.prepare("SELECT key,value,origin,revision,source_message_id FROM user_preferences WHERE scope='global' ORDER BY key").map_err(db_error)?;
        let rows = q
            .query_map([], |r| {
                Ok(Preference {
                    key: r.get(0)?,
                    value: r.get(1)?,
                    origin: r.get(2)?,
                    revision: r.get(3)?,
                    source_message_id: r.get(4)?,
                })
            })
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error);
        rows
    }
    pub fn change_preference(&mut self, key: &str, value: Option<&str>, now: i64) -> Result<()> {
        if !["owner_name", "speaking_style", "companion_style"].contains(&key)
            && !(key.starts_with("like:") && key.chars().count() <= 45)
        {
            return Err("未知偏好类型。".into());
        }
        if key == "speaking_style"
            && value.is_some_and(|v| !["brief", "gentle", "playful"].contains(&v))
        {
            return Err("说话方式请选择简短、温柔或活泼。".into());
        }
        if key == "companion_style" && value.is_some_and(|v| !["quiet", "normal"].contains(&v)) {
            return Err("陪伴方式请选择安静或正常。".into());
        }
        if key == "owner_name" && value.is_some_and(|v| v.chars().count() > 16) {
            return Err("称呼最多16字。".into());
        }
        if let Some(value) = value {
            if value.trim().is_empty() || value.chars().count() > 64 {
                return Err("偏好需为1到64字。".into());
            }
        }
        let mut baseline = self.profile()?;
        let tx = self.conn.transaction().map_err(db_error)?;
        if let Some(value) = value {
            Self::write_preference(&tx, key, value.trim(), "manual", None, now)?;
        } else {
            tx.execute(
                "DELETE FROM user_preferences WHERE key=?1 AND scope='global'",
                [key],
            )
            .map_err(db_error)?;
            tx.execute("DELETE FROM preference_changes WHERE key=?1", [key])
                .map_err(db_error)?;
            if key == "owner_name" {
                baseline.owner_name.clear();
                tx.execute(
                    "UPDATE personality_profiles SET data=?1 WHERE id=1",
                    [serde_json::to_string(&baseline).map_err(db_error)?],
                )
                .map_err(db_error)?;
            }
        }
        tx.commit().map_err(db_error)
    }
    pub fn undo_preference(&mut self, revision: &str, now: i64) -> Result<()> {
        let tx = self.conn.transaction().map_err(db_error)?;
        let (key, old): (String, Option<String>) = tx
            .query_row(
                "SELECT key,old_data FROM preference_changes WHERE revision=?1",
                [revision],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| "该记忆已无法撤销。".to_string())?;
        let active: Option<String> = tx
            .query_row(
                "SELECT revision FROM user_preferences WHERE key=?1 AND scope='global'",
                [&key],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_error)?;
        if active.as_deref() != Some(revision) {
            return Err("偏好已经修改，不能撤销旧记录。".into());
        }
        tx.execute(
            "DELETE FROM user_preferences WHERE key=?1 AND scope='global'",
            [&key],
        )
        .map_err(db_error)?;
        if let Some(old) = old {
            let p: Preference = serde_json::from_str(&old).map_err(db_error)?;
            tx.execute("INSERT INTO user_preferences(key,value,scope,origin,updated_at,revision) VALUES(?1,?2,'global',?3,?4,?5)",params![p.key,p.value,p.origin,now,p.revision]).map_err(db_error)?;
        }
        tx.execute(
            "DELETE FROM preference_changes WHERE revision=?1",
            [revision],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)
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
    #[test]
    fn deleting_name_clears_manual_baseline_and_full_reset_removes_references() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("db"), 100).unwrap();
        let mut profile = Personality::default();
        profile.owner_name = "旧名字".into();
        store.save_profile_with_preferences(&profile, 100).unwrap();
        store.change_preference("owner_name", None, 101).unwrap();
        assert!(store.profile().unwrap().owner_name.is_empty());
        store
            .credential_ref("https://example.com/v1", true)
            .unwrap();
        store.begin("name", "我喜欢你叫我小王", 102).unwrap();
        store.reset_personal_data().unwrap();
        assert!(store.preferences().unwrap().is_empty());
        assert!(store.messages(102).unwrap().is_empty());
        assert!(store.credential_urls().unwrap().is_empty());
        assert_eq!(store.profile().unwrap(), Personality::default());
        assert_eq!(store.settings().unwrap(), Settings::default());
    }
    #[test]
    fn explicit_memory_survives_chat_cleanup_and_rejects_stale_undo() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = super::Store::open(&dir.path().join("ai.sqlite3"), 100).unwrap();
        store.begin("name-one", "我喜欢你叫我小王", 100).unwrap();
        let first = store.preferences().unwrap()[0].clone();
        store
            .begin("name-two", "以后叫我老王，不叫小王了", 101)
            .unwrap();
        let second = store.preferences().unwrap()[0].clone();
        assert!(store.undo_preference(&first.revision, 102).is_err());
        store.undo_preference(&second.revision, 102).unwrap();
        assert_eq!(store.preferences().unwrap()[0].value, "小王");
        store.cleanup(102 + super::RETENTION_SECONDS).unwrap();
        assert_eq!(store.preferences().unwrap()[0].value, "小王");
        store
            .change_preference("owner_name", None, 103 + super::RETENTION_SECONDS)
            .unwrap();
        assert!(store.preferences().unwrap().is_empty());
        assert!(store
            .undo_preference(&first.revision, 104 + super::RETENTION_SECONDS)
            .is_err());
    }
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
