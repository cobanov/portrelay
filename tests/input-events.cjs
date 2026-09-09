// Pure event encoding/queue tests; this is not browser or hardware acceptance.
const { readFileSync } = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const context = { module: { exports: {} } };
vm.runInNewContext(readFileSync('app-ui/input-events.js', 'utf8'), context);
const { keys, Queue } = context.module.exports;
const plain = value => JSON.parse(JSON.stringify(value));
let q = new Queue();
q.move(0.4, -0.5); q.move(0.7, -0.6);
q.push({type:'key',code:keys.KeyA,value:1});
q.move(4, 5); q.move(6, -2);
q.push({type:'key',code:keys.KeyA,value:0});
assert.deepEqual(plain(q.take()), [
  {type:'move',x:1,y:-1}, {type:'key',code:30,value:1},
  {type:'move',x:10,y:3}, {type:'key',code:30,value:0},
]);
assert.equal(q.take().length,0);
q.wheel(0,50,0); q.wheel(0,50,0); q.wheel(3,0,1);
assert.deepEqual(plain(q.take()), [{type:'wheel',x:1,y:-1}]);
assert.equal(keys.Escape,undefined); assert.equal(keys.Power,undefined); assert.equal(keys.PrintScreen,undefined);
assert.equal(keys.ControlRight,97); assert.equal(keys.AltRight,100); assert.equal(keys.MetaLeft,125);
for(let i=0;i<64;i++) q.push({type:'button',button:0,down:i%2===0});
assert.throws(()=>q.push({type:'button',button:0,down:false}), /queue filled/);
q=new Queue(); assert.throws(()=>q.move(32768,0), /limit/);
console.log('PASS: fractional motion, ordered coalescing, wheel direction, physical keys, bounded queue and reserved Escape.');
