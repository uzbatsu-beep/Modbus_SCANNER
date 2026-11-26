import test from "node:test";
import assert from "node:assert/strict";
import {DEFAULT_LAYOUT,parseLayout,clampPanel,fitLayout,groupDevices} from "../src/types/workbenchLayout.ts";
test("layout persistence validates schema and clamps invalid dimensions",()=>{
 assert.deepEqual(parseLayout("{"),DEFAULT_LAYOUT);
 assert.deepEqual(parseLayout('{"version":9,"left":300}'),DEFAULT_LAYOUT);
 const l=parseLayout(JSON.stringify({version:1,left:999,right:-10,bottom:200,leftVisible:false,rightVisible:"false",bottomTab:"traffic"}));
 assert.equal(l.left,480);assert.equal(l.right,180);assert.equal(l.bottom,200);assert.equal(l.leftVisible,false);assert.equal(l.rightVisible,true);assert.equal(l.bottomTab,"traffic");
 assert.deepEqual(parseLayout(JSON.stringify(l)),l);
 assert.equal(clampPanel("bottom",NaN),DEFAULT_LAYOUT.bottom);
});
test("layout fit retains center and never mutates saved widths or visibility",()=>{
 const state={...DEFAULT_LAYOUT,left:480,right:520,bottom:420};
 const a=fitLayout(state,1024,480);
 assert.ok(1024-a.left-a.right-12>=360);assert.ok(480-a.bottom-6>=200);
 assert.equal(fitLayout(state,600,250).right,0);
 assert.equal(fitLayout(state,400,250).left,0);
 assert.equal(fitLayout({...state,leftVisible:false,rightVisible:false,bottomVisible:false},1024,640).bottom,0);
 assert.equal(state.right,520);assert.equal(state.rightVisible,true);
});
test("grouping distinguishes port and framing and preserves unit identity",()=>{
 const r=(key,unit,port=502,serial_mode=null)=>({key,target:{kind:"tcp",endpoint:"localhost",port,unit_id:unit,serial:null,serial_mode}});
 const gs=groupDevices([r("b",2),r("a",1),r("c",1,503),r("d",1,502,"ascii")]);
 assert.equal(gs.length,3);
 assert.deepEqual(gs.find(g=>g.records.length===2).records.map(r=>r.key),["a","b"]);
});

test("fitted geometry remains bounded across viewport sizes and visibility permutations",()=>{
 for(const w of [320,526,546,600,712,800,1024,1920])for(const h of [200,306,480,640])for(const leftVisible of [true,false])for(const rightVisible of [true,false]){
 const s={...DEFAULT_LAYOUT,left:480,right:520,bottom:420,leftVisible,rightVisible};const x=fitLayout(s,w,h);
 assert.ok(x.left>=0&&x.right>=0&&x.bottom>=0);
 if(w>=360)assert.ok(w-x.left-x.right-(x.left?6:0)-(x.right?6:0)>=360);
 if(x.left)assert.ok(x.left>=160);if(x.right)assert.ok(x.right>=180);if(x.bottom)assert.ok(h-x.bottom-6>=200);
 }
});
