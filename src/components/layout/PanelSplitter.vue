<script setup lang="ts">
import { onBeforeUnmount } from "vue";
import { LIMITS, type Panel } from "@/types/workbenchLayout";
const props=defineProps<{panel:Panel; value:number; label:string}>();
const emit=defineEmits<{resize:[value:number]; finish:[]}>();
let drag: {id:number; start:number; size:number; element:HTMLElement} | null=null;
const vertical=()=>props.panel==="bottom";
const position=(e:PointerEvent)=>vertical()?e.clientY:e.clientX;
const sign=()=>props.panel==="left"?1:-1;
function down(e:PointerEvent) {
  if(e.button!==0 || drag) return;
  const element=e.currentTarget as HTMLElement;
  element.focus(); element.setPointerCapture(e.pointerId);
  drag={id:e.pointerId,start:position(e),size:props.value,element};
  e.preventDefault();
}
function move(e:PointerEvent) {
  if(drag?.id===e.pointerId) emit("resize",drag.size+sign()*(position(e)-drag.start));
}
function finish(e?:PointerEvent) {
  if(!drag || (e && e.pointerId!==drag.id)) return;
  const d=drag; drag=null;
  if(d.element.hasPointerCapture(d.id)) d.element.releasePointerCapture(d.id);
  emit("finish");
}
function key(e:KeyboardEvent) {
  const delta=vertical()?(e.key==="ArrowDown"?1:e.key==="ArrowUp"?-1:0):(e.key==="ArrowRight"?1:e.key==="ArrowLeft"?-1:0);
  if(delta || e.key==="Home" || e.key==="End") {
    e.preventDefault();
    emit("resize",e.key==="Home"?LIMITS[props.panel][0]:e.key==="End"?LIMITS[props.panel][1]:props.value+delta*sign()*10);
    emit("finish");
  }
}
onBeforeUnmount(()=>finish());
</script>
<template>
  <div role="separator" tabindex="0" class="wb-splitter" :class="['split-'+panel,{horizontal:panel==='bottom'}]"
    :aria-label="label" :aria-orientation="panel==='bottom'?'horizontal':'vertical'"
    :aria-valuemin="LIMITS[panel][0]" :aria-valuemax="LIMITS[panel][1]" :aria-valuenow="value"
    @pointerdown="down" @pointermove="move" @pointerup="finish" @pointercancel="finish" @lostpointercapture="finish" @keydown="key" />
</template>
