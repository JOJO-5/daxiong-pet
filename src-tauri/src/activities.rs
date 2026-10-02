//! Short, cancellable tricks use the existing atlas and physical desktop bounds.
use crate::{atlas::Row,engine::{Input,PET_X,PET_Y,PET_W,PET_H}};
use serde::Serialize;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize)]
#[serde(rename_all="lowercase")]
pub enum Cue { Come, Spin, Down, Stay }
impl Cue {
    pub fn parse(s:&str)->Result<Self,String> {match s {"come"=>Ok(Self::Come),"spin"=>Ok(Self::Spin),"down"=>Ok(Self::Down),"stay"=>Ok(Self::Stay),_=>Err("未知指令".into())}}
    pub fn index(self)->usize {match self {Self::Come=>0,Self::Spin=>1,Self::Down=>2,Self::Stay=>3}}
    pub fn label(self)->&'static str {match self {Self::Come=>"过来",Self::Spin=>"转圈",Self::Down=>"趴下",Self::Stay=>"等一下"}}
}
#[derive(Clone,Debug,PartialEq,Serialize)]
pub struct ActivityView {pub id:u64,pub kind:Option<Cue>,pub phase:&'static str,pub rewardable:bool}
pub struct ActivityStep {pub movement:Option<(i32,i32)>,pub row:Option<Row>,pub col:Option<usize>,pub completed:bool}
pub struct Activities {view:ActivityView,elapsed:u64,attention:u64,home:(i32,i32),target:Option<(i32,i32)>}
impl Default for Activities {fn default()->Self {Self {view:ActivityView{id:0,kind:None,phase:"off",rewardable:false},elapsed:0,attention:700,home:(0,0),target:None}}}
impl Activities {
    pub fn view(&self)->ActivityView {self.view.clone()}
    pub fn active(&self)->bool {matches!(self.view.phase,"attention" | "performing")}
    pub fn cancel(&mut self) {self.view.kind=None;self.view.phase="off";self.view.rewardable=false;self.target=None;}
    pub fn blocked(&mut self) {self.cancel();self.view.phase="blocked";}
    pub fn start(&mut self,cue:Cue,input:&Input,learned:bool) {
        self.view=ActivityView{id:self.view.id.saturating_add(1),kind:Some(cue),phase:"attention",rewardable:false};
        self.elapsed=0;self.attention=if learned {300} else {700};self.home=input.win_pos;self.target=None;
    }
    fn bound(input:&Input,pos:(i32,i32))->(i32,i32) {
        let (sx,sy,sw,sh)=input.screen;
        (pos.0.clamp(sx,sx+(sw-input.win_size.0).max(0)),pos.1.clamp(sy,sy+(sh-input.win_size.1).max(0)))
    }
    pub fn tick(&mut self,input:&Input)->ActivityStep {
        let mut out=ActivityStep{movement:None,row:None,col:None,completed:false};
        if !self.active() {return out;}
        if !input.interactive || input.screen.2<input.win_size.0 || input.screen.3<input.win_size.1 {self.cancel();return out;}
        self.elapsed=self.elapsed.saturating_add(input.dt_ms.min(100));
        let s=input.scale_factor as f32;
        let cue=self.view.kind.unwrap();
        if self.elapsed<self.attention {
            let dx=input.cursor.0-(input.win_pos.0+((PET_X+PET_W/2) as f32*s) as i32);
            let dy=input.cursor.1-(input.win_pos.1+((PET_Y+PET_H/2) as f32*s) as i32);
            let angle=(dx as f32).atan2(-(dy as f32)).to_degrees().rem_euclid(360.0);
            let index=((angle/22.5).round() as usize)%16;
            out.row=Some(if index<8 {Row::LookA} else {Row::LookB});out.col=Some(index%8);return out;
        }
        self.view.phase="performing";
        let t=self.elapsed-self.attention;
        let done=match cue {
            Cue::Come=>{
                let target=*self.target.get_or_insert_with(||Self::bound(input,(input.cursor.0-((PET_X+PET_W/2) as f32*s) as i32,input.cursor.1-((PET_Y+PET_H/2) as f32*s) as i32)));
                let target=Self::bound(input,target);
                let dx=(target.0-input.win_pos.0) as f32;let dy=(target.1-input.win_pos.1) as f32;let d=dx.hypot(dy);
                let step=240.0*s*input.dt_ms.min(50) as f32/1000.0;
                let pos=if d<=step || d<1.0 {target} else {(input.win_pos.0+(dx/d*step).round() as i32,input.win_pos.1+(dy/d*step).round() as i32)};
                out.movement=Some(Self::bound(input,pos));out.row=Some(if dx>=0.0 {Row::RunRight} else {Row::RunLeft});
                if t>12_000 && d>8.0*s {self.cancel();self.view.phase="cancelled";return out;}
                d<=8.0*s
            }
            Cue::Spin=>{
                let angle=(t.min(1600) as f32/1600.0)*std::f32::consts::TAU;
                let pos=(self.home.0+(angle.sin()*28.0*s).round() as i32,self.home.1+((angle.cos()-1.0)*28.0*s).round() as i32);
                out.movement=Some(Self::bound(input,pos));out.row=Some(if angle.cos()>=0.0 {Row::RunRight} else {Row::RunLeft});t>=1600
            }
            Cue::Down=>{out.row=Some(if input.extra_animations {Row::Sleep} else {input.sleep_frame.0});out.col=Some(if input.extra_animations {0} else {input.sleep_frame.1});t>=1800}
            Cue::Stay=>{out.row=Some(Row::Waiting);out.col=Some(0);t>=2200}
        };
        if done {out.completed=true;self.view.phase="completed";self.view.rewardable=true;out.row=Some(if input.extra_animations {Row::HappyPat} else {Row::Waving});out.col=None;}
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn input(scale:f64)->Input {Input{dt_ms:16,interactive:true,cursor:(-700,550),win_pos:(-1300,300),win_size:((300.0*scale) as i32,(240.0*scale) as i32),scale_factor:scale,screen:(-1920,0,1920,1080),button_down:false,look_enabled:true,extra_animations:true,encounters_enabled:true,gravity:false,local_hour:12,sleep_frame:(Row::Failed,2)}}
    #[test]
    fn all_tricks_complete_with_bounds_and_legacy_frames() {
        for scale in [1.0,1.25,2.0] {for builtin in [false,true] {for cue in [Cue::Come,Cue::Spin,Cue::Down,Cue::Stay] {
            let mut i=input(scale);i.extra_animations=builtin;let mut a=Activities::default();a.start(cue,&i,false);let home=i.win_pos;
            for _ in 0..1000 {
                let step=a.tick(&i);if let Some(row)=step.row {assert!(builtin || (row as u8)<11);}
                if let Some(p)=step.movement {assert!(p.0>=-1920 && p.0<=-i.win_size.0 && p.1>=0 && p.1<=1080-i.win_size.1);i.win_pos=p;}
                if a.view().phase=="completed" {break;}
            }
            assert_eq!(a.view().phase,"completed");assert!(a.view().rewardable);
            if matches!(cue,Cue::Down | Cue::Stay | Cue::Spin) {assert_eq!(i.win_pos,home);}
            a.cancel();assert!(!a.active());assert!(!a.view().rewardable);
        }}}
    }
    #[test]
    fn learned_attention_is_shorter_and_hidden_commands_cancel() {
        let mut i=input(1.0);i.dt_ms=400;let mut a=Activities::default();a.start(Cue::Stay,&i,true);
        for _ in 0..3 {a.tick(&i);}assert_eq!(a.view().phase,"performing");
        a.start(Cue::Stay,&i,false);for _ in 0..3 {a.tick(&i);}assert_eq!(a.view().phase,"attention");
        i.interactive=false;a.tick(&i);assert_eq!(a.view().phase,"off");assert!(!a.view().rewardable);
    }
}
