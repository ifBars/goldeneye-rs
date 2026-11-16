// Inlined by report rendering: no network, imports or external runtime files.
const playerPreloadDashboard = (() => {
  const stages = ['entry','raw_flags','entry_keys','registry_count','row','weapon','call','awaiting_temporary','final_flag','returned'];
  const callStages = ['remap','raw_flag','raw_key','raw_indices','entry_scan','raw_occupied','raw_weapon','raw_mapping','entry_key','invalidate','reference','awaiting_temporary','returned'];
  const temporaryStages = ['awaiting_skeleton','binding_skeleton','awaiting_factory','initializing_factory','awaiting_initializer','awaiting_insertion','awaiting_forward_tick','forward_tick','awaiting_evaluation','bind_returned','failed','evaluating_bind','aborted'];
  const scriptStages = ['Constructed','AwaitingScript','ResolvingScript','AwaitingScriptBody','AwaitingInitializer','Initializing','AwaitingClipFile','AwaitingClipRange','AwaitingFrameCount','AwaitingAnimation','PreparingPosition','AwaitingAnimationOwner','AwaitingClipCache','AwaitingAnimationBody','AwaitingTimingCurve','AwaitingRootMotion','ReturnedWithoutPlay','ReturnedInitialized','ReturnedWithoutBody','Failed'];
  const dependency = p => p?.post_bind ? (postDependency(p.post_bind)??'preload_temporary_post_bind') : p?.stage === 'awaiting_temporary' ? (p.temporary?.bind_return===1?'preload_temporary_post_bind':'preload_temporary_bind') : p?.stage === 'returned' ? 'player_initializer_post_preload' : stages.includes(p?.stage) ? 'player_animation_preload' : null;
  const root = p => p?.stage === 'awaiting_temporary' ? 'player_preload_temporary_pending' : 'player_animation_preload_pending';
  const equivalent = (a,b) => {
    if(a===b) return true;
    if(a===null||b===null||typeof a!=='object'||typeof b!=='object'||Array.isArray(a)!==Array.isArray(b)) return false;
    const keys=Object.keys(a);
    return keys.length===Object.keys(b).length&&keys.every(k=>Object.hasOwn(b,k)&&equivalent(a[k],b[k]));
  };
  function call(v, ordinal, known, h) {
    const {closed,count,hash}=h,word=v=>count(v)&&v<=0xffffffff,index=(v,n)=>v===null||count(v)&&v<n;
    if(!closed(v,['stage','ordinal','source_ordinal','weapon','generic','alternate','variant','mapping','raw_flags_read','raw_keys_read','raw_index','raw_indices_written','entry_keys_read','entry_index','occupied_written','weapon_written','mapping_written','selected_key_empty','invalidated','extraction_returned','reference_outcome','reference','extraction','return_word','return_reason','state_sha256'])) return false;
    const s=callStages.indexOf(v.stage);
    if(s<0||v.ordinal!==ordinal||!word(v.weapon)||v.weapon===0||typeof v.alternate!=='boolean'||!count(v.variant)||v.variant>=3||v.generic!==(v.alternate?0x090007a7:0x09000765)||!count(v.raw_flags_read)||v.raw_flags_read>32||!count(v.raw_keys_read)||v.raw_keys_read>v.raw_flags_read||!count(v.entry_keys_read)||v.entry_keys_read>35||!index(v.raw_index,32)||!index(v.entry_index,34)||v.selected_key_empty!==null&&typeof v.selected_key_empty!=='boolean'||!hash(v.state_sha256)) return false;
    if(!['raw_indices_written','occupied_written','weapon_written','mapping_written','invalidated','extraction_returned'].every(k=>typeof v[k]==='boolean')) return false;
    for(const [field,reached] of [['occupied_written',6],['weapon_written',7],['mapping_written',8]]) if(s<reached?v[field]:s<12&&!v[field]) return false;
    if(s<5&&v.entry_index!==null||s<4&&v.raw_indices_written||s<3&&v.raw_index!==null||s>=5&&s<12&&v.entry_index===null||s>=9&&s<12&&v.selected_key_empty!==true||s===11&&(!v.extraction_returned||!v.invalidated)||s>=10&&s<12&&!v.invalidated) return false;
    if(v.mapping===null) {if(s!==0) return false;}
    else if(!closed(v.mapping,['output','mapped','steps','map_rows_read','state_sha256'])||s===0||!word(v.mapping.output)||typeof v.mapping.mapped!=='boolean'||!count(v.mapping.steps)||v.mapping.steps>128||!count(v.mapping.map_rows_read)||v.mapping.map_rows_read>4096||!hash(v.mapping.state_sha256)) return false;
    if(v.weapon_written&&!v.occupied_written||v.mapping_written&&!v.weapon_written||v.invalidated&&(!v.mapping_written||v.selected_key_empty!==true)) return false;
    if(v.raw_indices_written||v.occupied_written||v.weapon_written||v.mapping_written) {
      if(v.raw_index===null||v.raw_flags_read!==v.raw_index+1||v.raw_keys_read!==v.raw_index+(v.raw_indices_written?0:1)) return false;
      const base=16*v.raw_index;
      if(v.raw_indices_written) for(let i=5;i<13;i++) known.add(base+i);
      if(v.occupied_written) known.add(base+4);
      if(v.weapon_written) for(let i=0;i<4;i++) known.add(base+i);
      if(v.mapping_written) {if(v.entry_index===null) return false;known.add(base+(v.alternate?9:5)+v.variant);}
    }
    if(s<9&&v.selected_key_empty!==null||s<10&&v.invalidated||s<11&&v.extraction_returned) return false;
    if(v.extraction_returned) {
      if(s!==11||!v.invalidated) return false;
      if(v.reference_outcome==='no_reference') {if(v.reference!==null||v.extraction!==null) return false;}
      else if(v.reference_outcome==='class_four') {
        const e=v.extraction;
        if(!word(v.reference)||((v.reference>>>24)&127)!==4||!closed(e,['requested_index','baseline_index','transition_ordinal','command_ordinal','set_index','group_ordinal','child_ordinal','transitions_read','commands_read','groups_read','children_read'])) return false;
        if(!['requested_index','baseline_index','set_index'].every(k=>count(e[k])&&e[k]<32768)) return false;
        for(const [ordinal,reads] of [['transition_ordinal','transitions_read'],['command_ordinal','commands_read'],['group_ordinal','groups_read'],['child_ordinal','children_read']]) if(!count(e[ordinal])||!count(e[reads])||e[ordinal]>=e[reads]||e[reads]>65536) return false;
      } else return false;
    } else if(v.reference_outcome!==null||v.reference!==null||v.extraction!==null) return false;
    if(s===12) {
      if(v.reference!==null||v.extraction_returned) return false;
      switch(v.return_reason) {
        case 'variant_unavailable':if(v.return_word!==0||v.variant===0||v.mapping?.mapped!==false||v.raw_flags_read!==0||v.entry_keys_read!==0||v.raw_index!==null) return false;break;
        case 'raw_full':if(v.return_word!==0||v.raw_flags_read!==32||v.raw_keys_read!==32||v.raw_index!==null||v.entry_keys_read!==0) return false;break;
        case 'entries_full':if(v.return_word!==0||v.entry_keys_read!==34||v.entry_index!==null||v.occupied_written) return false;break;
        case 'existing_entry':if(v.return_word!==1||!v.mapping_written||v.selected_key_empty!==false||v.invalidated) return false;break;
        default:return false;
      }
    } else if(v.return_word!==null||v.return_reason!==null) return false;
    return !(s>=8&&s<12&&!v.mapping_written);
  }
  function publication(v,t,h) {
    if(v===null) return true;
    const {closed,count}=h,word=v=>count(v)&&v<=0xffffffff;
    if(!closed(v,['file','generation','descriptor_index','section_index','section_key','callback_mask','fresh_base','request_count','active_requests','retained_sections','history_unknown','raw_job','body_state','native_history_verified'])) return false;
    if(t===null||t.script===null||!word(v.file)||!word(v.generation)||v.file!==t.component_file||v.descriptor_index!==t.script.descriptor_index||!count(v.descriptor_index)||v.descriptor_index>=32767||!count(v.section_index)||v.section_index>=32767||!word(v.section_key)||v.callback_mask!==null&&!word(v.callback_mask)||v.raw_job!==null&&!word(v.raw_job)||typeof v.fresh_base!=='boolean'||v.fresh_base!==(v.callback_mask!==null)||typeof v.history_unknown!=='boolean'||v.native_history_verified!==false||!['request_count','active_requests','retained_sections'].every(k=>count(v[k]))) return false;
    if(v.body_state==='known_null') return v.fresh_base&&v.callback_mask!==null&&(v.callback_mask&8)!==0&&v.active_requests===0&&v.retained_sections===0&&!v.history_unknown&&v.raw_job===0&&(v.section_key>>>24)===8&&v.section_key!==0x08000000&&v.section_index>0;
    return v.body_state==='pending';
  }
  function evaluation(t,h,caller=false,trace=t.evaluation) {
    const v=trace,{closed,count,hash}=h,word=n=>count(n)&&n<=0xffffffff;
    if(v===null) return !['evaluating_bind','aborted'].includes(t.stage);
    if(!closed(v,['stage','pending_boundary','reserved','released','lease_live','aborted','arena_owner','ordinal','allocation_ordinal','reservation_start','cursor_after','live_after','slots','prepares','guards','node_visits','nonempty_layers','successful_samples','advanced_context','commit_register','blended_layers','committed','state_sha256'])) return false;
    if(!t.skeleton_bound||!t.head_inserted&&!caller||!t.factory_returned||!t.maintenance_skipped||t.reverse_skipped!==t.head_inserted||t.refresh_returned===t.head_inserted||!['running','pending','returned','failed','aborted'].includes(v.stage)||!hash(v.state_sha256)||!['reserved','released','lease_live','aborted','advanced_context','committed'].every(k=>typeof v[k]==='boolean')||!['node_visits','nonempty_layers','successful_samples','blended_layers'].every(k=>count(v[k]))||!word(v.commit_register)||v.successful_samples!==0||v.blended_layers!==0||v.committed) return false;
    if(!v.reserved||!['arena_owner','ordinal','allocation_ordinal'].every(k=>count(v[k])&&v[k]>0)||!count(v.reservation_start)||v.reservation_start%16!==0||!Array.isArray(v.slots)||v.slots.length>1||!Array.isArray(v.prepares)||v.prepares.length>1||!Array.isArray(v.guards)||v.guards.length>4096) return false;
    let visits=0;
    for(const s of v.slots) {
      if(!closed(s,['slot','weight_bits','branch','tail','prepared','masks_cleared','nodes','successes','flags_after'])||s.slot!==0||s.weight_bits!==null&&!word(s.weight_bits)||s.branch!==null&&!['nonempty','empty_tail','low_weight'].includes(s.branch)||typeof s.prepared!=='boolean'||typeof s.masks_cleared!=='boolean'||s.successes!==0||s.flags_after!==null&&!word(s.flags_after)||!Array.isArray(s.nodes)||s.masks_cleared&&!s.prepared) return false;
      if(s.nodes.length&&(!s.prepared||!s.masks_cleared||s.branch!=='nonempty')) return false;
      for(const n of s.nodes) if(!closed(n,['node','result'])||!count(n.node)||n.node===0||n.result!==0||n.node!==s.tail) return false;
      visits+=s.nodes.length;
    }
    if(visits!==v.node_visits) return false;
    for(const p of v.prepares) {
      if(!closed(p,['file','reference','generation','descriptor_bound','body_resident','auxiliary_words','lookups','body_reads','returned','pause'])||p.file!==t.skin_file||p.reference!==t.skin_reference||typeof p.descriptor_bound!=='boolean'||typeof p.body_resident!=='boolean'||!count(p.lookups)||p.lookups===0||!count(p.body_reads)) return false;
      if(p.returned==='AuxiliaryZero' ? !p.descriptor_bound||!p.body_resident||p.auxiliary_words!==0||p.pause!==null||!word(p.generation)||p.body_reads===0 : p.returned!==null&&!['DescriptorAbsent','BodyAbsent'].includes(p.returned)) return false;
    }
    for(const g of v.guards) {
      if(!closed(g,['node','pointer','record','clip_reference','animator_reference_read','animator_reference','result','pending_boundary'])||!count(g.node)||g.node===0||!['constructor_inline','record','null','unknown'].includes(g.pointer)||g.result!==null&&g.result!==0||typeof g.animator_reference_read!=='boolean'||g.clip_reference!==null&&!word(g.clip_reference)||g.animator_reference!==null&&!word(g.animator_reference)) return false;
      if(g.result===0&&(g.pending_boundary!==null||!v.slots.some(s=>s.nodes.some(n=>n.node===g.node))||g.clip_reference!==0xffffffff&&!(g.animator_reference_read&&g.animator_reference===0xffffffff))) return false;
      if(g.clip_reference===0xffffffff&&(g.animator_reference_read||g.animator_reference!==null)) return false;
    }
    if(caller&&!t.head_inserted) {
      if(v.prepares.length||v.guards.length||v.node_visits!==0||v.nonempty_layers!==0||v.advanced_context) return false;
      if(v.stage==='returned') return v.pending_boundary===null&&v.slots.length===1&&!v.slots[0].prepared&&!v.slots[0].masks_cleared&&v.slots[0].tail===null&&v.slots[0].nodes.length===0&&v.slots[0].branch==='empty_tail'&&v.slots[0].flags_after===0&&v.released&&!v.lease_live&&!v.aborted&&v.cursor_after===v.reservation_start&&v.live_after===0&&v.commit_register===0;
    }
    if(v.stage==='returned') return v.pending_boundary===null&&v.released&&!v.lease_live&&!v.aborted&&v.nonempty_layers===1&&v.node_visits===1&&v.advanced_context&&v.commit_register===0&&v.cursor_after===v.reservation_start&&v.live_after===0&&v.prepares.length===1&&v.prepares[0].returned!==null&&v.slots.length===1&&v.slots[0].flags_after===0&&v.guards.filter(g=>g.result===0).length===visits;
    return !v.released&&(caller||t.bind_return===null)&&(v.stage==='aborted'?v.aborted&&!v.lease_live:!v.aborted&&v.lease_live);
  }
  function postDependency(p) {
    if(p.stage==='awaiting_head') return 'preload_post_bind_head';
    if(p.stage==='awaiting_parameter') return 'preload_post_bind_parameter_pointer';
    if(p.stage==='awaiting_bone_outputs') return 'preload_temporary_bone_outputs';
    if(p.stage!=='evaluating') return null;
    const boundary=p.evaluation?.pending_boundary;
    return boundary==='Prepare'?(p.evaluation.prepares.at(-1)?.pause==='Auxiliary'?'preload_post_bind_evaluation_auxiliary_clear':'preload_post_bind_evaluation_skin'):boundary==='Node'?'preload_post_bind_evaluation_sampling':boundary?'preload_post_bind_evaluation_layers':null;
  }
  function postBind(p,t,h) {
    if(p===null) return true;
    const {closed,count,hash}=h,word=n=>count(n)&&n<=0xffffffff;
    if(!closed(p,['stage','pending_dependency','temporary_identity','head_reads','first_scope','head','parameter','process','capture','phase','evaluation','state_sha256'])||t?.bind_return!==1||p.temporary_identity!==t.identity||!hash(p.state_sha256)||!['awaiting_head','awaiting_parameter','reading_scalar','assigning_phase','evaluating','awaiting_bone_outputs','failed','aborted'].includes(p.stage)||p.pending_dependency!==postDependency(p)) return false;
    const process=p.process;
    if(!closed(process,['owner','initial_bits','current_bits','revision','reads','capture_issued'])||!count(process.owner)||process.owner===0||process.initial_bits!==0x3f800000||process.current_bits!==null||!count(process.revision)||!count(process.reads)) return false;
    if(p.head!==null&&(!closed(p.head,['kind','node'])||!count(p.head_reads)||!['class_four','nonmatching','null','unknown'].includes(p.head.kind))) return false;
    if(p.head!==null) {
      const kind=p.head.kind;
      if(kind==='unknown'&&(p.head.node!==null||p.stage!=='awaiting_head'||p.parameter!==null||p.capture!==null||p.phase!==null||p.evaluation!==null)) return false;
      if(kind==='null'&&(p.head.node!==null||t.head_inserted||p.parameter!==null||p.capture!==null||p.phase!==null)) return false;
      if(['class_four','nonmatching'].includes(kind)&&(!count(p.head.node)||p.head.node===0)) return false;
    }
    const v=p.parameter;
    if(v!==null&&(!closed(v,['node','pointer','offset','previous','bits'])||p.head?.kind!=='class_four'||v.node!==p.head.node||v.pointer!=='constructor_inline_phase'||v.offset!==0x64||v.bits!==0x3f000000||v.previous!==null&&!word(v.previous))) return false;
    const r=p.capture;
    if(r!==null) {
      if(!closed(r,['owner','read_sequence','revision','current_bits'])||v===null||r.owner!==process.owner||!count(r.read_sequence)||r.read_sequence===0||r.read_sequence>process.reads||!count(r.revision)||r.revision>process.revision||r.current_bits!==null||process.capture_issued!==true) return false;
    } else if(process.capture_issued!==null) return false;
    if(p.phase!==null) {
      const phase=p.phase;
      if(!closed(phase,['upper_bits','selected_bits','assignment','header_known_bits','header_expression_bound'])||r===null||phase.upper_bits!==0||phase.selected_bits!==null||phase.assignment!=='deferred'||phase.header_known_bits!==null||phase.header_expression_bound!==true) return false;
    }
    if(p.evaluation!==null) {
      if(p.head===null||p.head.kind==='class_four'&&p.phase===null||!evaluation(t,h,true,p.evaluation)) return false;
      const first=p.first_scope,e=p.evaluation;
      if(!closed(first,['arena_owner','ordinal','allocation_ordinal'])||!['arena_owner','ordinal','allocation_ordinal'].every(k=>count(first[k])&&first[k]>0)||first.arena_owner!==e.arena_owner||e.ordinal<=first.ordinal||e.allocation_ordinal<=first.allocation_ordinal) return false;
      if(t.evaluation!==null ? !['arena_owner','ordinal','allocation_ordinal'].every(k=>first[k]===t.evaluation[k])||t.evaluation.stage!=='returned'||!t.evaluation.released : !t.refresh_returned||t.head_inserted) return false;
    }
    return p.stage!=='awaiting_bone_outputs'||p.evaluation?.stage==='returned'&&p.evaluation.released;
  }
  function temporary(t,p,c,h) {
    const {closed,count,hash,phaseZeroValid}=h,word=v=>count(v)&&v<=0xffffffff,index=(v,n)=>v===null||count(v)&&v<n;
    if(!closed(t,['stage','pending_dependency','identity','source_receiver','player','component_file','reference','skin_file','skin_reference','attached','callbacks_bound','skeleton_bound','bones','auxiliary_words','skeleton_backlink_bound','pose_callback_assigned','frame_stamp_known','semantic_flags','component_flags','active_slots','factory_returned','head_inserted','script','forward','maintenance_skipped','reverse_skipped','bind_return','refresh_returned','evaluation','state_sha256'])) return false;
    const s=temporaryStages.indexOf(t.stage);
    if(s<0||!count(t.identity)||!count(t.source_receiver)||t.identity<=t.source_receiver||t.source_receiver===0||!count(t.player)||t.player===0||t.component_file!==0x01020016||!word(t.reference)||t.reference!==(p.current_call.reference??0)||!word(t.skin_file)||!word(t.skin_reference)||t.attached!==false||t.callbacks_bound!==false||t.pose_callback_assigned!==false||!equivalent(t.semantic_flags,[0,11])||!count(t.component_flags)||t.component_flags>255||t.active_slots!==1||!hash(t.state_sha256)||c.attachment!==null&&(!count(c.attachment?.registry_identities)||t.identity>c.attachment.registry_identities)) return false;
    if(!['skeleton_bound','skeleton_backlink_bound','frame_stamp_known','factory_returned','head_inserted','maintenance_skipped','reverse_skipped','refresh_returned'].every(k=>typeof t[k]==='boolean')||t.skeleton_backlink_bound!==t.skeleton_bound) return false;
    if(t.skeleton_bound ? !count(t.bones)||t.bones>128||!count(t.auxiliary_words)||t.auxiliary_words>4096 : t.bones!==null||t.auxiliary_words!==null||t.frame_stamp_known) return false;
    if(s>=2&&s<10&&!t.skeleton_bound) return false;
    const forwardPending=s===7&&t.forward?.stage==='AwaitingBody';
    const expected=s===0&&(t.skin_reference>>>24)!==13?'temporary_skin_remap':s<=1?'temporary_skeleton':s===2&&((t.reference>>>24)&127)===3?'temporary_factory_class':s===4&&t.script?.stage==='AwaitingScriptBody'||forwardPending?'temporary_script_body_residency':s===3||s===4?'temporary_script_initializer':s===6||s===7?'temporary_forward_tick':s===8?'temporary_animation_evaluation':s===11?(t.evaluation?.pending_boundary==='Prepare'?(t.evaluation.prepares.at(-1)?.pause==='Auxiliary'?'temporary_evaluation_auxiliary_clear':'temporary_evaluation_skin'):t.evaluation?.pending_boundary==='Node'?'temporary_evaluation_sampling':t.evaluation?.pending_boundary?'temporary_evaluation_layers':null):null;
    if(t.pending_dependency!==expected||s<6&&(t.factory_returned||t.head_inserted)||s>=6&&s<10&&!t.factory_returned||t.head_inserted&&(t.script===null||!t.factory_returned)) return false;
    if(s===8 ? !t.maintenance_skipped||t.reverse_skipped!==t.head_inserted||t.bind_return!==null||t.refresh_returned : s===9 ? t.bind_return!==1||(t.head_inserted?t.refresh_returned||t.evaluation?.stage!=='returned':!t.refresh_returned||t.evaluation!==null) : t.bind_return!==null||t.refresh_returned) return false;
    if(!evaluation(t,h)) return false;
    const a=t.script;
    if(a!==null) {
      if(!closed(a,['stage','slot','factory_flags','reference','descriptor_index','body_bound','initializer_returned','phase_zero','clips','state_sha256'])||a.reference!==t.reference||a.slot!==0||a.factory_flags!==3||!index(a.descriptor_index,32768)||a.body_bound!==null&&typeof a.body_bound!=='boolean'||typeof a.initializer_returned!=='boolean'||!hash(a.state_sha256)||!scriptStages.includes(a.stage)) return false;
      if(a.initializer_returned!==['ReturnedWithoutBody','ReturnedWithoutPlay','ReturnedInitialized'].includes(a.stage)) return false;
      if(a.phase_zero!==null&&(!phaseZeroValid(a.phase_zero)||a.body_bound!==true&&!(forwardPending&&a.body_bound===null)||a.descriptor_index===null)) return false;
      if(a.stage==='AwaitingScriptBody'&&(a.body_bound!==null||a.phase_zero!==null||a.initializer_returned||a.descriptor_index===null)||a.stage==='ReturnedInitialized'&&a.phase_zero?.stage!=='initialized'||a.stage==='ReturnedWithoutPlay'&&a.phase_zero?.stage!=='returned_without_play'||a.stage==='ReturnedWithoutBody'&&(a.phase_zero!==null||a.body_bound!==false&&!(forwardPending&&a.body_bound===null))) return false;
      if(t.head_inserted&&(!a.initializer_returned||!['ReturnedWithoutPlay','ReturnedInitialized','ReturnedWithoutBody'].includes(a.stage))) return false;
      if(!Array.isArray(a.clips)||a.clips.length>510||a.phase_zero!==null&&a.phase_zero.clip_records!==a.clips.length) return false;
      if(a.stage==='ReturnedWithoutBody'&&a.clips.length!==0) return false;
      for(const clip of a.clips) {
        if(!closed(clip,['file','reference','cache_generation','start_raw','end_raw','blend_bits'])||!['file','reference','cache_generation','blend_bits'].every(k=>clip[k]===null||word(clip[k]))||!['start_raw','end_raw'].every(k=>clip[k]===null||Number.isInteger(clip[k])&&clip[k]>=-32768&&clip[k]<=32767)) return false;
      }
    }
    const f=t.forward;
    if(f!==null) {
      if(!closed(f,['stage','phase_cleared','body_bound','duration_raw','fps_bits','result_bits'])||s<7||!t.head_inserted||typeof f.phase_cleared!=='boolean'||f.body_bound!==null&&typeof f.body_bound!=='boolean'||!['Entering','Resolving','AwaitingBody','Duration','Fps','Returned','Failed'].includes(f.stage)||!['duration_raw','fps_bits','result_bits'].every(k=>f[k]===null||word(f[k]))) return false;
      if(f.stage==='AwaitingBody'&&(!forwardPending||!f.phase_cleared||f.body_bound!==null||[f.duration_raw,f.fps_bits,f.result_bits].some(v=>v!==null)||a?.body_bound!==null||t.maintenance_skipped||t.reverse_skipped)) return false;
      if(f.stage==='Returned'&&typeof f.body_bound!=='boolean'||s===8&&(f.stage!=='Returned'||!f.phase_cleared)||f.stage==='Returned'&&f.body_bound===true&&!['duration_raw','fps_bits','result_bits'].every(k=>word(f[k]))) return false;
      if(f.stage==='Returned'&&f.body_bound===false&&[f.duration_raw,f.fps_bits,f.result_bits].some(v=>v!==null)) return false;
    } else if(s===8&&t.head_inserted) return false;
    return true;
  }
  function validate(p,c,h) {
    try {
      if(p===null) return c.stage!=='player_preload_temporary_pending';
      const {closed,count,hash,cacheStateValid}=h,word=v=>count(v)&&v<=0xffffffff;
      if(!closed(p,['stage','pending_dependency','entry_flag','cache_enabled','raw_flags_written','entry_keys_written','registry_count','source_row','rows_visited','enabled_rows','current_weapon','call_in_row','completed_calls','current_call','temporary','post_bind','script_publication','allocations','caches','flag_cleared','helper_returned','state_sha256'])) return false;
      const s=stages.indexOf(p.stage),prefix=c.initializer_prefix;
      if(s<0||prefix?.stage!=='awaiting_preload'||prefix.cache_enabled!==1||prefix.invalidated!==false||!equivalent(prefix.receivers_found,[true,true])||!hash(p.state_sha256)||!count(p.cache_enabled)||p.cache_enabled>255||typeof p.flag_cleared!=='boolean'||p.helper_returned!==(s===9)||!count(p.raw_flags_written)||p.raw_flags_written>32||!count(p.entry_keys_written)||p.entry_keys_written>34||!count(p.call_in_row)||p.call_in_row>=6||p.pending_dependency!==dependency(p)||c.pending_dependency!==dependency(p)||c.stage!==root(p)) return false;
      if(s===0?p.entry_flag!==null:!count(p.entry_flag)||p.entry_flag>255) return false;
      if(s>=2&&!(s===9&&p.entry_flag===0)&&p.raw_flags_written!==32||((s>=3&&s<9)||p.flag_cleared)&&p.entry_keys_written!==34) return false;
      if(!['rows_visited','enabled_rows','source_row'].every(k=>count(p[k]))) return false;
      if(p.registry_count===null ? !(s<=3||s===9&&p.entry_flag===0)||p.rows_visited!==0||p.enabled_rows!==0||p.source_row!==0 : !count(p.registry_count)||p.registry_count>1024||p.rows_visited>p.registry_count||p.enabled_rows>p.rows_visited||p.source_row>p.rows_visited) return false;
      const done=p.completed_calls;
      if(!Array.isArray(done)||done.length>6144||p.current_weapon!==null&&!word(p.current_weapon)) return false;
      const known=new Set();for(let i=0;i<p.raw_flags_written;i++) known.add(16*i+4);
      const calls=p.current_call===null?[...done]:[...done,p.current_call];let last=null;
      for(const [i,v] of calls.entries()) {
        if(!call(v,i,known,h)||!count(v.source_ordinal)||!count(p.registry_count)||v.source_ordinal>=p.registry_count||last!==null&&v.source_ordinal<last||last!==null&&v.source_ordinal!==last&&i%6!==0) return false;
        const position=last===v.source_ordinal?i%6:0;
        if(v.alternate!==(position%2!==0)||v.variant!==Math.floor(position/2)||i<done.length&&v.stage!=='returned') return false;
        last=v.source_ordinal;
      }
      const current=calls[done.length];
      if(current&&(current.source_ordinal!==p.source_row||current.weapon!==p.current_weapon||p.call_in_row!==done.length%6)) return false;
      if(s===7&&(p.current_call?.stage!=='awaiting_temporary'||p.flag_cleared||p.cache_enabled!==p.entry_flag)) return false;
      if(p.flag_cleared ? s!==9||p.source_row!==p.registry_count||done.length!==6*p.enabled_rows||p.current_call!==null||p.cache_enabled!==0 : p.entry_flag!==null&&p.cache_enabled!==p.entry_flag) return false;
      if(!Array.isArray(p.allocations)||p.allocations.length!==3) return false;
      const allocationShapes=[['raw',512,512,[null,null,null,null]],['array34',7088,7072,[208,34,null,null]],['array64',56080,56064,[876,64,null,null]]];
      for(const [i,[kind,bytes,payload,header]] of allocationShapes.entries()) {
        const a=p.allocations[i];
        if(a===null) {if(s>2&&p.entry_flag!==0) return false;continue;}
        if(!closed(a,['kind','native_bytes','payload_bytes','known_payload_bytes','header_words'])||a.kind!==kind||a.native_bytes!==bytes||a.payload_bytes!==payload||!equivalent(a.header_words,header)||!count(a.known_payload_bytes)) return false;
        if(i===0?a.known_payload_bytes<known.size||a.known_payload_bytes>416:i===1?a.known_payload_bytes<4*p.entry_keys_written||a.known_payload_bytes>136:a.known_payload_bytes!==0) return false;
      }
      if(!cacheStateValid(p.caches,prefix.receivers_found)) return false;
      const currentCache=JSON.parse(JSON.stringify(p.caches)),historical=prefix.caches,invalidations=calls.filter(v=>v.invalidated).length;
      for(let i=0;i<2;i++) for(const key of ['invalidation_count_reads','invalidation_pointer_reads']) {
        const old=historical.headers[i][key],selected=historical.selected===['first_person','third_person'][i];
        if(currentCache.headers[i][key]!==old+(selected?invalidations:0)) return false;
        currentCache.headers[i][key]=old;
      }
      currentCache.state_sha256=historical.state_sha256;
      if(!equivalent(currentCache,historical)) return false;
      return (p.temporary===null||s===7&&temporary(p.temporary,p,c,h))&&publication(p.script_publication,p.temporary,h)&&postBind(p.post_bind,p.temporary,h);
    } catch { return false; }
  }
  function summary(p) {
    if(p===null) return '';
    const current=p.current_call,t=p.temporary,selected=p.caches.selected==='first_person'?0:p.caches.selected==='third_person'?1:null;
    const history=selected===null?'No cache is selected.':`Current selected-cache validity reads: ${p.caches.headers[selected].invalidation_count_reads}; the initializer cache receipt remains historical.`;
    const callText=current?` Current preload call ${current.ordinal+1} is ${current.stage.replaceAll('_',' ')}. ${current.selected_key_empty===true?'The selected animation-entry key is still empty; no key commit has occurred.':''} ${current.extraction_returned?current.reference_outcome==='class_four'?`Class-four reference 0x${current.reference.toString(16).padStart(8,'0')} was extracted.`:'The extractor returned a known no-reference result; temporary binding is still required.':'Reference extraction has not returned.'}`:'';
    const temp=t?` A distinct temporary animator ${t.identity} retains ${t.bones??'unknown'} bones and ${t.auxiliary_words??'unknown'} auxiliary words. ${t.script?.stage==='AwaitingScriptBody'?'Its script descriptor is retained, but body residency is unknown. No head insertion, forward tick or evaluation has run.':`Its reached dependency is ${(t.pending_dependency??t.stage).replaceAll('_',' ')}.`} No player attachment or callback was borrowed.`:' No temporary animator has been allocated yet.';
    const evaluationText=t?.evaluation?.stage==='returned'?` Nonempty animation evaluation returned after releasing its own scratch scope: ${t.evaluation.node_visits} node visited, ${t.evaluation.successful_samples} samples, no pose commit. Inner bind returned.`:t?.evaluation?.stage==='aborted'?' The retained evaluation scope was canceled; cleanup is not a bind return.':'';
    const postText=p.post_bind?.stage==='awaiting_bone_outputs'?`${p.post_bind.parameter!==null?' The caller stored 0.5 through its actual parameter pointee. KnownInitial 1.0 remains separate from UnknownCurrent; header phase retains a deferred expression.':' The known NULL or nonmatching head skipped the parameter store and scalar read.'} A separate second evaluation returned and released with zero samples or commits. Six-bone outputs, teardown and entry commit remain pending.`:'';
    const publicationText=p.script_publication===null?'':p.script_publication.body_state==='known_null'?` Fresh base publication proves a NULL script body for descriptor ${p.script_publication.descriptor_index} in section ${p.script_publication.section_index}; original runtime history remains unverified.`:' Current script publication is pending; historical initializer state remains separate.';
    return ` Preload wrote ${p.raw_flags_written} Raw flags and ${p.entry_keys_written} entry keys; ${p.rows_visited} registry rows were visited and ${p.enabled_rows} ${p.enabled_rows===1?'was':'were'} enabled. Shared payload knowledge: ${p.allocations.map(a=>a?.known_payload_bytes??'absent').join(' / ')} bytes (Raw / array34 / array64); other bytes remain unknown. ${history}${callText}${temp}${evaluationText}${postText}${publicationText} The shared preload flag remains ${p.cache_enabled}; ${p.helper_returned?'only the nested preload helper returned':'preload has not returned'}. No sampled pose, completed player or camera is claimed.`;
  }
  return {validate,summary,dependency,root};
})();
