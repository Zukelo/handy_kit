/* Shared appearance controller. Loaded before content to avoid a theme flash. */
(function(scope) {
  'use strict';
  const KEY='handykit.appearance.v1', valid=value=>['system','light','dark'].includes(value);
  let preference='system';
  try {const stored=scope.localStorage.getItem(KEY);if(valid(stored))preference=stored;} catch { /* Session preference still works. */ }
  const media=scope.matchMedia?scope.matchMedia('(prefers-color-scheme: dark)'):null;
  function syncControls(){scope.document.querySelectorAll('[data-appearance]').forEach(control=>{control.value=preference;});}
  function apply(){
    const resolved=preference==='system'?(media?.matches?'dark':'light'):preference;
    scope.document.documentElement.dataset.appearance=preference;
    scope.document.documentElement.dataset.theme=resolved;
    syncControls();
    scope.document.dispatchEvent(new CustomEvent('handykit:appearance',{detail:{preference,resolved}}));
    // The standalone prototype uses Tauri's documented optional global API.
    scope.__TAURI__?.core?.invoke('set_appearance',{mode:preference}).catch(()=>{});
  }
  function setPreference(value){if(!valid(value))return;preference=value;try{scope.localStorage.setItem(KEY,value);}catch{}apply();}
  scope.HandyAppearance={getPreference:()=>preference,setPreference,syncControls};
  apply();
  scope.document.addEventListener('DOMContentLoaded',syncControls);
  scope.document.addEventListener('change',event=>{if(event.target.matches?.('[data-appearance]'))setPreference(event.target.value);});
  const onSystemChange=()=>{if(preference==='system')apply();};
  if(media?.addEventListener)media.addEventListener('change',onSystemChange);
  else media?.addListener?.(onSystemChange);
  scope.addEventListener?.('storage',event=>{if(event.key===KEY){preference=valid(event.newValue)?event.newValue:'system';apply();}});
})(globalThis);
