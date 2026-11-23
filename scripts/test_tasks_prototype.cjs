const {test}=require('node:test');
const assert=require('node:assert/strict');
const Cron=require('../docs/prototypes/tasks/cron.js');
test('five-field validation does not silently accept Quartz or invalid bounds',()=>{
 for(const pattern of ['0 0 9 * * ?','*/0 * * * *','60 * * * *','0 24 * * *','0 9 * 13 *','0 9 * * 8'])assert.throws(()=>Cron.parse(pattern));
 assert.deepEqual(Cron.parse('0 9 * * 7').fields[4].values,[0]);
 assert.deepEqual(Cron.parse('2-8/2 * * * *').fields[0].values,[2,4,6,8]);
});
test('visual round-trip is semantic and complex syntax is preserved in advanced mode',()=>{
 assert.equal(Cron.equivalent('0 9 * * 1-5','0 9 * * 1,2,3,4,5'),true);
 assert.equal(Cron.equivalent('0 9 1 * 1','0 9 * * 1'),false);
 assert.equal(Cron.visualFields('*/5 9-17 * * 1,3,5')[0].mode,'step');
 assert.throws(()=>Cron.visualFields('1,5-15/2 * * * *'),/不能无损回填/);
});
test('weekly preview uses task timezone and returns ten future triggers',async()=>{
 const next=await Cron.next('0 9 * * 1-5','Asia/Shanghai',10,new Date('2026-11-20T02:00:00Z'));
 assert.equal(next.length,10);assert.equal(next[0].toISOString(),'2026-11-23T01:00:00.000Z');
 assert.ok(next.every(date=>date>new Date('2026-11-20T02:00:00Z')));
});
test('day-of-month and weekday use Unix OR when both restricted',async()=>{
 const next=await Cron.next('0 9 1 * 1','UTC',3,new Date('2026-11-19T00:00:00Z'));
 assert.deepEqual(next.map(date=>date.toISOString().slice(0,10)),['2026-11-23','2026-11-30','2026-12-01']);
});
test('DST spring gap is skipped rather than shifted to another wall hour',async()=>{
 const next=await Cron.next('30 2 * * *','America/New_York',2,new Date('2027-03-13T08:00:00Z'));
 assert.equal(next[0].toISOString(),'2027-03-15T06:30:00.000Z');
});
test('DST repeated wall time chooses first occurrence, not both',async()=>{
 const next=await Cron.next('30 1 * * *','America/New_York',2,new Date('2026-11-01T00:00:00Z'));
 assert.equal(next[0].toISOString(),'2026-11-01T05:30:00.000Z');
 assert.equal(next[1].toISOString(),'2026-11-02T06:30:00.000Z');
});
test('month day is never moved to the end of a shorter month',async()=>{
 const next=await Cron.next('0 9 31 * *','UTC',2,new Date('2026-04-01T00:00:00Z'));
 assert.equal(next[0].toISOString(),'2026-05-31T09:00:00.000Z');
});
test('impossible calendar combination produces an empty preview',async()=>{
 assert.deepEqual(await Cron.next('0 9 31 2 *','UTC',1,new Date('2026-01-01T00:00:00Z')),[]);
});
test('leap-day preview can find ten future runs without minute-by-minute scan',async()=>{
 const next=await Cron.next('0 9 29 2 *','UTC',10,new Date('2026-01-01T00:00:00Z'));
 assert.equal(next.length,10);assert.equal(next[0].getUTCFullYear(),2028);assert.equal(next.at(-1).getUTCFullYear(),2064);
});
