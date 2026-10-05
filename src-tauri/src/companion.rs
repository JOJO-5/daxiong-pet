//! Positive, local-only memories. Nothing decays while the app is closed.
use serde::{Deserialize, Serialize};
use std::{io::{self, Write}, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Memory {
    pub version: u32,
    pub nickname: String,
    pub affection: u32,
    pub encounters_enabled: bool,
    pub playful_fetch: bool,
    pub quiet_companion: bool,
    pub training: [u8;4],
    pub treats: u32,
    pub fetches: u32,
    pub pats: u32,
    pub last_treat: Option<u64>,
    pub last_pat: Option<u64>,
    pub last_fetch_reward: Option<u64>,
}

impl Default for Memory {
    fn default() -> Self {
        Self { version:1, nickname:String::new(), affection:0, treats:0, fetches:0, pats:0,
            last_treat:None,last_pat:None,last_fetch_reward:None,encounters_enabled:true,playful_fetch:true,quiet_companion:false,training:[0;4] }
    }
}

#[derive(Clone, Serialize)]
pub struct MemoryView {
    pub nickname: String,
    pub affection: u32,
    pub encounters_enabled: bool,
    pub playful_fetch: bool,
    pub quiet_companion: bool,
    pub training: [u8;4],
    pub stage: &'static str,
    pub treats: u32,
    pub fetches: u32,
    pub pats: u32,
    pub treat_wait: u64,
    pub error: Option<String>,
}

#[derive(Clone, Copy)]
pub enum Reward { Treat, Fetch, Pat }

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d|d.as_secs()).unwrap_or(0)
}

fn wait(last: Option<u64>, clock: u64, cooldown: u64) -> u64 {
    // A clock correction must not lock the interaction for years.
    last.map(|t| if clock<t { 0 } else { cooldown.saturating_sub(clock-t) }).unwrap_or(0)
}

