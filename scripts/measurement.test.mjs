import test from "node:test";
import assert from "node:assert/strict";
import { mergeChannel, trendSegments } from "../src/types/measurement.ts";
const sample = (sequence,elapsed_us,value,quality="good") => ({sequence,elapsed_us,timestamp_ms:0,quality,value:value===null?null:{kind:"float",value}});
const state = (samples, session_id="a",capacity=3) => ({session_id,key:"x",latest:samples.at(-1),samples,history_capacity:capacity});
test("incremental merge, deduplication, bounded history and stale snapshot",()=>{
  const a=sample(1,100,0), b=sample(2,200,-50), c=sample(3,300,50), d=sample(4,400,0);
  let merged=mergeChannel(state([a,b]),state([b,c,d]));
  assert.deepEqual(merged.samples.map(x=>x.sequence),[2,3,4]);
  merged=mergeChannel(merged,state([a,b]));
  assert.equal(merged.latest.sequence,4);
  assert.deepEqual(merged.samples.map(x=>x.sequence),[2,3,4]);
  assert.equal(mergeChannel(merged,state([a],"new")).samples.length,1);
});
test("negative values, real elapsed x spacing and explicit gaps",()=>{
  const result=trendSegments([sample(1,0,50),sample(2,100000,-50),sample(3,300000,null,"timeout"),sample(4,500000,0)],500000,500);
  assert.equal(result.zero,60);
  assert.equal(result.segments.length,2);
  assert.equal(result.segments[0],"0.00,0.00 120.00,120.00");
  assert.equal(result.segments[1],"600.00,60.00");
  assert.equal(trendSegments([sample(1,0,null,"timeout")],0,500),null);
});
