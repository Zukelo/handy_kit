const {test}=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs');
const path=require('node:path');
const source=fs.readFileSync(path.join(__dirname,'../development/design-system/appearance.js'),'utf8');
function setup({saved=null,dark=false,denied=false,native=false}={}) {
 const controls=[{value:''}],root={dataset:{}},events=[],listeners={},globalListeners={};
 let systemChange,stored=saved;
 const media={matches:dark,addEventListener:(_,callback)=>{systemChange=callback;}};
 const nativeCalls=[];
 const context=vm.createContext({
  localStorage:{getItem(){if(denied)throw Error('denied');return stored;},setItem(_,value){if(denied)throw Error('denied');stored=value;}},
  document:{documentElement:root,querySelectorAll:()=>controls,addEventListener:(name,handler)=>{listeners[name]=handler;},dispatchEvent:event=>events.push(event)},
  matchMedia:()=>media,addEventListener:(name,handler)=>{globalListeners[name]=handler;},
  CustomEvent:class{constructor(type,options){this.type=type;this.detail=options.detail;}},
  ...(native?{__TAURI__:{core:{invoke:(command,args)=>{nativeCalls.push({command,...args});return Promise.resolve();}}}}:{}),
 });
 vm.runInContext(source,context);
 return {root,controls,events,nativeCalls,api:context.HandyAppearance,stored:()=>stored,system(value){media.matches=value;systemChange();},change(value){listeners.change({target:{value,matches:()=>true}});},external(value){globalListeners.storage({key:'handykit.appearance.v1',newValue:value});}};
}
test('first launch follows system and responds to system appearance changes',()=>{
 const theme=setup({dark:true});assert.equal(theme.root.dataset.theme,'dark');assert.equal(theme.api.getPreference(),'system');
 theme.system(false);assert.equal(theme.root.dataset.theme,'light');
});
test('explicit selection persists and is not overridden by a system change',()=>{
 const theme=setup();theme.change('dark');assert.equal(theme.stored(),'dark');assert.equal(theme.controls[0].value,'dark');
 theme.system(false);assert.equal(theme.root.dataset.theme,'dark');
 theme.change('system');assert.equal(theme.root.dataset.theme,'light');
});
test('saved preferences restore before content; invalid preferences fall back safely',()=>{
 assert.equal(setup({saved:'dark'}).root.dataset.theme,'dark');assert.equal(setup({saved:'invalid',dark:true}).api.getPreference(),'system');
});
test('denied storage still allows in-session appearance changes',()=>{
 const theme=setup({denied:true});theme.change('dark');assert.equal(theme.root.dataset.theme,'dark');
});
test('theme changes emit semantic events and the prototype native bridge uses valid modes',()=>{
 const theme=setup({native:true});theme.change('dark');assert.equal(theme.events.at(-1).detail.resolved,'dark');
 assert.deepEqual(JSON.parse(JSON.stringify(theme.nativeCalls.at(-1))),{command:'set_appearance',mode:'dark'});
 theme.api.setPreference('untrusted');assert.equal(theme.api.getPreference(),'dark');
});
test('other open pages receive stored appearance changes',()=>{
 const theme=setup();theme.external('dark');assert.equal(theme.root.dataset.theme,'dark');theme.external(null);assert.equal(theme.api.getPreference(),'system');
});
