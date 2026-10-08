//! Gentle pointer contact, separate from the press-and-drag gesture.
use crate::atlas::Row;

const DWELL_MS: u64 = 1200;
const GRACE_MS: u64 = 350;
const COOLDOWN_MS: u64 = 6000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Zone { Head, Belly }

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Feedback {
    pub kind: Zone,
    pub phase: &'static str,
    pub progress: u8,
    pub x: i32,
    pub y: i32,
    pub stroking: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode { Idle, Head, Down, Belly, Up, LegacyBelly }

pub struct Step {
    pub engaged: bool,
    pub started: Option<Zone>,
    pub frame: Option<(Row, usize)>,
    pub feedback: Option<Feedback>,
}

pub struct Petting {
    mode: Mode,
    candidate: Option<Zone>,
    dwell: u64,
    elapsed: u64,
    outside: u64,
    cooldown: u64,
    last: Option<(f32, f32)>,
    builtin: bool,
}

impl Default for Petting {
    fn default() -> Self {
        Self { mode: Mode::Idle, candidate: None, dwell: 0, elapsed: 0,
            outside: 0, cooldown: 0, last: None, builtin: false }
    }
}

impl Petting {
    pub fn cancel(&mut self) {
        self.mode = Mode::Idle;
        self.candidate = None;
        self.dwell = 0;
        self.elapsed = 0;
        self.outside = 0;
        self.last = None;
    }

    fn standing_zone(x: f32, y: f32) -> Option<Zone> {
        if (10.0..=104.0).contains(&x) && (10.0..=86.0).contains(&y) {
            Some(Zone::Head)
        } else if (76.0..=133.0).contains(&x) && (91.0..=136.0).contains(&y) {
            Some(Zone::Belly)
        } else { None }
    }

    /// x/y are logical coordinates within the 144×156 pet, independent of DPI.
    pub fn tick(&mut self, dt: u64, x: f32, y: f32, allowed: bool, builtin: bool) -> Step {
        self.cooldown = self.cooldown.saturating_sub(dt);
        if builtin != self.builtin { self.cancel(); self.builtin = builtin; }
        if !allowed {
            self.cancel();
            return Step { engaged: false, started: None, frame: None, feedback: None };
        }
        let distance = self.last.map_or(0.0, |(a,b)| (x-a).hypot(y-b));
        let gentle = distance <= 550.0 * dt.max(1) as f32 / 1000.0;
        self.last = Some((x,y));
        let standing = Self::standing_zone(x,y);
        // Side lying keeps the cheek, shoulder and hip low; contact follows the underside.
        let lying_belly = (65.0..=130.0).contains(&x) && (107.0..=144.0).contains(&y);
        let lying_head = (8.0..=64.0).contains(&x) && (108.0..=149.0).contains(&y);
        let belly_contact = lying_belly || lying_head
            || (self.mode == Mode::Down && standing == Some(Zone::Belly));
        let contact = gentle && match self.mode {
            Mode::Head => standing == Some(Zone::Head),
            Mode::Down | Mode::Belly => belly_contact,
            Mode::LegacyBelly => standing == Some(Zone::Belly),
            Mode::Up => false,
            Mode::Idle => standing.is_some(),
        };
        let mut started = None;
        if self.mode == Mode::Idle {
            let candidate = if gentle { standing } else { None };
            if candidate != self.candidate { self.dwell = 0; self.candidate = candidate; }
            if candidate.is_some() { self.dwell += dt; } else { self.dwell = 0; }
            if self.dwell >= DWELL_MS && self.cooldown == 0 {
                started = candidate;
                self.mode = match candidate {
                    Some(Zone::Head) => Mode::Head,
                    Some(Zone::Belly) if builtin => Mode::Down,
                    Some(Zone::Belly) => Mode::LegacyBelly,
                    None => Mode::Idle,
                };
                self.elapsed = 0;
                self.outside = 0;
                self.cooldown = COOLDOWN_MS;
            }
        } else {
            self.elapsed += dt;
            if contact || (self.mode == Mode::Belly && self.elapsed < 1200) { self.outside = 0; } else { self.outside += dt; }
            match self.mode {
                Mode::Down if self.elapsed >= 720 => { self.mode = Mode::Belly; self.elapsed = 0; }
                Mode::Belly if self.outside >= GRACE_MS => { self.mode = Mode::Up; self.elapsed = 0; }
                Mode::Up if self.elapsed >= 720 => self.cancel(),
                Mode::Head | Mode::LegacyBelly if self.outside >= GRACE_MS => self.cancel(),
                _ => {}
            }
        }
        let frame = match self.mode {
            Mode::Head if builtin => Some((Row::HappyPat, ((self.elapsed / 180) % 8) as usize)),
            Mode::Head | Mode::LegacyBelly => Some((Row::Waving, ((self.elapsed / 180) % 4) as usize)),
            Mode::Down => Some((Row::BellyRoll, (self.elapsed / 180).min(3) as usize)),
            Mode::Belly => Some((Row::BellyRub, ((self.elapsed / 220) % 8) as usize)),
            Mode::Up => Some((Row::BellyRoll, 4 + (self.elapsed / 180).min(3) as usize)),
            Mode::Idle => None,
        };
        let kind = match self.mode {
            Mode::Head => Some(Zone::Head),
            Mode::Belly if lying_head => Some(Zone::Head),
            Mode::Down | Mode::Belly | Mode::Up | Mode::LegacyBelly => Some(Zone::Belly),
            Mode::Idle => self.candidate,
        };
        let feedback = kind.filter(|_| contact && (self.mode != Mode::Idle || self.dwell >= 150)).map(|kind| Feedback {
            kind,
            phase: match self.mode { Mode::Idle => "waiting", Mode::Head => "head", Mode::Down => "down",
                Mode::Belly if lying_head => "head", Mode::Belly | Mode::LegacyBelly => "belly", Mode::Up => "up" },
            progress: if self.mode == Mode::Idle { ((self.dwell.min(DWELL_MS) * 100) / DWELL_MS) as u8 } else { 100 },
            x: if self.mode == Mode::Down { 170 } else { (x + 78.0).round() as i32 },
            y: if self.mode == Mode::Down { [194,194,206,210][(self.elapsed/180).min(3) as usize] } else { (y + 84.0).round() as i32 },
            stroking: contact && distance >= 0.5,
        });
        Step { engaged: contact || frame.is_some(), started, frame, feedback }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dwell(p: &mut Petting, x:f32,y:f32,builtin:bool) -> Step {
        let mut result=p.tick(16,x,y,true,builtin);
        for _ in 0..80 { result=p.tick(16,x,y,true,builtin); }
        result
    }
    #[test]
    fn head_and_belly_have_distinct_poses_and_require_contact() {
        let mut head=Petting::default();assert_eq!(dwell(&mut head,65.0,45.0,true).frame.unwrap().0,Row::HappyPat);
        let mut belly=Petting::default();assert_eq!(dwell(&mut belly,100.0,115.0,true).frame.unwrap().0,Row::BellyRoll);
        let mut feet=Petting::default();assert!(dwell(&mut feet,30.0,140.0,true).frame.is_none());
    }
    #[test]
    fn switching_zones_resets_dwell_and_a_fast_pass_does_not_pet() {
        let mut p=Petting::default();for _ in 0..40 {p.tick(16,65.0,45.0,true,true);}
        for _ in 0..40 {assert!(p.tick(16,100.0,115.0,true,true).started.is_none());}
        let mut fast=Petting::default();for i in 0..100 {assert!(fast.tick(16,if i%2==0 {15.0}else{100.0},45.0,true,true).frame.is_none());}
    }
    #[test]
    fn belly_rubs_loop_then_roll_back_up_without_repeating_reward() {
        let mut p=Petting::default();let mut rewards=0;
        for _ in 0..130 {let s=p.tick(16,100.0,115.0,true,true);rewards+=usize::from(s.started.is_some());}
        p.tick(120,95.0,127.0,true,true);
        for _ in 0..650 {let s=p.tick(16,95.0,127.0,true,true);rewards+=usize::from(s.started.is_some());}
        assert_eq!(rewards,1);assert_eq!(p.mode,Mode::Belly);
        let mut saw_up=false;for _ in 0..100 {let s=p.tick(16,-500.0,-500.0,true,true);saw_up|=s.frame.is_some_and(|(r,c)|r==Row::BellyRoll&&c>=4);}
        assert!(saw_up);assert_eq!(p.mode,Mode::Idle);
    }
    #[test]
    fn brief_contact_gap_does_not_pop_out_of_pose_and_press_cancels() {
        let mut p=Petting::default();dwell(&mut p,65.0,45.0,true);
        assert!(p.tick(100,-500.0,-500.0,true,true).frame.is_some());
        assert!(p.tick(16,65.0,45.0,true,true).frame.is_some());
        assert!(p.tick(16,65.0,45.0,false,true).feedback.is_none());assert_eq!(p.mode,Mode::Idle);
    }
    #[test]
    fn touch_follows_the_rolled_anatomy_and_head_can_be_petted_while_lying() {
        let mut p=Petting::default();dwell(&mut p,100.0,115.0,true);
        for _ in 0..60 {p.tick(16,100.0,115.0,true,true);}
        let belly=p.tick(120,95.0,127.0,true,true);
        assert_eq!(belly.feedback.unwrap().kind,Zone::Belly);
        let head=p.tick(180,35.0,125.0,true,true);
        assert_eq!(head.feedback.unwrap().kind,Zone::Head);
        assert_eq!(head.frame.unwrap().0,Row::BellyRub);
        assert!(head.started.is_none());
        // The old inverted pose's belly position is now empty space above the dog.
        assert!(p.tick(180,95.0,60.0,true,true).feedback.is_none());
    }
    #[test]
    fn old_pets_never_get_new_rows_and_switch_cancels_the_roll() {
        let mut p=Petting::default();assert_eq!(dwell(&mut p,100.0,115.0,false).frame.unwrap().0,Row::Waving);
        p.cancel();p.cooldown=0;dwell(&mut p,100.0,115.0,true);
        let next=p.tick(16,100.0,115.0,true,false);assert!(next.frame.is_none());
    }
}
