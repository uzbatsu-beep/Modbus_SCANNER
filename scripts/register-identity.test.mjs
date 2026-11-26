import test from "node:test";
import assert from "node:assert/strict";
import { addressSpace, readFunction, registerKey, readSpan, selectAdjacent, registerWidth } from "../src/types/registerIdentity.ts";
const r = (functionCode, address, raw) => ({function:functionCode,address,raw,as_hex:"",rtt_us:0});
const span = (address_space,address,quantity) => ({start:{device_key:"dev",address_space,address},quantity});
test("identical address in two tables does not collide", () => {
  const records = [r(3,0,0),r(4,0,22),r(3,1,1)];
  assert.notEqual(registerKey(3,0), registerKey(4,0));
  assert.deepEqual(readSpan(records,span("holding_registers",0,2)), [0,1]);
  assert.deepEqual(readSpan(records,span("input_registers",0,1)), [22]);
  assert.equal(readSpan(records,span("input_registers",0,2)), null);
});
test("selection never combines different FC or nonadjacent words", () => {
  const a=r(3,0,0), b=r(4,1,0), c=r(3,1,0), d=r(3,4,0);
  assert.deepEqual(selectAdjacent([a],b),[b]);
  assert.deepEqual(selectAdjacent([a],d),[d]);
  assert.deepEqual(selectAdjacent([c],a),[a,c]);
  assert.deepEqual(selectAdjacent([a,c],a),[c]);
});
test("four read spaces and boundary check", () => {
  for (const fc of [1,2,3,4]) assert.equal(readFunction(addressSpace(fc)),fc);
  assert.equal(addressSpace(6),undefined);
  assert.equal(readSpan([r(3,65535,0)],span("holding_registers",65535,2)),null);
  assert.equal(registerWidth("float32"),2);
  assert.equal(registerWidth("uint16"),1);
});
