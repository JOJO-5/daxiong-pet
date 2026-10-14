import test from 'node:test';
import assert from 'node:assert/strict';
import {replyMotion} from '../src/reply-motion.ts';
test('serious topics override question and happiness cues',()=>{
  assert.equal(replyMotion('你累了，先歇一会儿好吗？'),'calm');
  assert.equal(replyMotion('我知道你难过，之后会有开心的事，太好了也别急。'),'calm');
  assert.equal(replyMotion('这次失败了，对不起。'),'calm');
});
test('questions, explicit celebration and ordinary replies have distinct cues',()=>{
  assert.equal(replyMotion('要一起玩吗？'),'curious');
  assert.equal(replyMotion('太好了，你做到了！'),'happy');
  assert.equal(replyMotion('我在这里陪你。'),'gentle');
});