impl Memory {
    pub fn load(path: &Path) -> io::Result<Self> {
        let bytes = match std::fs::read(path) {
            Ok(v)=>v, Err(e) if e.kind()==io::ErrorKind::NotFound=>return Ok(Self::default()), Err(e)=>return Err(e)
        };
        let memory:Self=serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        if memory.version != 1 || memory.affection > 1000 || memory.training.iter().any(|n| *n>3) || Self::validated_nickname(&memory.nickname).is_err() {
            return Err(io::Error::other("陪伴记忆格式无效或版本不支持"));
        }
        Ok(memory)
    }
    pub fn save(&self,path:&Path) -> io::Result<()> {
        let dir=path.parent().ok_or_else(||io::Error::other("记忆目录无效"))?;
        std::fs::create_dir_all(dir)?;
        let mut file=tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer_pretty(&mut file,self).map_err(io::Error::other)?;
        file.flush()?;file.as_file().sync_all()?;
        file.persist(path).map_err(|e|e.error)?;
        Ok(())
    }
    pub fn validated_nickname(value:&str) -> Result<String,String> {
        if value.chars().any(char::is_control) { return Err("昵称不能包含换行或控制字符".into()); }
        let value=value.trim();
        if value.chars().count()>16 { return Err("昵称最多 16 个字".into()); }
        Ok(value.into())
    }
    pub fn view(&self,clock:u64,error:Option<String>) -> MemoryView {
        MemoryView { nickname:self.nickname.clone(), affection:self.affection, encounters_enabled:self.encounters_enabled,playful_fetch:self.playful_fetch,quiet_companion:self.quiet_companion,training:self.training,stage:match self.affection {
            0..=19=>"初次相识",20..=99=>"越来越熟",100..=299=>"默契伙伴",_=>"最好的朋友"
        },treats:self.treats,fetches:self.fetches,pats:self.pats,treat_wait:wait(self.last_treat,clock,20),error }
    }
    /// Returns whether anything changed. Callers commit to RAM only after atomic save succeeds.
    pub fn reward(&mut self,reward:Reward,clock:u64) -> Result<bool,String> {
        let points=match reward {
            Reward::Treat=>{
                let left=wait(self.last_treat,clock,20);
                if left>0 { return Err(format!("还在嚼饼干呢，{left} 秒后再喂吧")); }
                self.last_treat=Some(clock);self.treats=self.treats.saturating_add(1);4
            }
            Reward::Fetch=>{
                self.fetches=self.fetches.saturating_add(1);
                if wait(self.last_fetch_reward,clock,15)>0 { 0 } else { self.last_fetch_reward=Some(clock);3 }
            }
            Reward::Pat=>{
                if wait(self.last_pat,clock,60)>0 { return Ok(false); }
                self.last_pat=Some(clock);self.pats=self.pats.saturating_add(1);2
            }
        };
        self.affection=self.affection.saturating_add(points).min(1000);
        Ok(true)
    }
    pub fn reward_trick(&mut self,cue:crate::activities::Cue,clock:u64)->Result<bool,String> {
        self.reward(Reward::Treat,clock)?;
        self.training[cue.index()]=self.training[cue.index()].saturating_add(1).min(3);Ok(true)
    }
    pub fn address(&self) -> &str { if self.nickname.is_empty() { "你" } else { &self.nickname } }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quiet_preference_round_trips_and_old_memories_keep_existing_encounter_preference() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("memory.json");
        std::fs::write(&path,b"{\"version\":1,\"encounters_enabled\":true,\"nickname\":\"Jo\",\"affection\":20}").unwrap();
        let mut memory=Memory::load(&path).unwrap();assert!(!memory.quiet_companion);
        memory.quiet_companion=true;memory.save(&path).unwrap();
        let restored=Memory::load(&path).unwrap();assert!(restored.view(1_000_000,None).quiet_companion);
        assert!(restored.encounters_enabled);assert_eq!(restored.nickname,"Jo");assert_eq!(restored.affection,20);
    }
    #[test]
    fn training_reward_requires_real_treat_and_survives_restart_without_decay() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("memory.json");let mut m=Memory::default();
        for i in 0..3 {m.reward_trick(crate::activities::Cue::Stay,100+i*21).unwrap();}
        assert_eq!(m.training[3],3);m.save(&path).unwrap();let loaded=Memory::load(&path).unwrap();assert_eq!(loaded.training,m.training);
        assert!(m.reward_trick(crate::activities::Cue::Come,143).is_err());assert_eq!(m.training[0],0);
        let old=dir.path().join("old.json");std::fs::write(&old,b"{\"version\":1,\"nickname\":\"\",\"affection\":4}").unwrap();assert_eq!(Memory::load(&old).unwrap().training,[0;4]);
    }
    #[test]
    fn saved_memory_survives_offline_time_without_decay_and_cooldown_survives_restart() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("companion.json");
        let mut m=Memory::default();m.nickname=Memory::validated_nickname("  乔乔  ").unwrap();
        m.reward(Reward::Treat,100).unwrap();m.reward(Reward::Fetch,102).unwrap();m.reward(Reward::Pat,102).unwrap();
        m.save(&path).unwrap();let mut restored=Memory::load(&path).unwrap();
        assert_eq!(restored.nickname,"乔乔");assert_eq!(restored.affection,9);
        assert!(restored.reward(Reward::Treat,110).is_err());assert_eq!(restored.treats,1);
        assert_eq!(restored.view(1_000_000,None).affection,9);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(),1);
    }
    #[test]
    fn nickname_and_unknown_or_corrupt_save_are_rejected() {
        assert!(Memory::validated_nickname("a\nb").is_err());assert!(Memory::validated_nickname(&"熊".repeat(17)).is_err());
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("companion.json");
        for bytes in [b"broken".as_slice(),b"{\"version\":2}".as_slice(),b"{\"affection\":1001}".as_slice()] {
            std::fs::write(&path,bytes).unwrap();assert!(Memory::load(&path).is_err());assert_eq!(std::fs::read(&path).unwrap(),bytes);
        }
    }
    #[test]
    fn repeated_rewards_have_limits_and_a_failed_replace_preserves_disk() {
        let mut m=Memory::default();m.reward(Reward::Pat,100).unwrap();
        assert!(!m.reward(Reward::Pat,101).unwrap());assert_eq!(m.pats,1);
        for _ in 0..20 { m.reward(Reward::Fetch,101).unwrap(); }
        assert_eq!(m.fetches,20);assert_eq!(m.affection,5);
        let dir=tempfile::tempdir().unwrap();assert!(m.save(dir.path()).is_err());
    }
}
