import { Window } from 'happy-dom';
import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';
import { test } from 'node:test';

const template = readFileSync(new URL('../../xtask/src/dashboard.html', import.meta.url), 'utf8');
const generated = readFileSync(new URL('../../progress/index.html', import.meta.url), 'utf8');
const latest = JSON.parse(readFileSync(new URL('../../progress/latest.json', import.meta.url), 'utf8'));
const kinds = ['checks','coverage','assets','resources','render','viewer','navigation','paths','menu','startup','mission'];

function fixture() {
  const criteria = [
    { id:'render.geometry', domain:'Rendering', title:'Geometry', acceptance:'Preserve shapes', state:'in_progress' },
    { id:'assets.read', domain:'Assets', title:'Read assets', acceptance:'Bounded reads', state:'implemented' },
    { id:'audio.music', domain:'Audio', title:'Music', acceptance:'Match cue timing', state:'not_started' },
  ];
  return {
    generated_unix: 1700000000, source_hash:'a'.repeat(64), scope_note:'Incomplete acceptance inventory', next:'Verify the next dependency',
    parity:{ verified:0,total:3,implemented:1,in_progress:1 }, criteria,
    groups:{ Rendering:{total:1,implemented:0,in_progress:1}, Assets:{total:1,implemented:1,in_progress:0}, Audio:{total:1,implemented:0,in_progress:0} },
    compatibility:[{edition:'Authored fixture',revision:'0',container:'RVZ',assets:'not_run',resources:'not_run',render:'not_run',viewer:'not_run',menu:'not_run',startup:'not_run',runtime:'not_implemented'}],
    history:[], ...Object.fromEntries(kinds.map(k=>[k,{state:'not_run'}])),
  };
}
function embed(report) {
  const safe = JSON.stringify(report).replaceAll('<','\\u003c').replaceAll('>','\\u003e').replaceAll('&','\\u0026');
  return template.replace('"__REPORT_JSON__"',safe);
}

test('mission context reports measured retention without implying playable worlds',async()=>{
  const report=fixture();
  report.mission={state:'passed',details:{probe_count:2,contexts:Array.from({length:2},()=>({
    resources_retained:true,world_registered:false,player_initialized:false,reference_compared:false,
    catalog:{primary_rows_read:9,secondary_rows_read:4,primary_rows_registered:8,secondary_rows_registered:4,duplicate_rows:1},
  }))}};
  await inspect(embed(report),d=>{
    const summary=d.querySelector('#mission-summary').textContent;
    assert.match(summary,/2 local context probes/);
    assert.match(summary,/8 level rows and 4 tuning rows/);
    assert.match(summary,/1 duplicate arrivals rejected/);
    assert.match(summary,/World and player initialization remain incomplete/);
    assert.equal(d.querySelectorAll('#gates .gate').length,kinds.length);
  });
});

test('mission missing stale failed and incomplete receipts cannot imply current retention',async()=>{
  for (const state of ['not_run','stale','failed']) {
    const report=fixture();report.mission={state,details:{probe_count:99,contexts:[]}};
    await inspect(embed(report),d=>{
      assert.match(d.querySelector('#mission-summary').textContent,/No current retained-resource evidence/);
      assert.doesNotMatch(d.querySelector('#mission-summary').textContent,/99 local context probes/);
    });
  }
  for (const context of [{resources_retained:true,world_registered:true},
    {resources_retained:true,world_registered:false,player_initialized:false,reference_compared:false,catalog:{primary_rows_read:0}}]) {
    const report=fixture();report.mission={state:'passed',details:{probe_count:1,contexts:[context]}};
    await inspect(embed(report),d=>assert.match(d.querySelector('#mission-summary').textContent,/incomplete mission-context diagnostics/));
  }
});

test('mission residency and prepared rows stay distinct from world readiness and legacy metadata',async()=>{
  const report=fixture(), context={resources_retained:true,world_registered:false,player_initialized:false,reference_compared:false,
    catalog:{primary_rows_read:3,secondary_rows_read:2,primary_rows_registered:2,secondary_rows_registered:2,duplicate_rows:1},
    residency:{complete:true,retained_files:2,total_payload_bytes:10240,resources:[{},{}]},
    registration:{prepared:true,native_init_timing_verified:false,rows:[{},{}]}};
  report.mission={state:'passed',details:{probe_count:1,contexts:[context]}};
  await inspect(embed(report),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/2 resident files, 10,240 retained payload bytes and 2 prepared registration rows/);
    assert.match(text,/original init timing is unverified/);
    assert.match(text,/World and player initialization remain incomplete/);
  });
  context.registration.native_init_timing_verified=true;
  await inspect(embed(report),d=>assert.match(d.querySelector('#mission-summary').textContent,/not been recorded completely/));
  delete context.residency;
  await inspect(embed(report),d=>assert.doesNotMatch(d.querySelector('#mission-summary').textContent,/2 resident files/));
});

function missionProgressContext() {
  return {
    resources_retained:true,world_registered:false,player_initialized:false,reference_compared:false,
    catalog:{primary_rows_read:3,secondary_rows_read:2,primary_rows_registered:2,secondary_rows_registered:2,duplicate_rows:1},
    residency:{complete:true,retained_files:2,total_payload_bytes:10240,resources:[{},{}]},
    registration:{prepared:true,native_init_timing_verified:false,rows:[{},{}]},
    map_instances:{bound:true,preregistration_slots:3,loaded_instances:2,
      base_animators_initialized:false,trigger_factories_run:false,navigation_initialized:false,
      instances:[
        {loaded_ordinal:0,preregistration_index:1,zones:2,bsp_nodes:1,placements_bound:5,navigation_placements:1},
        {loaded_ordinal:1,preregistration_index:2,zones:3,bsp_nodes:0,placements_bound:7,navigation_placements:0},
      ]},
    spawn:{state:'prepared_startpoint',requested_id_before:1,requested_id_after:1,registration_rows:2,
      eligible_files:1,filtered_files:1,prepared_entries:3,visits:2,selection_found:true,used_fallback:false,
      selected_catalog_index:1,selected:{file_hash:0x018100a5,source_ordinal:0,prepared_row_index:0},
      pose_sha256:'a'.repeat(64),pose_sha256_encoding:'position_u32be[4] || rotation_u32be[4] || rotation_mode_u8',rotation_mode:3,
      catalog_membership_verified:false,catalog_created:false,creation_inputs_verified:false,camera_frame_ready:false},
  };
}

function missionProgressReport(contexts=[missionProgressContext()]) {
  const report=fixture();
  report.mission={state:'passed',details:{probe_count:contexts.length,contexts}};
  return report;
}

test('mission summary displays bound map data and prepared source selection without readiness claims',async()=>{
  await inspect(embed(missionProgressReport()),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/2 map data instances bound, 3 preregistration slots including null slot zero, 5 zones and 12 placements \(1 navigation placements\)/);
    assert.match(text,/requested ID 1 selected, catalog entry 1 of 3 prepared entries; source pose prepared/);
    assert.match(text,/Base animators, trigger factories and navigation initialization remain incomplete/);
    assert.match(text,/Live catalog membership, player creation inputs and camera frame remain unverified/);
    assert.match(text,/World and player initialization remain incomplete/);
    assert.doesNotMatch(text,/100%|player ready|camera ready|world ready/i);
  });
});

test('mission source fallback and missing StartPoint remain distinct from successful player creation',async()=>{
  const context=missionProgressContext();
  Object.assign(context.spawn,{requested_id_before:99,requested_id_after:0,used_fallback:true,visits:5});
  await inspect(embed(missionProgressReport([context])),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/requested ID 99 fell back to ID 0, catalog entry 1 of 3 prepared entries; source pose prepared/);
    assert.match(text,/World and player initialization remain incomplete/);
  });
  Object.assign(context.spawn,{state:'missing_startpoint',requested_id_after:99,selection_found:false,
    used_fallback:false,selected_catalog_index:null,selected:null,pose_sha256:null,rotation_mode:null,visits:6});
  await inspect(embed(missionProgressReport([context])),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/no eligible selection for requested ID 99 or fallback ID 0 among 3 prepared entries/);
    assert.doesNotMatch(text,/source pose prepared|fell back to ID/);
    assert.match(text,/2 map data instances bound/);
  });
});

test('incomplete or contradictory map and spawn evidence cannot promote their diagnostics',async()=>{
  const mapMutations=[
    context=>{delete context.map_instances;},
    context=>{context.map_instances.bound=false;},
    context=>{context.map_instances.instances.pop();},
    context=>{context.map_instances.preregistration_slots=2;},
    context=>{context.map_instances.instances[0].placements_bound=-1;},
    context=>{context.map_instances.trigger_factories_run=true;},
  ];
  for (const mutate of mapMutations) {
    const first=missionProgressContext(), second=missionProgressContext();mutate(second);
    await inspect(embed(missionProgressReport([first,second])),d=>{
      const text=d.querySelector('#mission-summary').textContent;
      assert.match(text,/Map data bindings have not been recorded completely/);
      assert.doesNotMatch(text,/map data instances bound/);
      assert.match(text,/World and player initialization remain incomplete/);
    });
  }
  const spawnMutations=[
    context=>{delete context.spawn;},
    context=>{context.spawn.camera_frame_ready=true;},
    context=>{context.spawn.catalog_created=true;},
    context=>{context.spawn.selected_catalog_index=3;},
    context=>{context.spawn.pose_sha256=null;},
    context=>{context.spawn.used_fallback=true;},
    context=>{delete context.spawn.creation_inputs_verified;},
    context=>{context.spawn.selected=null;},
  ];
  for (const mutate of spawnMutations) {
    const first=missionProgressContext(), second=missionProgressContext();mutate(second);
    await inspect(embed(missionProgressReport([first,second])),d=>{
      const text=d.querySelector('#mission-summary').textContent;
      assert.match(text,/Prepared source spawn selection has not been recorded completely/);
      assert.doesNotMatch(text,/source pose prepared|fell back to ID/);
      assert.match(text,/World and player initialization remain incomplete/);
    });
  }
  const legacy=missionProgressContext();delete legacy.map_instances;delete legacy.spawn;
  await inspect(embed(missionProgressReport([legacy])),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/2 resident files/);
    assert.doesNotMatch(text,/map data instances bound|prepared source StartPoint/);
  });
});

function playerComponentsContext() {
  const context=missionProgressContext();
  context.player_components={
    state:'fp_skeleton_constructor_initialized',requested_character:0x4d0000a5,selected_character:0x4d0000a5,
    catalog_resource:0x010900a5,catalog_payload_bytes:1024,character_sheet:0x140000a5,
    character_group:1,character_row_stride:24,character_rows:5,duplicate_character_rows:1,
    selected_source_index:2,selected_source_address:256,native_default_fallback_used:false,
    component_resource:0x010200a5,component_payload_bytes:4096,component_reference:0,
    skin_resource:0x010a00a5,skin_payload_bytes:8192,skin_reference:0x0d0000a5,
    skin_directory_index:0,skin_hash:0x0d0000a5,skin_body_address:512,
    retained_resources:3,retained_payload_bytes:13312,bone_count:4,root_count:1,
    topology_sha256:'b'.repeat(64),constructor_state_sha256:'c'.repeat(64),
    camera_tag:0x0e000028,camera_tag_found:true,camera_bone:3,camera_parent_chain_length:3,
    camera_parent_chain_sha256:'d'.repeat(64),skeleton_constructor_initialized:true,
    animation_sampled:false,player_physics_initialized:false,camera_frame_ready:false,
  };
  return context;
}

test('mission skeleton constructor summary reports retained dependencies and resolved tag without sampled-pose readiness',async()=>{
  const first=playerComponentsContext(), second=playerComponentsContext();
  second.player_components.bone_count=6;
  await inspect(embed(missionProgressReport([first,second])),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/first-person skeleton constructor: 3 retained dependencies \(character catalog, animation component and skin; 13,312 payload bytes\), 4 bones and 1 root/);
    assert.match(text,/camera tag 0xe000028 resolved to bone 3, 3 bones in its parent chain/);
    assert.match(text,/Constructor identity is not a sampled animation pose; player physics and camera frames remain incomplete/);
    assert.match(text,/World and player initialization remain incomplete/);
    assert.doesNotMatch(text,/6 bones and|100%|player ready|camera ready|world ready/i);
  });
});

test('unresolved camera tags legacy contexts and stale or failed receipts never invent resolved skeleton evidence',async()=>{
  const context=playerComponentsContext();
  Object.assign(context.player_components,{camera_tag_found:false,camera_bone:null,
    camera_parent_chain_length:0,camera_parent_chain_sha256:null});
  await inspect(embed(missionProgressReport([context])),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/4 bones and 1 root; camera tag 0xe000028 unresolved; no parent chain/);
    assert.doesNotMatch(text,/resolved to bone/);
    assert.match(text,/camera frames remain incomplete/);
  });
  await inspect(embed(missionProgressReport()),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/2 map data instances bound/);
    assert.doesNotMatch(text,/first-person skeleton constructor:|camera tag/);
  });
  for (const state of ['not_run','stale','failed']) {
    const report=missionProgressReport([playerComponentsContext()]);report.mission.state=state;
    await inspect(embed(report),d=>{
      const text=d.querySelector('#mission-summary').textContent;
      assert.match(text,/No current retained-resource evidence/);
      assert.doesNotMatch(text,/skeleton constructor:|retained dependencies|resolved to bone/);
    });
  }
});

test('missing or contradictory skeleton evidence across contexts cannot promote constructor or camera diagnostics',async()=>{
  const mutations=[
    context=>{delete context.player_components;},
    context=>{context.player_components=null;},
    context=>{context.player_components.state='not_initialized';},
    context=>{context.player_components.skeleton_constructor_initialized=false;},
    context=>{context.player_components.animation_sampled=true;},
    context=>{delete context.player_components.player_physics_initialized;},
    context=>{context.player_components.camera_frame_ready=true;},
    context=>{context.player_components.retained_payload_bytes++;},
    context=>{context.player_components.retained_resources=4;},
    context=>{context.player_components.bone_count=0;},
    context=>{context.player_components.root_count=5;},
    context=>{context.player_components.camera_bone=4;},
    context=>{context.player_components.camera_parent_chain_length=0;},
    context=>{context.player_components.camera_parent_chain_length=5;},
    context=>{context.player_components.camera_parent_chain_sha256=null;},
    context=>{context.player_components.camera_tag_found=false;},
    context=>{context.player_components.constructor_state_sha256='invalid';},
    context=>{context.player_components.selected_character++;},
  ];
  for (const mutate of mutations) {
    const first=playerComponentsContext(), second=playerComponentsContext();mutate(second);
    await inspect(embed(missionProgressReport([first,second])),d=>{
      const text=d.querySelector('#mission-summary').textContent;
      assert.match(text,/First-person skeleton constructor evidence has not been recorded completely/);
      assert.doesNotMatch(text,/skeleton constructor:|retained dependencies|resolved to bone/);
      assert.match(text,/World and player initialization remain incomplete/);
    });
  }
});

function explicitModeContext() {
  const context=playerComponentsContext();
  context.player_components.mode_plan={
    state:'planned',input_source:'explicit_host_state',requested_mode:0x090000a1,previous_mode:0,
    fallback_mode:0x090000a2,target_index:7,return_word:0x090000b1,attempts:1,
    matched_transitions:1,planned_commands:2,mask_operations:1,start_sets:1,layer_groups:2,
    child_requests:3,previous_matches:1,fallback_matches:0,plan_sha256:'e'.repeat(64),
    plan_encoding:'mode_plan_be_v1',initial_request_verified:false,runtime_children_created:false,
    animation_sampled:false,camera_frame_ready:false,
  };
  return context;
}

test('optional mode summary counts explicit probes and samples the first plan after default contexts',async()=>{
  const defaults=playerComponentsContext(), first=explicitModeContext(), second=explicitModeContext();
  defaults.player_components.mode_plan=null;
  second.player_components.mode_plan.requested_mode=0x090000a3;
  await inspect(embed(missionProgressReport([defaults,playerComponentsContext(),first,second])),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/4 local context probes/);
    assert.match(text,/2 explicit mode-plan probes/);
    assert.match(text,/First explicit probe: requested 0x90000a1, previous 0x0, fallback 0x90000a2; state planned, return 0x90000b1/);
    assert.match(text,/1 attempt, 2 command plans, 2 layer groups and 3 child requests/);
    assert.doesNotMatch(text,/requested 0x90000a3|4 explicit mode-plan probes/);
    assert.match(text,/Initial request history remains unverified; runtime child creation, animation sampling and camera output remain incomplete/);
    assert.match(text,/Constructor identity is not a sampled animation pose/);
    assert.match(text,/World and player initialization remain incomplete/);
  });
});

test('missing mode plans preserve skeleton summaries while native misses and stale receipts stay explicit',async()=>{
  const legacy=playerComponentsContext(), empty=playerComponentsContext();empty.player_components.mode_plan=null;
  await inspect(embed(missionProgressReport([legacy,empty])),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/first-person skeleton constructor: 3 retained dependencies/);
    assert.match(text,/No explicit mode plan has been recorded/);
    assert.doesNotMatch(text,/mode-plan probe|skeleton constructor evidence has not been recorded completely/);
  });
  const missing=explicitModeContext();
  Object.assign(missing.player_components.mode_plan,{state:'target_missing',target_index:null,return_word:0,
    attempts:0,matched_transitions:0,planned_commands:0,mask_operations:0,start_sets:0,layer_groups:0,
    child_requests:0,previous_matches:0});
  await inspect(embed(missionProgressReport([missing])),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/1 explicit mode-plan probe/);
    assert.match(text,/state target missing, return 0x0; 0 attempts, 0 command plans/);
    assert.doesNotMatch(text,/state planned/);
  });
  const zero=explicitModeContext();
  Object.assign(zero.player_components.mode_plan,{state:'no_nonzero_return',return_word:0,attempts:2});
  await inspect(embed(missionProgressReport([zero])),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/state no nonzero return, return 0x0; 2 attempts, 2 command plans/);
    assert.match(text,/runtime child creation, animation sampling and camera output remain incomplete/);
  });
  for (const state of ['not_run','stale','failed']) {
    const report=missionProgressReport([explicitModeContext()]);report.mission.state=state;
    await inspect(embed(report),d=>{
      const text=d.querySelector('#mission-summary').textContent;
      assert.match(text,/No current retained-resource evidence/);
      assert.doesNotMatch(text,/explicit mode-plan probe|state planned|command plans/);
    });
  }
});

test('contradictory mode plans across explicit probes cannot promote animation or invalidate constructor evidence',async()=>{
  const mutations=[
    mode=>{mode.state='sampled';},
    mode=>{mode.input_source='inferred_initial_state';},
    mode=>{mode.initial_request_verified=true;},
    mode=>{mode.runtime_children_created=true;},
    mode=>{mode.animation_sampled=true;},
    mode=>{mode.camera_frame_ready=true;},
    mode=>{delete mode.return_word;},
    mode=>{mode.requested_mode=-1;},
    mode=>{mode.target_index=null;},
    mode=>{mode.attempts=3;},
    mode=>{mode.matched_transitions=2;},
    mode=>{mode.planned_commands=3;},
    mode=>{mode.start_sets=0;mode.mask_operations=2;},
    mode=>{mode.layer_groups=0;},
    mode=>{mode.plan_sha256=null;},
    mode=>{mode.state='target_missing';mode.target_index=null;},
    mode=>{mode.state='no_nonzero_return';mode.return_word=0;},
  ];
  for (const mutate of mutations) {
    const first=explicitModeContext(), second=explicitModeContext();mutate(second.player_components.mode_plan);
    await inspect(embed(missionProgressReport([playerComponentsContext(),first,second])),d=>{
      const text=d.querySelector('#mission-summary').textContent;
      assert.match(text,/Explicit mode-plan evidence has not been recorded completely/);
      assert.doesNotMatch(text,/explicit mode-plan probes|First explicit probe:|state planned/);
      assert.match(text,/first-person skeleton constructor: 3 retained dependencies/);
      assert.match(text,/World and player initialization remain incomplete/);
    });
  }
});

function initializedLayersContext() {
  const context=playerComponentsContext();
  context.player_components.initial_layers={
    state:'fresh_fp_layers_initialized',boundary:'fresh_fp_constructor_callback_snapshot',
    inventory:{inventory_key:0x020000a5,records:1,selected_record:0,count:1,limit:1,flags:0,ordinal_raw:255,
      guns_resource:0x010900a6,guns_payload_bytes:2048,metadata_source_index:2,metadata_primary_key:0x4b0000a5,
      weapon_registry_keys:8,weapon_order_keys:6,selected_order_index:4,primary_request:0x090000a5,
      secondary_request:0x090000a6,static_data:{game_id:'SJBE52',revision:0,main_dol_partition_offset:4096,
        dol_bytes:6087648,dol_sha256:'0d101e5eba1aca38847c03d7a353f5156f3ec2d9025d6c6773d105561532208b',
        data_section_index:5,data_virtual_address:0x8056bba0,data_bytes:383136,
        data_sha256:'76d3456f33f013817ea80ddec93635c466d25bba233929800399b4d3e703836d'}},
    mode_requests:2,active_layers:2,live_nodes:3,controls:1,script_children:2,
    bound_clips:7,unique_animations:3,captured_bones:4,selection_mask:251,clear_mask:251,
    slot_zero_nodes:2,slot_one_nodes:1,membership_sha256:'e'.repeat(64),bindings_sha256:'f'.repeat(64),
    post_selector_scalars_applied:true,post_selector_head_writes:2,raw_frame:null,prewarm_pose:null,animation_ticked:false,pose_sampled:false,
    player_initialized:false,camera_frame_ready:false,
  };
  return context;
}

test('owned layer summary selects first nonnull snapshot and reports measured ownership without a player frame',async()=>{
  const defaults=playerComponentsContext(), first=initializedLayersContext(), second=initializedLayersContext();
  defaults.player_components.initial_layers=null;
  second.player_components.initial_layers.bound_clips=8;
  await inspect(embed(missionProgressReport([defaults,playerComponentsContext(),first,second])),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/2 local owned-layer snapshots/);
    assert.match(text,/First initialized context: 2 source mode requests, 2 layers and 3 owned nodes \(1 control, 2 class04 children\); 7 bound clips and 3 unique skeletal animations; 4 captured bones/);
    assert.match(text,/Default inventory 0x20000a5 selected record 0; supplied SJBE52 revision 0 DATA5 profile verified/);
    assert.match(text,/Both post-selector head writes applied \(2\); animation tick, pose sampling and camera output remain incomplete/);
    assert.match(text,/No raw animation frame decode has been recorded/);
    assert.match(text,/World and player initialization remain incomplete/);
    assert.doesNotMatch(text,/8 bound clips|player ready|camera ready|playable game|100%/i);
  });
});

test('absent layer snapshots preserve legacy skeleton and explicit mode summaries',async()=>{
  const missing=playerComponentsContext(), empty=explicitModeContext();
  empty.player_components.initial_layers=null;
  await inspect(embed(missionProgressReport([missing,empty])),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/No owned animation-layer snapshot has been recorded/);
    assert.match(text,/first-person skeleton constructor: 3 retained dependencies/);
    assert.match(text,/1 explicit mode-plan probe/);
    assert.doesNotMatch(text,/First initialized context:|owned-layer evidence has not/);
  });
});

test('contradictory stale and private layer evidence cannot promote or leak a snapshot',async()=>{
  const mutations=[
    layers=>{layers.animation_ticked=true;}, layers=>{layers.pose_sampled=true;},
    layers=>{layers.player_initialized=true;}, layers=>{layers.camera_frame_ready=true;},
    layers=>{layers.post_selector_scalars_applied=false;}, layers=>{delete layers.boundary;},
    layers=>{layers.post_selector_head_writes=1;}, layers=>{delete layers.raw_frame;},
    layers=>{delete layers.prewarm_pose;},
    layers=>{layers.live_nodes++;}, layers=>{layers.slot_one_nodes++;},
    layers=>{layers.unique_animations=layers.bound_clips+1;}, layers=>{layers.captured_bones++;},
    layers=>{layers.bound_clips=1;}, layers=>{layers.controls=0;},
    layers=>{layers.bindings_sha256='bad';}, layers=>{layers.inventory.count=2;},
    layers=>{layers.inventory.selected_record=1;}, layers=>{layers.inventory.selected_order_index=6;},
    layers=>{layers.inventory.static_data.data_section_index=4;},
    layers=>{layers.inventory.static_data.dol_sha256='0'.repeat(64);},
    layers=>{layers.inventory.static_data.data_sha256='0'.repeat(64);},
    layers=>{layers.inventory.static_data.data_bytes++;},
    layers=>{layers.inventory.static_data.game_id='<img src=x onerror=alert(1)>';},
  ];
  for (const mutate of mutations) {
    const first=initializedLayersContext(), second=initializedLayersContext();
    mutate(second.player_components.initial_layers);
    await inspect(embed(missionProgressReport([playerComponentsContext(),first,second])),d=>{
      const text=d.querySelector('#mission-summary').textContent;
      assert.match(text,/Owned animation-layer evidence has not been recorded completely/);
      assert.doesNotMatch(text,/First initialized context:|profile verified|local owned-layer snapshots|<img/);
      assert.match(text,/first-person skeleton constructor: 3 retained dependencies/);
    });
  }
  const conflicting=initializedLayersContext();
  conflicting.player_components.mode_plan=explicitModeContext().player_components.mode_plan;
  await inspect(embed(missionProgressReport([conflicting])),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/Owned animation-layer evidence has not been recorded completely/);
    assert.match(text,/1 explicit mode-plan probe/);
    assert.doesNotMatch(text,/First initialized context:/);
  });
  const incomplete=initializedLayersContext();incomplete.residency.complete=false;
  await inspect(embed(missionProgressReport([incomplete])),d=>{
    assert.match(d.querySelector('#mission-summary').textContent,/Owned animation-layer evidence has not been recorded completely/);
    assert.doesNotMatch(d.querySelector('#mission-summary').textContent,/First initialized context:/);
  });
  for (const state of ['not_run','stale','failed']) {
    const report=missionProgressReport([initializedLayersContext()]);report.mission.state=state;
    await inspect(embed(report),d=>{
      const text=d.querySelector('#mission-summary').textContent;
      assert.match(text,/No current retained-resource evidence/);
      assert.doesNotMatch(text,/First initialized context:|profile verified|bound clips/);
    });
  }
  const context=initializedLayersContext(), layers=context.player_components.initial_layers;
  Object.assign(layers,{scope:'PRIVATE_SCOPE',original_rows:['PRIVATE_ASSET'],path:'D:\\PRIVATE_PATH'});
  Object.assign(layers.inventory.static_data,{path:'D:\\PRIVATE_DOL',original_bytes:[101,102,103]});
  await inspect(embed(missionProgressReport([context])),d=>{
    assert.match(d.querySelector('#mission-summary').textContent,/First initialized context:/);
    // Embedded fixture JSON is input; only the rendered surface is summarized.
    assert.doesNotMatch(d.querySelector('main').textContent,/PRIVATE_SCOPE|PRIVATE_ASSET|PRIVATE_PATH|PRIVATE_DOL/);
  });
});

function rawFrameContext() {
  const context=initializedLayersContext();
  context.player_components.initial_layers.raw_frame={
    state:'raw_frame_decoded',source_child:0x840000a5,selected_clip:1,blend_fraction_bits:0,
    requested_format_flags_raw:0,animation_reference:0x830000a6,animation_directory_index:3,frame:0,
    rotations:4,translations:2,exact_branch_rotations:1,unverified_sqrt_rotations:3,
    raw_frame_sha256:'1'.repeat(64),camera_raw_sha256:'2'.repeat(64),camera_raw_exact_branch:true,
    raw_encoding:'raw_polynomial_frame_be_v1',cached_frames:1,decode_calls:1,cache_hits:0,
    root_bias_applied:false,skin_mapping_applied:false,layer_blending_applied:false,skeleton_committed:false,
    native_numeric_parity_verified:false,first_displayed_pose_verified:false,camera_frame_ready:false,
  };
  return context;
}

test('raw frame summary selects first nonnull decode among ordinary and initialized contexts',async()=>{
  const first=rawFrameContext(), second=rawFrameContext();
  second.player_components.initial_layers.raw_frame.selected_clip=2;
  await inspect(embed(missionProgressReport([playerComponentsContext(),initializedLayersContext(),first,second])),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/3 local owned-layer snapshots/);
    assert.match(text,/2 local raw-frame decodes. First decoded context: clip 1, frame 0; 4 rotations and 2 sparse translations/);
    assert.match(text,/Rotation precision: 1 exact-branch results, 3 unverified host-sqrt results/);
    assert.match(text,/Exact-branch labels describe evaluated XYZ, not native numeric parity/);
    assert.match(text,/Owned cache: 1 frames, 1 decode calls, 0 hits/);
    assert.match(text,/this is not the first game pose or a camera frame/);
    assert.doesNotMatch(text,/clip 2,|numeric parity verified|first pose verified|camera ready/i);
  });
});

test('contradictory raw decode and precision evidence cannot promote a frame',async()=>{
  const mutations=[
    raw=>{raw.state='sampled_pose';}, raw=>{raw.rotations=0;},
    raw=>{raw.rotations=5;}, raw=>{raw.translations=5;},
    raw=>{raw.exact_branch_rotations++;}, raw=>{raw.unverified_sqrt_rotations=-1;},
    raw=>{raw.selected_clip=7;}, raw=>{raw.source_child=3;},
    raw=>{raw.animation_reference=0;}, raw=>{raw.blend_fraction_bits=0x7fc00000;},
    raw=>{raw.cached_frames=0;}, raw=>{raw.decode_calls=0;},
    raw=>{raw.raw_frame_sha256='invalid';}, raw=>{raw.camera_raw_sha256=null;},
    raw=>{raw.root_bias_applied=true;}, raw=>{raw.skin_mapping_applied=true;},
    raw=>{raw.layer_blending_applied=true;}, raw=>{raw.skeleton_committed=true;},
    raw=>{raw.native_numeric_parity_verified=true;}, raw=>{raw.first_displayed_pose_verified=true;},
    raw=>{raw.camera_frame_ready=true;}, raw=>{delete raw.raw_encoding;},
  ];
  for (const mutate of mutations) {
    const first=rawFrameContext(), second=rawFrameContext();
    mutate(second.player_components.initial_layers.raw_frame);
    await inspect(embed(missionProgressReport([first,second])),d=>{
      const text=d.querySelector('#mission-summary').textContent;
      assert.match(text,/Raw-frame evidence has not been recorded completely/);
      assert.match(text,/Both post-selector head writes applied/);
      assert.doesNotMatch(text,/First decoded context:|local raw-frame decodes|Rotation precision:/);
    });
  }
});

test('raw summaries hide private vectors and reject stale receipts without inventing a camera',async()=>{
  const context=rawFrameContext(), raw=context.player_components.initial_layers.raw_frame;
  Object.assign(raw,{scope:'PRIVATE_RAW_SCOPE',path:'D:\\PRIVATE_RAW_PATH',rotations_original:['PRIVATE_QUAT'],
    translations_original:['PRIVATE_TRANSLATION'],source_code:'PRIVATE_CODE'});
  raw.camera_raw_sha256=null;raw.camera_raw_exact_branch=false;
  await inspect(embed(missionProgressReport([context])),d=>{
    assert.match(d.querySelector('#mission-summary').textContent,/1 local raw-frame decode/);
    assert.doesNotMatch(d.querySelector('main').textContent,/PRIVATE_RAW_SCOPE|PRIVATE_RAW_PATH|PRIVATE_QUAT|PRIVATE_TRANSLATION|PRIVATE_CODE/);
  });
  for (const state of ['not_run','stale','failed']) {
    const report=missionProgressReport([rawFrameContext()]);report.mission.state=state;
    await inspect(embed(report),d=>{
      const text=d.querySelector('#mission-summary').textContent;
      assert.match(text,/No current retained-resource evidence/);
      assert.doesNotMatch(text,/First decoded context:|local raw-frame decode|Rotation precision:/);
    });
  }
});

function prewarmContext() {
  const context=initializedLayersContext(), layers=context.player_components.initial_layers;
  context.player_components.animation_sampled=true;
  layers.animation_ticked=true;layers.pose_sampled=true;
  layers.prewarm_pose={
    state:'source_reset_prewarm_locals_committed',seconds_bits:0x3e4ccccd,baseline:60,scaled_step_bits:0x41400000,
    children:[
      {node:3,reference:0x840000a5,normalized_position_bits:0,previous_phase_bits:0,
        advanced_phase_bits:0x3dcccccd,transition_bits:0x3f800000,reciprocal_bits:0},
      {node:2,reference:0x840000a6,normalized_position_bits:0,previous_phase_bits:0,
        advanced_phase_bits:0x3d4ccccd,transition_bits:0x3f800000,reciprocal_bits:0},
    ],
    updated_controls:1,sampled_controls:1,sampled_scripts:1,skipped_layers:1,selected_clip:1,frame:0,
    fade_bits:0x3f800000,animation_translations:2,skin_translations:2,committed_bones:4,scratch_flags_raw:0x4000,
    locals_sha256:'3'.repeat(64),locals_encoding:'source_context_locals_be_v1',post_update_bindings_sha256:'4'.repeat(64),
    unverified_sqrt_rotations:3,root_bias_applied:true,skin_mapping_applied:true,skeleton_committed:true,
    matrices_refreshed:false,native_numeric_parity_verified:false,external_step_executed:false,
    first_displayed_pose_verified:false,camera_frame_ready:false,
  };
  return context;
}

test('optional prewarm summary uses measured counts while preserving constructor and precision boundaries',async()=>{
  const first=prewarmContext(), second=prewarmContext();
  second.player_components.initial_layers.prewarm_pose.animation_translations=1;
  second.player_components.initial_layers.prewarm_pose.skin_translations=3;
  await inspect(embed(missionProgressReport([playerComponentsContext(),initializedLayersContext(),first,second])),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/3 local owned-layer snapshots/);
    assert.match(text,/2 local source reset prewarms. First prewarmed context: 0.2 seconds at baseline 60; 2 script children and 1 controls updated/);
    assert.match(text,/4 local bones committed from 2 animation translations and 2 source skin translations; 1 zero-weight layers skipped/);
    assert.match(text,/Rotation precision retains 3 unverified host-sqrt results/);
    assert.match(text,/Native numeric parity and the first displayed pose remain unverified; matrix refresh and camera output remain incomplete/);
    assert.match(text,/fixed prewarm does not execute the external scheduler/);
    assert.match(text,/Constructor identity is not a sampled animation pose/);
    assert.match(text,/World and player initialization remain incomplete/);
    assert.doesNotMatch(text,/48 local bones|1 animation translations and 3|animation tick, pose sampling and camera output remain incomplete|camera ready|native numeric parity verified/i);
  });
  await inspect(embed(missionProgressReport([initializedLayersContext()])),d=>{
    assert.match(d.querySelector('#mission-summary').textContent,/No source reset prewarm has been recorded/);
    assert.match(d.querySelector('#mission-summary').textContent,/animation tick, pose sampling and camera output remain incomplete/);
  });
});

test('malformed prewarm and conflicting sampling flags fail closed across all supplied contexts',async()=>{
  const mutations=[
    context=>{context.player_components.animation_sampled=false;},
    context=>{context.player_components.initial_layers.animation_ticked=false;},
    context=>{context.player_components.initial_layers.pose_sampled=false;},
    context=>{context.player_components.initial_layers.prewarm_pose=null;},
    context=>{delete context.player_components.initial_layers.prewarm_pose;},
    context=>{context.player_components.initial_layers.raw_frame=rawFrameContext().player_components.initial_layers.raw_frame;},
    context=>{context.player_components.initial_layers.inventory.count=2;},
    context=>{context.residency.complete=false;},
    ...[
      warm=>{warm.seconds_bits=0;},warm=>{warm.baseline=30;},warm=>{delete warm.scaled_step_bits;},
      warm=>{warm.updated_controls=2;},warm=>{warm.sampled_scripts=0;},warm=>{warm.sampled_controls=2;},
      warm=>{warm.skipped_layers=2;},warm=>{warm.selected_clip=7;},warm=>{warm.frame=1;},
      warm=>{warm.animation_translations++;},warm=>{warm.committed_bones=48;},
      warm=>{warm.unverified_sqrt_rotations=5;},warm=>{warm.unverified_sqrt_rotations=0;},
      warm=>{warm.fade_bits=0;},warm=>{warm.scratch_flags_raw=0;},warm=>{warm.root_bias_applied=false;},
      warm=>{warm.skin_mapping_applied=false;},warm=>{warm.skeleton_committed=false;},
      warm=>{warm.matrices_refreshed=true;},warm=>{warm.native_numeric_parity_verified=true;},
      warm=>{warm.external_step_executed=true;},warm=>{warm.first_displayed_pose_verified=true;},
      warm=>{warm.camera_frame_ready=true;},warm=>{warm.locals_sha256='invalid';},
      warm=>{warm.post_update_bindings_sha256=null;},warm=>{warm.locals_encoding='unknown';},
      warm=>{warm.children.pop();},warm=>{warm.children[1].node=warm.children[0].node;},
      warm=>{warm.children[0].reference=0;},warm=>{warm.children[0].previous_phase_bits=1;},
      warm=>{warm.children[0].advanced_phase_bits=0x7fc00000;},warm=>{warm.children[0].transition_bits=0;},
      warm=>{warm.children[0].reciprocal_bits=1;},
    ].map(mutate=>context=>mutate(context.player_components.initial_layers.prewarm_pose)),
  ];
  for (const mutate of mutations) {
    const first=prewarmContext(), second=prewarmContext();mutate(second);
    await inspect(embed(missionProgressReport([first,second])),d=>{
      const text=d.querySelector('#mission-summary').textContent;
      assert.match(text,/Owned animation-layer evidence has not been recorded completely/);
      assert.match(text,/Prewarm evidence has not been recorded completely/);
      assert.doesNotMatch(text,/First prewarmed context:|local source reset prewarms|local bones committed|First initialized context:/);
      assert.match(text,/World and player initialization remain incomplete/);
    });
  }
});

test('prewarm summaries never expose private locals and stale receipts do not promote the fixed update',async()=>{
  const context=prewarmContext(), warm=context.player_components.initial_layers.prewarm_pose;
  Object.assign(warm,{scope:'PRIVATE_PREWARM_SCOPE',path:'D:\\PRIVATE_PREWARM_PATH',
    locals_original:['PRIVATE_LOCAL_POSE'],source_code:'PRIVATE_PREWARM_CODE'});
  Object.assign(warm.children[0],{private_clip:['PRIVATE_CLIP']});
  await inspect(embed(missionProgressReport([context])),d=>{
    assert.match(d.querySelector('#mission-summary').textContent,/1 local source reset prewarm/);
    assert.doesNotMatch(d.querySelector('main').textContent,/PRIVATE_PREWARM_SCOPE|PRIVATE_PREWARM_PATH|PRIVATE_LOCAL_POSE|PRIVATE_PREWARM_CODE|PRIVATE_CLIP/);
  });
  for (const state of ['not_run','stale','failed']) {
    const report=missionProgressReport([prewarmContext()]);report.mission.state=state;
    await inspect(embed(report),d=>{
      const text=d.querySelector('#mission-summary').textContent;
      assert.match(text,/No current retained-resource evidence/);
      assert.doesNotMatch(text,/First prewarmed context:|local source reset prewarm|local bones committed/);
    });
  }
});

function cameraBoneContext(cached=false) {
  const context=prewarmContext(), player=context.player_components;
  player.camera_bone_transform={
    state:'local_camera_bone_fk_prepared',boundary:'source_reset_prewarm_committed_locals',
    cache_selection:cached ? 'fresh_fp_actor_cache' : 'explicit_disabled_inspection',
    tag:player.camera_tag,bone:player.camera_bone,chain_length:player.camera_parent_chain_length,
    local_transforms:3,matrix_products:cached ? 2 : 3,
    cache_slots:cached ? player.bone_count : 0,cache_hits:cached ? 1 : 0,
    cache_misses:cached ? 3 : 0,valid_cached_bones:cached ? 3 : 0,
    unverified_input_rotations:2,host_matrix_arithmetic_unverified:true,
    encoding:cached ? 'source_cached_camera_bone_fk_be_v1' : 'source_camera_bone_fk_be_v1',
    transform_sha256:'5'.repeat(64),committed_locals_sha256:player.initial_layers.prewarm_pose.locals_sha256,
    tag_bound:true,committed_pose_used:true,owner_world_applied:false,matrices_refreshed:false,
    native_numeric_parity_verified:false,first_displayed_pose_verified:false,camera_frame_ready:false,
    scope:'Authored local transform fixture',
  };
  return context;
}

test('camera bone FK summary reports generic measured chain and cache work without camera readiness',async()=>{
  const first=cameraBoneContext(), cached=cameraBoneContext(true), constructor=playerComponentsContext();
  constructor.player_components.camera_bone_transform=null;
  const player=cached.player_components, bone=player.camera_bone_transform;
  Object.assign(player,{bone_count:6,camera_bone:5,camera_parent_chain_length:2});
  player.initial_layers.captured_bones=6;
  Object.assign(player.initial_layers.prewarm_pose,{committed_bones:6,skin_translations:4,unverified_sqrt_rotations:5});
  Object.assign(bone,{bone:5,chain_length:2,local_transforms:2,matrix_products:1,
    cache_slots:6,cache_misses:2,valid_cached_bones:2,unverified_input_rotations:1});
  await inspect(embed(missionProgressReport([constructor,first,cached])),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/2 local camera-bone FK probes from committed prewarm poses/);
    assert.match(text,/First transform: 3 bones in its chain, 3 local transforms and 3 matrix products; 2 provisional input rotations/);
    assert.match(text,/1 fresh FP cache probe; first cache: 6 slots, 1 hit, 2 misses, 2 valid bones and 1 matrix product/);
    assert.match(text,/Host matrix arithmetic remains unverified/);
    assert.match(text,/Owner\/world transforms, cached skeleton matrix refresh, full native FP\/TP cache setup and camera output remain incomplete/);
    assert.match(text,/World and player initialization remain incomplete/);
    assert.doesNotMatch(text,/48 slots|bone 47|camera ready|native numeric parity verified/i);
  });
});

test('camera bone evidence fails closed across probes on missing provenance contradictory work or matrix exports',async()=>{
  const mutations=[
    context=>{context.player_components.initial_layers.prewarm_pose=null;},
    context=>{context.player_components.initial_layers.inventory.count=2;},
    context=>{context.player_components.animation_sampled=false;},
    context=>{context.residency.complete=false;},
    ...[
      bone=>{bone.boundary='constructor_identity';},bone=>{bone.tag++;},bone=>{bone.bone++;},
      bone=>{bone.chain_length=0;},bone=>{bone.local_transforms++;},bone=>{bone.matrix_products++;},
      bone=>{bone.cache_hits=2;},bone=>{bone.cache_misses++;},bone=>{bone.cache_slots++;},
      bone=>{bone.valid_cached_bones++;},bone=>{bone.unverified_input_rotations=4;},
      bone=>{bone.host_matrix_arithmetic_unverified=false;},bone=>{bone.encoding='unknown';},
      bone=>{bone.transform_sha256='invalid';},bone=>{bone.committed_locals_sha256='6'.repeat(64);},
      bone=>{bone.tag_bound=false;},bone=>{bone.committed_pose_used=false;},
      bone=>{bone.owner_world_applied=true;},bone=>{bone.matrices_refreshed=true;},
      bone=>{bone.native_numeric_parity_verified=true;},bone=>{bone.first_displayed_pose_verified=true;},
      bone=>{bone.camera_frame_ready=true;},bone=>{delete bone.cache_selection;},
      bone=>{bone.full_bits=['PRIVATE_MATRIX_LANES'];},
    ].map(mutate=>context=>mutate(context.player_components.camera_bone_transform)),
  ];
  for (const mutate of mutations) {
    const first=cameraBoneContext(), second=cameraBoneContext(true);mutate(second);
    await inspect(embed(missionProgressReport([first,second])),d=>{
      const text=d.querySelector('#mission-summary').textContent;
      assert.match(text,/Camera-bone transform evidence has not been recorded completely/);
      assert.doesNotMatch(text,/First transform:|FK probes from committed|first cache:/);
      assert.doesNotMatch(d.querySelector('main').textContent,/PRIVATE_MATRIX_LANES/);
      assert.match(text,/World and player initialization remain incomplete/);
    });
  }
});

test('absent null and stale camera bone evidence preserve legacy constructor and readiness boundaries',async()=>{
  const absent=playerComponentsContext(), empty=playerComponentsContext();
  empty.player_components.camera_bone_transform=null;
  await inspect(embed(missionProgressReport([absent,empty])),d=>{
    const text=d.querySelector('#mission-summary').textContent;
    assert.match(text,/first-person skeleton constructor:/);
    assert.match(text,/No local camera-bone FK has been recorded/);
    assert.doesNotMatch(text,/transform evidence has not been recorded completely|First transform:|first cache:/);
  });
  for (const state of ['not_run','stale','failed']) {
    const report=missionProgressReport([cameraBoneContext(true)]);report.mission.state=state;
    await inspect(embed(report),d=>{
      const text=d.querySelector('#mission-summary').textContent;
      assert.match(text,/No current retained-resource evidence/);
      assert.doesNotMatch(text,/First transform:|FK probe|first cache:|local transforms/);
    });
  }
});

async function inspect(html, assertions) {
  const window = new Window({url:'http://localhost/',settings:{enableJavaScriptEvaluation:true,disableJavaScriptFileLoading:true,disableCSSFileLoading:true}});
  const errors=[];
  window.addEventListener('error',e=>errors.push(e.message));
  try {
    window.document.write(html);
    await window.happyDOM.waitUntilComplete();
    assert.deepEqual(errors,[],'dashboard script must finish without errors');
    await assertions(window.document,window);
    assert.deepEqual(errors,[],'interactions must finish without errors');
  } finally { await window.happyDOM.close(); }
}

test('generated dashboard agrees with machine-readable report without local asset assumptions',async()=>{
  await inspect(generated,d=>{
    assert.deepEqual(JSON.parse(d.querySelector('#report-data').textContent),latest);
    assert.equal(d.querySelectorAll('#criteria tr').length,latest.criteria.length);
    assert.equal(d.querySelector('#parity-count').textContent,`${latest.parity.verified} of ${latest.parity.total} tracked criteria verified`);
    assert.equal(d.querySelectorAll('#gates .gate').length,kinds.length);
    assert.equal(d.querySelector('#menu-number').textContent,latest.menu.state==='passed'?'100%':latest.menu.state==='failed'?'0%':'—');
    assert.equal(latest.initial_checkpoint.total,1);
    assert.equal(latest.initial_checkpoint.passed,latest.menu.state==='passed'?1:0);
    assert.equal(d.querySelectorAll('#compatibility-rows tr').length,latest.compatibility.length);
    assert.equal(d.querySelectorAll('#history-chart .chart-dot.parity').length,latest.history.length);
    const lines=latest.coverage.details?.totals?.lines;
    if(lines) assert.equal(d.querySelector('#coverage-number').textContent,`${lines.percent.toFixed(1)}%`);
    if(latest.resources.details?.grenade_prototype) assert.match(d.querySelector('#resource-summary').textContent,/resolved in script directory/);
    if(latest.resources.details?.collision) {
      assert.match(d.querySelector('#collision-summary').textContent,/root 0x601 mesh entries decoded/);
      assert.match(d.querySelector('#collision-summary').textContent,/original-runtime collision remain unverified/);
    }
    assert.equal(d.querySelectorAll('script[src],link[rel="stylesheet"]').length,0,'offline report must be self contained');
  });
});

test('fresh checkout renders missing evidence and empty history honestly',async()=>{
  await inspect(embed(fixture()),d=>{
    assert.match(d.querySelector('#coverage-count').textContent,/No measured coverage/);
    assert.match(d.querySelector('#collision-summary').textContent,/decoding is separate from live collision results/);
    assert.match(d.querySelector('#mesh-contact-summary').textContent,/records alone do not establish contact behavior/);
    assert.match(d.querySelector('#world-request-summary').textContent,/do not prove BSP\/portal composition/);
    assert.match(d.querySelector('#world-line-summary').textContent,/navigation rays do not establish this segment path/);
    assert.equal(d.querySelectorAll('#gates .not_run').length,kinds.length);
    assert.equal(d.querySelector('#menu-number').textContent,'—');
    assert.equal(d.querySelector('#menu-count').textContent,'0 of 1 current launch checks passed');
    assert.match(d.querySelector('#menu-summary').textContent,/No native menu launch evidence/);
    assert.match(d.querySelector('#startup-summary').textContent,/No startup media evidence/);
    assert.match(d.querySelector('#music-summary').textContent,/has not been recorded/);
    assert.equal(d.querySelectorAll('#history-chart circle').length,0);
    assert.match(d.querySelector('#observations').textContent,/0 measured snapshots/);
    assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
  });
});

test('older snapshots without a menu receipt display an unmeasured launch checkpoint',async()=>{
  const report=fixture();
  delete report.menu;
  delete report.compatibility[0].menu;
  await inspect(embed(report),d=>{
    assert.equal(d.querySelector('#menu-state .status').textContent,'Not run');
    assert.equal(d.querySelector('#menu-number').textContent,'—');
    assert.equal(d.querySelectorAll('#gates .gate').length,kinds.length);
    assert.equal(d.querySelectorAll('#compatibility-rows td').length,9);
    assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
  });
});

for (const state of ['passed','failed','stale']) {
  test(`${state} menu evidence measures only the initial launch checkpoint`,async()=>{
    const report=fixture();
    report.menu={state,time_unix:1699999999,details:{reference_compared:false,
      window:{frames_presented:3,resize_count:1,scene_uploads:1,textures:17,adapter:'Authored GPU',backend:'Vulkan'}}};
    report.compatibility[0].menu=state;
    await inspect(embed(report),d=>{
      assert.equal(d.querySelector('#menu-state .status').textContent,state[0].toUpperCase()+state.slice(1));
      assert.equal(d.querySelector('#menu-number').textContent,state==='passed'?'100%':state==='failed'?'0%':'—');
      assert.equal(d.querySelector('#menu-count').textContent,`${state==='passed'?1:0} of 1 current launch checks passed`);
      assert.match(d.querySelector('#menu-summary').textContent,/3 hidden native frames, 1 resize, 1 scene upload and 17 textures/);
      assert.match(d.querySelector('#menu-checkpoint').textContent,/not total-game progress or visual parity/);
      assert.match(d.querySelector('#menu-summary').textContent,/Visible controls and original-runtime comparison are unverified/);
      if(state==='stale') assert.match(d.querySelector('#menu-summary').textContent,/Previous evidence is stale/);
      assert.equal(d.querySelectorAll('#compatibility-rows .not_implemented').length,1);
      assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
    });
  });
}

test('failed menu smoke without window details remains a failure',async()=>{
  const report=fixture();
  report.menu={state:'failed',details:{message:'Native menu smoke failed; inspect local terminal output.',reference_compared:false}};
  await inspect(embed(report),d=>{
    assert.equal(d.querySelector('#menu-number').textContent,'0%');
    assert.match(d.querySelector('#menu-summary').textContent,/Native menu launch smoke failed/);
    assert.match(d.querySelector('#menu-summary').textContent,/no passing launch is recorded/);
  });
});

function menuInteractionFixture(state='passed') {
  const report=fixture();
  const settings={music_volume:54,sound_volume:92,dialog_volume:88,brightness:105};
  report.menu={state,details:{
    interaction:{window:{menu:{screen:'options',settings:{...settings}},
      menu_session:{writes:3,dirty:false,probe:[{input:'up'},{input:'confirm'},{input:'left'},{input:'back'}]},
      playback:{menu_audio:{cues:[{info:{sound_hash:1}},{info:{sound_hash:2}},{info:{sound_hash:3}},{info:{sound_hash:4}}],voices_started:17,capacity_dropped:2},
        audio:{oneshot_started:17,oneshot_completed:16,oneshot_active:1,oneshot_output_frames:3848,source_frames_consumed:772}}}},
    persisted_reopen:{window:{menu:{screen:'sound_settings',settings:{...settings}},
      menu_session:{writes:0,dirty:false,probe:[{input:'confirm'}]}}},
    credits:{window:{menu:{screen:'credits'},credits_lines:29,menu_elapsed_seconds:1.25}},
    credits_return:{window:{menu:{screen:'extras'}}},
  }};
  return report;
}

test('fresh menu interaction evidence reports actual settings voices and credits without changing launch or parity denominators',async()=>{
  const report=menuInteractionFixture();
  await inspect(embed(report),d=>{
    const text=d.querySelector('#menu-interaction-summary').textContent;
    assert.match(text,/Options \/ Sound Settings: 4 applied inputs; music 54, sound 92, dialog 88, brightness 105/);
    assert.match(text,/preferences reopened with matching values \(3 writes; no writes on reopen\)/);
    assert.match(text,/4 source cue bindings; 17 independent voices started, 16 completed, 1 active, 2 dropped/);
    assert.match(text,/3,848 voice output frames while music consumed 772 source frames/);
    assert.match(text,/Credits opened 29 loaded source lines and returned to Extras/);
    assert.match(text,/DOM checks do not establish human controls, speaker output or original visual\/audio parity/);
    assert.equal(d.querySelector('#menu-count').textContent,'1 of 1 current launch checks passed');
    assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
  });
});

test('missing legacy and incomplete menu receipts never invent submenu persistence sound or credits passes',async()=>{
  for (const report of [fixture(),{...fixture(),menu:{state:'passed',details:{}}}]) {
    await inspect(embed(report),d=>{
      const text=d.querySelector('#menu-interaction-summary').textContent;
      assert.match(text,/evidence has not been recorded/);
      assert.doesNotMatch(text,/0 independent voices|0 loaded source lines|preferences reopened with matching values/);
    });
  }
  const partial=menuInteractionFixture();
  partial.menu.details.persisted_reopen.window.menu.settings.music_volume=100;
  delete partial.menu.details.interaction.window.playback.audio.oneshot_completed;
  partial.menu.details.credits_return.window.menu.screen={unavailable:'credits'};
  await inspect(embed(partial),d=>{
    const text=d.querySelector('#menu-interaction-summary').textContent;
    assert.match(text,/Settings navigation\/persistence evidence has not been recorded completely/);
    assert.match(text,/Native menu sound submission has not been recorded completely/);
    assert.match(text,/Credits opening\/return evidence has not been recorded completely/);
    assert.doesNotMatch(text,/preferences reopened with matching values|independent voices started|Credits opened/);
  });
});

test('stale and failed menu receipts cannot promote their retained interaction details into current results',async()=>{
  for (const state of ['stale','failed']) {
    await inspect(embed(menuInteractionFixture(state)),d=>{
      const text=d.querySelector('#menu-interaction-summary').textContent;
      assert.match(text,state==='stale'?/Previous menu interaction evidence is stale/:/Menu interaction gate failed/);
      assert.match(text,/No current submenu|no current submenu/);
      assert.doesNotMatch(text,/Passed menu gate|preferences reopened with matching values|17 independent voices|Credits opened 29/);
      assert.equal(d.querySelector('#menu-count').textContent,'0 of 1 current launch checks passed');
      assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
    });
  }
});

function gameSessionFixture(state='passed') {
  const report=fixture();
  const controller=active=>({requested:1,loaded:1,cancelled:0,failed:0,retired_workers:0,retired_worker_failures:0,
    active,provider:'host_local_semantic',native_provider_timing_verified:false,scope:'Authored CPU retention fixture'});
  const active=difficulty=>({stage:'awaiting_world',difficulty,level_hash:0x0181016b,start_point_id:1,
    session_active:true,local_users:1,resources_retained:5+difficulty,map_instances:3,source_start_points:7+difficulty,
    world_registered:false,player_created:false,camera_ready:false,mode:3,backend:2,
    manager_ready_flags_raw:0,user_ready_flags_raw:0,source_create_counter:7,created_session_selected:true,failure:null,world_initialization:null});
  const trace=steps=>steps.map(([input,effect,selected,screen='new_game_difficulty'],index)=>({input,effect,selected,screen,frame:(index+1)*6}));
  const probe=(session,steps,suspended=true)=>({reference_compared:false,capture_png_sha256:'a'.repeat(64),capture_png_bytes:15,
    window:{frames_presented:60,game_session_shutdown_complete:true,game_session:session,
      playback:{stage:'main_menu',session_suspended:suspended,muted:false,audio:{primary_gain:suspended?0:1,device_errors:0}},
      menu:{screen:steps.at(-1)[3]||'new_game_difficulty',selected:steps.at(-1)[2]},menu_session:{probe:trace(steps)}}});
  const difficulty_probes=Array.from({length:4},(_,difficulty)=>probe(controller(active(difficulty)),
    [['confirm','accept',0],...Array.from({length:difficulty},(_,i)=>['down','navigate',i+1]),['confirm','accept',difficulty],['confirm','none',difficulty]]));
  for(const sample of difficulty_probes.slice(1)) {
    sample.window.playback.muted=true;sample.window.playback.audio=null;sample.window.playback.menu_audio=null;
  }
  const difficulty_page=probe({...controller(null),requested:0,loaded:0},[['confirm','accept',0]],false);
  const cancel_reentry=probe({...controller(active(1)),requested:2,loaded:2,cancelled:1,retired_workers:1},
    [['confirm','accept',0],['confirm','accept',0],['back','back',0,'main_menu'],['confirm','accept',0],['down','navigate',1],['confirm','accept',1]]);
  const returned_menu=probe({...controller(null),loaded:0,cancelled:1,retired_workers:1},
    [['confirm','accept',0],['confirm','accept',0],['back','back',0,'main_menu']],false);
  report.menu={state,details:{game_session:{difficulty_page,difficulty_probes,cancel_reentry,returned_menu,
    reference_compared:false,scope:'Authored native handler fixture'}}};
  return report;
}

test('native New Game summary validates all difficulties retention cancellation and music without promoting gameplay',async()=>{
  await inspect(embed(gameSessionFixture()),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/all 4 difficulties admitted one local user to Tutorial 0x0181016b, StartPoint 1/);
    assert.match(text,/one request per probe despite repeated confirmation/);
    assert.match(text,/First probe retained 5 resources, 3 bound map instances and 7 prepared source StartPoints/);
    assert.match(text,/Difficulty navigation, cancellation\/re-entry and return to the main menu were checked; primary menu music was suspended/);
    assert.match(text,/manager and user readiness flags remain 0/);
    assert.match(text,/world registration, player creation and camera output remain incomplete/);
    assert.match(text,/does not establish native provider timing or original gameplay parity/);
    assert.equal(d.querySelector('#menu-number').textContent,'100%');
    assert.equal(d.querySelector('#menu-count').textContent,'1 of 1 current launch checks passed');
    assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
  });
});

function actorDefinitionFixture() {
  return {resource_hash:0x01090013,spreadsheet_hash:0x14000021,payload_bytes:640,
    primary_rows:6,secondary_rows:3,registered_primary:5,duplicate_primary:1,runtime_groups:2,
    grouped_members:4,group8_members:2,selection_calls:2,resource_shared:true,
    primary_present:true,secondary_present:true,fresh_cursors_zero:false,default_record_bound:false,
    special_records_bound:false,original_cursor_history_verified:false,rng_bound:true,
    native9_constructor_ready:true,rng_algorithm:'source_two_step_lcg32',rng_origin:'fresh_constructor_snapshot',
    rng_draws:1,rng_state_sha256:'d'.repeat(64),original_rng_history_verified:false,
    state_sha256:'c'.repeat(64),scope:'Authored mutable actor snapshot; original histories unverified'};
}

function resourceStateFixture() {
  return {origin:'supplied_snapshot',lifetime:'native_window_controller',shared_owner:true,
    records:2,capacity:4,nonzero_values:1,reset_limit:333,lookups:1,hits:1,misses:0,
    snapshot_bound:false,original_history_verified:false,state_sha256:'e'.repeat(64),
    scope:'Authored supplied state owner, separate from any captured buffer or original history'};
}

function withWorldInitialization(report) {
  const data=report.menu.details.game_session;
  for(const sample of [...data.difficulty_probes,data.cancel_reentry]) {
    const snapshot=sample.window.game_session.active;
    snapshot.source_start_points=2;
    // Two relay-kind objects include one with a disabled header. Never derive
    // active count by adding all relay objects to the StartPoint count.
    snapshot.world_initialization={maps_activated:snapshot.map_instances,original_slots:39,
      disabled_slots:1,filtered_slots:2,unclassified_slots:3,unsupported_factory_slots:1,unsupported_shape_slots:0,
      owned_triggers:30,common_scratch:30,membership_entries:30,active_triggers:3,
      start_points:2,generic_triggers:2,common_be_triggers:1,staged_triggers:0,stopped_animators:27,
      resource08_triggers:1,active91_triggers:2,range0c_triggers:1,radius06_triggers:1,actor12_triggers:2,range82_triggers:1,a3_triggers:2,range27_triggers:1,common09_triggers:2,ad_triggers:1,radius03_triggers:1,common8a_triggers:2,radius1b_triggers:2,radius74_triggers:1,shape71_triggers:2,materialized_shapes:1,radius88_triggers:1,auxac_triggers:2,source_null_slots:2,map_initialization_blocked:0,
      blocked_references:1,resolved_references:4,
      animators_initialized:2,animators_failed:0,shared_variables:8,unwritten_scratch_words:4,state_flags:4,
      execution_slots:32,active_executions:0,create_events_denied:2,registration_events_unmapped:2,
      zone_queries:39,catalog_entries:0,catalog_complete:false,native_zone_precision_verified:false,
      shape_selectors_initialized:1,actor_definitions:actorDefinitionFixture(),resource_states:resourceStateFixture(),navigation:null,post_trigger_resources:null,world_hooks:null,loader:null,player_initialization:null,
      state_sha256:'b'.repeat(64),scope:'Authored partial world fixture'};
  }
  return report;
}

function withCompleteCatalog(report) {
  withWorldInitialization(report);
  const data=report.menu.details.game_session;
  for(const probe of [...data.difficulty_probes,data.cancel_reentry]) {
    const world=probe.window.game_session.active.world_initialization;
    world.original_slots-=world.unclassified_slots+world.unsupported_factory_slots;
    world.zone_queries=world.original_slots;
    world.unclassified_slots=0;
    world.unsupported_factory_slots=0;
    world.blocked_references=0;
    world.materialized_shapes=world.shape71_triggers;
    world.catalog_entries=world.start_points;
    world.catalog_complete=true;
  }
  return report;
}

test('validated partial world ownership shows original slots and blockers without promoting a catalog or player',async()=>{
  await inspect(embed(withWorldInitialization(gameSessionFixture())),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/preserved 39 original ordinal slots and owns 30 triggers: 2 StartPoints, 2 generic triggers, 1 other common triggers/);
    assert.match(text,/1 resource triggers, 2 relay triggers, 1 range triggers, 1 radius triggers, 2 actor triggers, 1 extended range triggers, 2 subtype A3 triggers, 1 subtype 27 range triggers, 2 subtype 09 common triggers, 1 subtype AD triggers, 1 subtype 03 radius triggers, 2 subtype 8A common triggers, 2 subtype 1B radius triggers, 1 subtype 74 radius triggers, 2 subtype 71 shape triggers, 1 subtype 88 radius triggers and 2 subtype AC auxiliary triggers/);
    assert.match(text,/30 membership entries contain 3 active triggers; 0 staged objects remain outside membership/);
    assert.match(text,/Source returned no object for 2 original slots/);
    assert.match(text,/2 base animators initialized; 27 shutdown paths found no animator. 4 references resolved/);
    assert.match(text,/3 unclassified slots, 1 unsupported factory slots, 0 unsupported shape slots and 1 blocked references remain/);
    assert.doesNotMatch(text,/membership entries contain 4 active triggers/);
    assert.match(text,/No animation execution or completed live spawn catalog is claimed/);
    assert.match(text,/1 source shape selectors retained; 1 shapes materialized as owned data/);
    assert.match(text,/Actor definitions reuse the retained character resource: 6 primary rows retain 5 registered definitions and 1 duplicate inputs; 3 secondary rows remain owned/);
    assert.match(text,/2 runtime groups contain 4 members, including 2 in group 8/);
    assert.match(text,/2 source selections and 1 two-step LCG draws use a fresh constructor snapshot; original cursor and RNG histories remain unverified/);
    assert.match(text,/resource state owns 2 records \(1 nonzero values\) and completed 1 lookups: 1 hits and 0 source misses/);
    assert.match(text,/supplied snapshot has no retained capture buffer; original resource-state history remains unverified/);
    assert.match(text,/manager and user readiness flags remain 0/);
    assert.equal(d.querySelector('#menu-number').textContent,'100%');
  });
});

test('complete trigger catalog includes source-null slots without promoting the rest of the world',async()=>{
  await inspect(embed(withCompleteCatalog(gameSessionFixture())),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/preserved 35 original ordinal slots and owns 30 triggers/);
    assert.match(text,/Source returned no object for 2 original slots. 30 membership entries contain 3 active triggers/);
    assert.match(text,/0 unclassified slots, 0 unsupported factory slots, 0 unsupported shape slots and 0 blocked references remain/);
    assert.match(text,/Trigger constructor, reference and map initialization passes completed; the trigger catalog owns 2 StartPoint entries/);
    assert.match(text,/Sessions await remaining world initialization/);
    assert.match(text,/world registration, player creation and camera output remain incomplete/);
    assert.doesNotMatch(text,/No animation execution or completed live spawn catalog is claimed/);
    assert.equal(d.querySelector('#menu-count').textContent,'1 of 1 current launch checks passed');
  });
});

test('catalog completion fails closed on unfinished passes partial entries missing null slots or gameplay readiness',async()=>{
  const changes=[
    snapshot=>{const w=snapshot.world_initialization;w.unclassified_slots=1;w.original_slots++;w.zone_queries++;},
    snapshot=>{const w=snapshot.world_initialization;w.unsupported_factory_slots=1;w.original_slots++;w.zone_queries++;},
    snapshot=>{const w=snapshot.world_initialization;w.unsupported_shape_slots=1;w.original_slots++;w.zone_queries++;},
    snapshot=>{snapshot.world_initialization.blocked_references=1;},
    snapshot=>{const w=snapshot.world_initialization;w.map_initialization_blocked=1;w.actor_definitions.native9_constructor_ready=false;},
    snapshot=>{snapshot.world_initialization.materialized_shapes=1;},
    snapshot=>{snapshot.world_initialization.catalog_entries=1;},
    snapshot=>{snapshot.world_initialization.catalog_complete=false;},
    snapshot=>{snapshot.world_initialization.catalog_complete='true';},
    snapshot=>{delete snapshot.world_initialization.source_null_slots;},
    snapshot=>{snapshot.world_initialization.source_null_slots=-1;},
    snapshot=>{snapshot.world_registered=true;},snapshot=>{snapshot.player_created=true;},snapshot=>{snapshot.camera_ready=true;},
  ];
  for(const change of changes) {
    const report=withCompleteCatalog(gameSessionFixture());
    change(report.menu.details.game_session.difficulty_probes[0].window.game_session.active);
    await inspect(embed(report),d=>{
      const text=d.querySelector('#game-session-summary').textContent;
      assert.match(text,/has not been recorded completely/);
      assert.doesNotMatch(text,/trigger catalog owns/);
    });
  }
});

test('world initialization schema counters digests readiness and mixed recording fail closed',async()=>{
  const changes=[
    world=>{world.original_slots=16;},world=>{world.zone_queries=16;},
    world=>{world.maps_activated=2;},world=>{world.owned_triggers=1;},
    world=>{world.common_scratch=1;},world=>{world.membership_entries=1;},
    world=>{world.active_triggers=11;},world=>{world.start_points=3;},world=>{world.generic_triggers=3;},
    world=>{world.common_be_triggers=2;},world=>{world.resource08_triggers=2;},world=>{world.active91_triggers=3;},
    world=>{world.range0c_triggers=2;},world=>{world.radius06_triggers=2;},world=>{world.staged_triggers=6;},world=>{world.stopped_animators=2;},
    world=>{world.actor12_triggers=3;},world=>{world.range82_triggers=2;},world=>{world.map_initialization_blocked=3;},
    world=>{world.a3_triggers=3;},world=>{world.range27_triggers=2;},
    world=>{world.a3_triggers=-1;},world=>{delete world.range27_triggers;},
    world=>{world.common09_triggers=3;},world=>{world.ad_triggers=2;},
    world=>{world.common09_triggers=-1;},world=>{delete world.ad_triggers;},
    world=>{world.radius03_triggers=2;},world=>{world.common8a_triggers=3;},
    world=>{world.radius03_triggers=-1;},world=>{delete world.common8a_triggers;},
    world=>{world.radius1b_triggers=3;},world=>{delete world.radius1b_triggers;},
    world=>{world.radius74_triggers=2;},world=>{world.shape71_triggers=3;},
    world=>{world.materialized_shapes=3;},world=>{world.materialized_shapes=-1;},world=>{delete world.materialized_shapes;},
    world=>{world.radius88_triggers=2;},world=>{world.auxac_triggers=3;},
    world=>{world.source_null_slots=3;},world=>{world.source_null_slots=-1;},world=>{delete world.source_null_slots;},
    world=>{world.animators_failed=2;},world=>{world.blocked_references=241;},world=>{world.resolved_references=240;},
    world=>{world.execution_slots=31;},world=>{world.unwritten_scratch_words=0;},
    world=>{world.active_executions=1;},world=>{world.create_events_denied=1;},
    world=>{world.registration_events_unmapped=1;},world=>{world.catalog_entries=2;},
    world=>{world.catalog_complete=true;},world=>{world.native_zone_precision_verified=true;},
    world=>{world.state_sha256='B'.repeat(64);},world=>{delete world.state_flags;},
    world=>{world.original_payload=[1,2,3];},
    world=>{world.shape_selectors_initialized=31;},world=>{world.shape_selectors_initialized=-1;},
  ];
  for(const change of changes) {
    const report=withWorldInitialization(gameSessionFixture());
    change(report.menu.details.game_session.difficulty_probes[0].window.game_session.active.world_initialization);
    await inspect(embed(report),d=>{
      assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
      assert.doesNotMatch(d.querySelector('#game-session-summary').textContent,/all 4 difficulties admitted|Partial world initialization preserved/);
    });
  }
  for(const remove of [snapshot=>{delete snapshot.world_initialization;},snapshot=>{snapshot.world_initialization=null;}]) {
    const report=withWorldInitialization(gameSessionFixture());
    remove(report.menu.details.game_session.difficulty_probes[0].window.game_session.active);
    await inspect(embed(report),d=>{
      assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
    });
  }
});

test('resource-state ownership accounting history bounds and original exports fail closed',async()=>{
  const changes=[
    states=>{states.origin='original_runtime';},states=>{states.lifetime='mission_worker';},
    states=>{states.shared_owner=false;},states=>{states.records=5;},states=>{states.capacity=1;},
    states=>{states.nonzero_values=3;},states=>{states.reset_limit=0x100000000;},
    states=>{states.lookups=2;states.hits=1;states.misses=1;},states=>{states.hits=2;},states=>{states.misses=1;},
    states=>{states.snapshot_bound='false';},states=>{states.original_history_verified=true;},
    states=>{states.state_sha256='E'.repeat(64);},states=>{states.scope='';},
    states=>{states.keys=[1,2];},states=>{states.values=[0,1];},states=>{delete states.reset_limit;},
    states=>{states.records=0;states.capacity=0;states.nonzero_values=0;},
  ];
  for(const change of changes) {
    const report=withWorldInitialization(gameSessionFixture());
    change(report.menu.details.game_session.difficulty_probes[0].window.game_session.active.world_initialization.resource_states);
    await inspect(embed(report),d=>{
      const text=d.querySelector('#game-session-summary').textContent;
      assert.match(text,/has not been recorded completely/);
      assert.doesNotMatch(text,/source misses/);
    });
  }
});

test('recorded source misses differ from an absent provider and capture buffers do not imply original history',async()=>{
  const report=withWorldInitialization(gameSessionFixture());
  for(const probe of [...report.menu.details.game_session.difficulty_probes,report.menu.details.game_session.cancel_reentry]) {
    const states=probe.window.game_session.active.world_initialization.resource_states;
    Object.assign(states,{origin:'fresh_constructor_snapshot',records:0,capacity:0,nonzero_values:0,hits:0,misses:1,snapshot_bound:true});
  }
  await inspect(embed(report),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/resource state owns 0 records \(0 nonzero values\) and completed 1 lookups: 0 hits and 1 source misses/);
    assert.match(text,/fresh constructor snapshot has a retained capture buffer; original resource-state history remains unverified/);
    assert.match(text,/world registration, player creation and camera output remain incomplete/);
  });
  for(const probe of [...report.menu.details.game_session.difficulty_probes,report.menu.details.game_session.cancel_reentry]) {
    probe.window.game_session.active.world_initialization.resource_states=null;
  }
  await inspect(embed(report),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/Resource-state lookup evidence is absent; no empty-dictionary result is inferred/);
    assert.doesNotMatch(text,/0 hits and 1 source misses/);
  });
  const world=report.menu.details.game_session.difficulty_probes[0].window.game_session.active.world_initialization;
  world.resource_states=resourceStateFixture();
  await inspect(embed(report),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
  });
  delete world.resource_states;
  await inspect(embed(report),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
  });
});

test('zero blocked references do not complete a world with unclassified constructors',async()=>{
  const report=withWorldInitialization(gameSessionFixture());
  for(const probe of [...report.menu.details.game_session.difficulty_probes,report.menu.details.game_session.cancel_reentry]) {
    const world=probe.window.game_session.active.world_initialization;
    world.blocked_references=0;
    world.unclassified_slots+=1;
    world.original_slots+=1;
    world.zone_queries+=1;
  }
  await inspect(embed(report),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/4 unclassified slots, 1 unsupported factory slots, 0 unsupported shape slots and 0 blocked references remain/);
    assert.match(text,/No animation execution or completed live spawn catalog is claimed/);
    assert.match(text,/world registration, player creation and camera output remain incomplete/);
  });
  const snapshot=report.menu.details.game_session.difficulty_probes[0].window.game_session.active;
  snapshot.world_initialization.catalog_complete=true;
  await inspect(embed(report),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
  });
  snapshot.world_initialization.catalog_complete=false;
  snapshot.world_registered=true;
  await inspect(embed(report),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
  });
});

test('materialized shape data can use a supported fallback without a selector and still does not establish readiness',async()=>{
  const report=withWorldInitialization(gameSessionFixture());
  for(const probe of [...report.menu.details.game_session.difficulty_probes,report.menu.details.game_session.cancel_reentry]) {
    probe.window.game_session.active.world_initialization.shape_selectors_initialized=0;
  }
  await inspect(embed(report),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/0 source shape selectors retained; 1 shapes materialized as owned data/);
    assert.match(text,/Materialization does not establish collision queries or world readiness/);
    assert.match(text,/world registration, player creation and camera output remain incomplete/);
  });
  const snapshot=report.menu.details.game_session.difficulty_probes[0].window.game_session.active;
  snapshot.world_initialization.materialized_shapes=3;
  await inspect(embed(report),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
  });
  snapshot.world_initialization.materialized_shapes=1;
  snapshot.world_registered=true;
  await inspect(embed(report),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
  });
});

test('actor registry metadata rejects contradictory ownership cursor readiness and original row exports',async()=>{
  const changes=[
    actor=>{actor.resource_hash=1;},actor=>{actor.spreadsheet_hash=2;},
    actor=>{actor.primary_rows=7;},actor=>{actor.registered_primary=4;},actor=>{actor.duplicate_primary=0;},
    actor=>{actor.runtime_groups=5;},actor=>{actor.grouped_members=6;},actor=>{actor.group8_members=5;},
    actor=>{actor.primary_present=false;},actor=>{actor.secondary_present=false;},
    actor=>{actor.resource_shared=false;},actor=>{actor.selection_calls=0;},actor=>{actor.fresh_cursors_zero=true;},
    actor=>{actor.default_record_bound=true;},actor=>{actor.special_records_bound=true;},
    actor=>{actor.original_cursor_history_verified=true;},actor=>{actor.rng_bound=false;},actor=>{actor.original_rng_history_verified=true;},
    actor=>{actor.rng_draws=3;},actor=>{actor.rng_algorithm='host_random';},actor=>{actor.rng_origin='original_runtime';},
    actor=>{actor.rng_state_sha256='D'.repeat(64);},actor=>{actor.original_rng_state=[1,2,3];},
    actor=>{actor.state_sha256='C'.repeat(64);},actor=>{actor.scope='';},
    actor=>{actor.rows=[[1,2,3]];},actor=>{delete actor.secondary_rows;},
  ];
  for(const change of changes) {
    const report=withWorldInitialization(gameSessionFixture());
    change(report.menu.details.game_session.difficulty_probes[0].window.game_session.active.world_initialization.actor_definitions);
    await inspect(embed(report),d=>{
      assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
      assert.doesNotMatch(d.querySelector('#game-session-summary').textContent,/Actor definitions reuse/);
    });
  }
});

test('standalone world audits can omit actor ownership without inventing provider readiness',async()=>{
  const report=withWorldInitialization(gameSessionFixture());
  for(const probe of [...report.menu.details.game_session.difficulty_probes,report.menu.details.game_session.cancel_reentry]) {
    probe.window.game_session.active.world_initialization.actor_definitions=null;
  }
  await inspect(embed(report),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/1 source shape selectors retained/);
    assert.doesNotMatch(text,/Actor definitions reuse/);
  });
  report.menu.details.game_session.difficulty_probes[0].window.game_session.active.world_initialization.actor_definitions=actorDefinitionFixture();
  await inspect(embed(report),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
  });
});

test('blocked actor map initialization remains a partial checkpoint without world or player readiness',async()=>{
  const report=withWorldInitialization(gameSessionFixture());
  for(const probe of [...report.menu.details.game_session.difficulty_probes,report.menu.details.game_session.cancel_reentry]) {
    const world=probe.window.game_session.active.world_initialization;
    world.map_initialization_blocked=1;
    world.actor_definitions.native9_constructor_ready=false;
  }
  await inspect(embed(report),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/1 actor map-reference initializations remain blocked/);
    assert.match(text,/Actor construction or map-reference initialization remains incomplete/);
    assert.match(text,/world registration, player creation and camera output remain incomplete/);
    assert.doesNotMatch(text,/Supported actor constructor and map-reference scans completed/);
  });
  report.menu.details.game_session.difficulty_probes[0].window.game_session.active.world_initialization.actor_definitions.native9_constructor_ready=true;
  await inspect(embed(report),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
  });
});

test('staged construction stays outside membership and retains its unsupported ordinal slot',async()=>{
  const report=withWorldInitialization(gameSessionFixture());
  for(const probe of [...report.menu.details.game_session.difficulty_probes,report.menu.details.game_session.cancel_reentry]) {
    Object.assign(probe.window.game_session.active.world_initialization,{
      staged_triggers:3,membership_entries:27,stopped_animators:24,unsupported_factory_slots:4,
    });
  }
  await inspect(embed(report),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/owns 30 triggers/);
    assert.match(text,/27 membership entries contain 3 active triggers; 3 staged objects remain outside membership/);
    assert.match(text,/4 unsupported factory slots, 0 unsupported shape slots and 1 blocked references remain/);
    assert.match(text,/world registration, player creation and camera output remain incomplete/);
  });
  const world=report.menu.details.game_session.difficulty_probes[0].window.game_session.active.world_initialization;
  world.unsupported_factory_slots=0;
  await inspect(embed(report),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
    assert.doesNotMatch(d.querySelector('#game-session-summary').textContent,/membership entries contain/);
  });
});

test('required geometry adds staging capacity without counting provider-free kinds',async()=>{
  const report=withWorldInitialization(gameSessionFixture());
  for(const probe of [...report.menu.details.game_session.difficulty_probes,report.menu.details.game_session.cancel_reentry]) {
    const world=probe.window.game_session.active.world_initialization;
    Object.assign(world,{staged_triggers:14,membership_entries:16,stopped_animators:13,unsupported_factory_slots:15,materialized_shapes:0});
    world.actor_definitions.native9_constructor_ready=false;
    world.resource_states=null;
  }
  await inspect(embed(report),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/16 membership entries contain 3 active triggers; 14 staged objects remain outside membership/);
    assert.match(text,/2 subtype A3 triggers, 1 subtype 27 range triggers, 2 subtype 09 common triggers, 1 subtype AD triggers, 1 subtype 03 radius triggers, 2 subtype 8A common triggers, 2 subtype 1B radius triggers, 1 subtype 74 radius triggers, 2 subtype 71 shape triggers, 1 subtype 88 radius triggers and 2 subtype AC auxiliary triggers/);
    assert.match(text,/Actor construction or map-reference initialization remains incomplete/);
    assert.match(text,/world registration, player creation and camera output remain incomplete/);
  });
  const world=report.menu.details.game_session.difficulty_probes[0].window.game_session.active.world_initialization;
  Object.assign(world,{staged_triggers:15,membership_entries:15,stopped_animators:12,unsupported_factory_slots:16});
  await inspect(embed(report),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
  });
});

test('incomplete contradictory or exported New Game data never produces an admission summary',async()=>{
  const mutations=[
    data=>{delete data.difficulty_page;},
    data=>{data.difficulty_page.window.game_session.requested=1;},
    data=>{data.difficulty_probes.pop();},
    data=>{data.difficulty_probes[2].window.game_session.active.difficulty=1;},
    data=>{data.difficulty_probes[0].window.game_session.active.user_ready_flags_raw=0x100;},
    data=>{data.difficulty_probes[0].window.game_session.active.camera_ready=true;},
    data=>{data.difficulty_probes[0].window.game_session.active.original_matrix=[1,2,3];},
    data=>{data.difficulty_probes[0].window.playback.audio.primary_gain=1;},
    data=>{data.difficulty_probes[1].window.playback.audio={primary_gain:1};},
    data=>{data.difficulty_probes[0].window.menu_session.probe.at(-1).effect='accept';},
    data=>{data.difficulty_probes[0].window.game_session.requested=2;},
    data=>{data.cancel_reentry.window.game_session.cancelled=0;},
    data=>{data.returned_menu.window.playback.session_suspended=true;},
    data=>{data.returned_menu.window.menu.screen='new_game_difficulty';},
    data=>{data.difficulty_probes[0].window.game_session_shutdown_complete=false;},
    data=>{data.difficulty_probes[0].capture_png_sha256='bad';},
  ];
  for(const mutate of mutations) {
    const report=gameSessionFixture();mutate(report.menu.details.game_session);
    await inspect(embed(report),d=>{
      const text=d.querySelector('#game-session-summary').textContent;
      assert.match(text,/evidence has not been recorded completely/);
      assert.doesNotMatch(text,/all 4 difficulties admitted|First probe retained|primary menu music was suspended/);
      assert.equal(d.querySelector('#menu-number').textContent,'100%','session validation does not change the initial launch denominator');
    });
  }
});

test('failed stale and legacy menu records cannot promote retained New Game probe details',async()=>{
  for(const state of ['failed','stale','not_run']) {
    await inspect(embed(gameSessionFixture(state)),d=>{
      const text=d.querySelector('#game-session-summary').textContent;
      assert.doesNotMatch(text,/all 4 difficulties admitted|First probe retained|primary menu music was suspended/);
      assert.match(text,state==='failed'?/menu gate failed/:state==='stale'?/evidence is stale/:/has not been recorded completely/);
      assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
    });
  }
  for(const report of [fixture(),{...fixture(),menu:{state:'passed',details:{}}}]) {
    await inspect(embed(report),d=>{
      assert.match(d.querySelector('#game-session-summary').textContent,/evidence has not been recorded completely/);
      assert.doesNotMatch(d.querySelector('#game-session-summary').textContent,/all 4 difficulties admitted/);
    });
  }
});

test('older snapshots without startup evidence preserve the initial menu denominator',async()=>{
  const report=fixture(); delete report.startup; delete report.compatibility[0].startup;
  report.menu={state:'passed'};
  await inspect(embed(report),d=>{
    assert.equal(d.querySelector('#startup-state .status').textContent,'Not run');
    assert.match(d.querySelector('#startup-summary').textContent,/No startup media evidence/);
    assert.equal(d.querySelector('#menu-number').textContent,'100%');
    assert.equal(d.querySelector('#menu-count').textContent,'1 of 1 current launch checks passed');
    assert.equal(d.querySelectorAll('#gates .gate').length,kinds.length);
  });
});
for (const state of ['passed','stale','failed']) {
  test(`${state} startup evidence reports decoding and native PCM without promoting parity`,async()=>{
    const report=fixture(); report.menu={state:'passed'};
    report.startup={state,time_unix:1699999999,details:{reference_compared:false,
      media:{clips:[{movie:'activision'},{movie:'eurocom'},{movie:'mgm'},{movie:'frontend'}],video_frames_decoded:8,interleaved_audio_samples_decoded:12,
        title_music:{resource_hash:0x2d500017,sound_hash:0x2d700109,wave_hash:0x2da021bf,channels:2,sample_rate:31999,decoded_sample_frames:2,nonzero_samples:3}},
      presentations:[{stage:'title',window:{playback:{music:{decoded_frames:2},audio:{source_frames_consumed:20,output_frames_submitted:30,device_errors:0}}}}],
      title_motion:{captures_differ:true}}};
    report.compatibility[0].startup=state;
    await inspect(embed(report),d=>{
      assert.equal(d.querySelector('#startup-state .status').textContent,state[0].toUpperCase()+state.slice(1));
      const text=d.querySelector('#startup-summary').textContent;
      assert.match(text,/Activision, Eurocom, MGM, Frontend/);
      assert.match(text,/8 video frames decoded and 12 interleaved PCM samples/);
      assert.match(text,/consumed 20 source frames and submitted 30 output frames/);
      assert.match(text,/First\/last title captures differ/);
      assert.match(text,/does not measure speaker output or original synchronization/);
      const music=d.querySelector('#music-summary').textContent;
      assert.match(music,/sound 0x2d700109 → wave 0x2da021bf in bank 0x2d500017/);
      assert.ok(music.includes(`2 channels at ${(31999).toLocaleString()} Hz`));
      assert.match(music,/2 sample frames decoded with 3 nonzero samples/);
      assert.match(music,/Native title PCM consumed 20 source frames/);
      assert.match(music,/loop history, mixing and cue synchronization remain unverified/);
      if(state==='stale') assert.match(text,/Previous startup evidence is stale/);
      if(state==='failed') assert.match(text,/failed gate is not a passing startup result/);
      if(state==='stale') assert.match(music,/Previous music evidence is stale/);
      if(state==='failed') assert.match(music,/No passing music result is recorded/);
      assert.equal(d.querySelector('#menu-number').textContent,'100%');
      assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
      assert.equal(d.querySelectorAll('#gates .gate').length,kinds.length);
    });
  });
}
test('failed startup without details stays failed and never becomes a zero-valued decode pass',async()=>{
  const report=fixture(); report.startup={state:'failed',details:{message:'device unavailable'}};
  await inspect(embed(report),d=>{
    assert.equal(d.querySelector('#startup-state .status').textContent,'Failed');
    assert.match(d.querySelector('#startup-summary').textContent,/no passing startup result is recorded/);
    assert.doesNotMatch(d.querySelector('#startup-summary').textContent,/0 video frames decoded/);
    assert.match(d.querySelector('#music-summary').textContent,/no passing binding\/decode evidence/);
    assert.equal(d.querySelector('#menu-number').textContent,'—');
  });
});

test('legacy startup details without music never invent a successful music decode',async()=>{
  const report=fixture();report.startup={state:'stale',details:{media:{clips:[],video_frames_decoded:1,interleaved_audio_samples_decoded:0},presentations:[]}};
  await inspect(embed(report),d=>{
    assert.match(d.querySelector('#music-summary').textContent,/has not been recorded/);
    assert.doesNotMatch(d.querySelector('#music-summary').textContent,/sample frames decoded/);
    assert.equal(d.querySelector('#startup-state .status').textContent,'Stale');
  });
});

function startupCueFixture(state='passed') {
  const report=fixture();
  report.startup={state,details:{
    media:{clips:[],video_frames_decoded:2,interleaved_audio_samples_decoded:6,
      intro_cue:{info:{resource_hash:0x2d500003,sound_hash:0x2d700123,wave_hash:0x2da00456,
        channels:2,sample_rate:24000,sample_frames:37},decoded_frames:37}},
    presentations:[{stage:'legal',window:{playback:{intro_cue_requests:99,intro_cue_voices:99}}},
      {stage:'title',window:{playback:{intro_cue_requests:3,intro_cue_voices:2,intro_cue_gain:0.75,
        audio:{source_frames_consumed:83,output_frames_submitted:101,device_errors:0,source_rate:32000,output_rate:48000,
          oneshot_started:2,oneshot_completed:1,oneshot_active:1,oneshot_output_frames:74}}}}],
    action_initialization:{window:{playback:{startup_resources:{state:'initialized',actions_ready:true,
      resource_hash:0x01090003,actions:7,composites:3,resolved_slots:5,unresolved_slots:7,error:null,
      scope:'Only authored Action tables; other native initializers remain pending'}}}},
  }};
  return report;
}

test('title intro cue binds authored assets and separates independent voices from primary music',async()=>{
  await inspect(embed(startupCueFixture()),d=>{
    const text=d.querySelector('#intro-cue-summary').textContent;
    assert.match(text,/Passed title intro cue diagnostics/);
    assert.match(text,/sound 0x2d700123 → wave 0x2da00456 in bank 0x2d500003/);
    assert.ok(text.includes(`2 channels at ${(24000).toLocaleString()} Hz`));
    assert.match(text,/37 sample frames decoded \(37 source frames\)/);
    assert.match(text,/Native title: 3 cue requests, 2 voices, gain 0.75/);
    assert.match(text,/2 started, 1 completed, 1 active; 74 voice output frames/);
    assert.ok(text.includes(`Primary music consumed 83 source frames at ${(32000).toLocaleString()} Hz; device output ${(48000).toLocaleString()} Hz`));
    assert.match(text,/Original mixing and cue synchronization remain unverified/);
    assert.doesNotMatch(text,/99|button|confirm/i);
    assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
  });
});

test('Action initializer reports nullable binding and retains its partial scope',async()=>{
  await inspect(embed(startupCueFixture()),d=>{
    const text=d.querySelector('#action-summary').textContent;
    assert.match(text,/state initialized; actions_ready true · resource 0x1090003/);
    assert.match(text,/7 action rows, 3 composite rows; 5 resolved slots and 7 unresolved \(native NULL\) slots/);
    assert.match(text,/Only authored Action tables; other native initializers remain pending/);
    assert.match(text,/only the supported Action initializer; complete native startup readiness remains unverified/);
    assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
    assert.equal(d.querySelectorAll('#compatibility-rows .not_implemented').length,1);
  });
});

test('passed legacy receipts never invent cue or Action evidence or zero-filled voice counters',async()=>{
  const report=fixture(); report.startup={state:'passed',details:{}};
  await inspect(embed(report),d=>{
    assert.match(d.querySelector('#intro-cue-summary').textContent,/binding\/decode has not been recorded/);
    assert.match(d.querySelector('#action-summary').textContent,/Action initializer evidence has not been recorded/);
    assert.doesNotMatch(d.querySelector('#intro-cue-summary').textContent,/0 voices|0 started|Passed title intro cue diagnostics/);
  });
  const partial=startupCueFixture(); delete partial.startup.details.presentations;
  await inspect(embed(partial),d=>{
    assert.match(d.querySelector('#intro-cue-summary').textContent,/Native title cue submission has not been sampled/);
    assert.doesNotMatch(d.querySelector('#intro-cue-summary').textContent,/0 voices|0 started/);
  });
});

for(const state of ['stale','failed']) {
  test(`${state} cue and Action diagnostics preserve state and errors without promoting readiness`,async()=>{
    const report=startupCueFixture(state);
    if(state==='failed') Object.assign(report.startup.details.action_initialization.window.playback.startup_resources,
      {state:'failed',actions_ready:false,error:'authored table unavailable'});
    await inspect(embed(report),d=>{
      const cue=d.querySelector('#intro-cue-summary').textContent,
        action=d.querySelector('#action-summary').textContent;
      for(const text of [cue,action]) {
        assert.match(text,new RegExp(`^${state}`,'i'));
        assert.match(text,/unverified/);
        assert.doesNotMatch(text,/^Passed/);
      }
      if(state==='stale') {
        assert.match(cue,/Previous cue evidence is stale/);
        assert.match(action,/Previous Action evidence is stale/);
      } else {
        assert.match(cue,/No passing cue result is recorded/);
        assert.match(action,/state failed; actions_ready false/);
        assert.match(action,/Error: authored table unavailable/);
        assert.match(action,/No passing Action result is recorded/);
      }
      assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
    });
  });
}

test('passed fade evidence reports actual capture and phase counts without a fixed sample total',async()=>{
  const report=fixture();
  report.startup={state:'passed',details:{fade_presentations:[
    {label:'strap-first',phase:'first',stage:'strap',warning_fade:true,frames:2,window:{}},
    {label:'strap-middle',phase:'middle',stage:'strap',warning_fade:true,frames:3,window:{}},
    {label:'strap-complete',phase:'complete',stage:'zapper',warning_fade:false,frames:4,window:{}},
    {label:'legal-first',phase:'first',stage:'legal',warning_fade:false,frames:2,window:{}},
    {label:'legal-complete',phase:'complete',stage:'legal',warning_fade:false,frames:4,window:{}},
  ]}};
  await inspect(embed(report),d=>{
    const text=d.querySelector('#fade-summary').textContent;
    assert.match(text,/Passed fade diagnostics/);
    assert.match(text,/5 hidden native captures \(first: 2, middle: 1, complete: 2\)/);
    assert.match(text,/Original frame timing and visual parity remain unverified/);
    assert.doesNotMatch(text,/9 hidden native captures/);
    assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
  });
});

test('a passed older startup receipt never invents fade checks when captures are missing',async()=>{
  const report=fixture(); report.startup={state:'passed',details:{}};
  await inspect(embed(report),d=>{
    const text=d.querySelector('#fade-summary').textContent;
    assert.match(text,/Passed startup gate/);
    assert.match(text,/Startup fades have not been checked/);
    assert.match(text,/no native fade captures were recorded/);
    assert.doesNotMatch(text,/Passed fade diagnostics|0 hidden native captures/);
  });
});

for(const state of ['stale','failed']) {
  test(`${state} fade evidence retains its state and sampled phase counts`,async()=>{
    const report=fixture();
    report.startup={state,details:{fade_presentations:[
      {label:'zapper-first',phase:'first',stage:'zapper',warning_fade:true,frames:2,window:{}},
      {label:'zapper-middle',phase:'middle',stage:'zapper',warning_fade:true,frames:3,window:{}},
      {label:'zapper-complete',phase:'complete',stage:'activision',warning_fade:false,frames:4,window:{}},
    ]}};
    await inspect(embed(report),d=>{
      const text=d.querySelector('#fade-summary').textContent;
      assert.match(text,new RegExp(`^${state[0].toUpperCase()+state.slice(1)} fade diagnostics`));
      assert.match(text,/3 hidden native captures \(first: 1, middle: 1, complete: 1\)/);
      assert.match(text,/Original frame timing and visual parity remain unverified/);
      if(state==='stale') assert.match(text,/Previous fade evidence is stale/);
      if(state==='failed') assert.match(text,/No passing fade result is recorded/);
      assert.doesNotMatch(text,/Passed fade diagnostics/);
      assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
    });
  });
}

test('sampled collision contacts retain their authored scope and precision limits',async()=>{
  const report=fixture();
  report.resources={state:'stale',details:{parsed:1,resources:1,local_images:0,decoded_mips:0,error_count:0,external_references:0,
    collision:{found:3,decoded:3,enabled_triangles:14,disabled_triangles:2,excluded_batches:1,excluded_declared_triangles:5,degenerate_enabled_triangles:1,
      box_triangle:{queries:8,sampled_triangles:4,samples_per_mesh_limit:2,contact_axes:{1:1,4:2},candidates_without_contact:1,far_queries:4,far_separated:4,plane_validated:2,plane_ambiguous:1,plane_degenerate:0}}}};
  await inspect(embed(report),d=>{
    const text=d.querySelector('#collision-summary').textContent;
    assert.match(text,/stale/i);
    assert.match(text,/8 queries over 4 triangles/);
    assert.match(text,/3 contacts, 1 candidates without contact/);
    assert.match(text,/4\/4 distant boxes separated/);
    assert.match(text,/1 within a coordinate-scaled rounding bound/);
    assert.match(text,/not live world results/);
    assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
  });
});

test('BSP evidence separates stable point queries from near-plane ambiguity',async()=>{
  const report=fixture();
  report.resources={state:'passed',details:{parsed:1,resources:1,local_images:0,decoded_mips:0,error_count:0,external_references:0,
    map_spatial:{decoded:3,maps:3,bsp_maps:2,absent_bsp_maps:1,reachable_nodes:17,zones:9,queries:12,validated_queries:8,ambiguous_queries:4,maximum_nodes_tested:6}}};
  await inspect(embed(report),d=>{
    const text=d.querySelector('#spatial-summary').textContent;
    assert.match(text,/3\/3 map directories decoded/);
    assert.match(text,/8 agree with independent f64 traversal, 4 are near a plane/);
    assert.match(text,/does not collect geometry/);
    assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
  });
});

test('portal evidence retains rounding limits and never presents live clearance',async()=>{
  const report=fixture();
  report.resources={state:'stale',details:{parsed:1,resources:1,local_images:0,decoded_mips:0,error_count:0,external_references:0,
    map_spatial:{decoded:1,maps:1,bsp_maps:1,absent_bsp_maps:0,reachable_nodes:3,zones:2,queries:3,validated_queries:2,ambiguous_queries:1,maximum_nodes_tested:2,
      portals:{maps:1,groups:4,rows:5,inline_shapes:1,linked_shapes:2,expanded_queries:9,stable_expansions:6,ambiguous_expansions:3,distance_calls:13,triangles_tested:20,strict_distance_checks:10,rounding_ambiguous_distances:1,degenerate_distance_calls:1,unstable_distance_calls:1,maximum_zones_returned:2}}}};
  await inspect(embed(report),d=>{
    const text=d.querySelector('#portal-summary').textContent;
    assert.match(text,/stale/i);
    assert.match(text,/9 authored radius queries: 6 stable zone-list agreements/);
    assert.match(text,/1 require a rounding bound, 1 contain determinant-rejected triangles/);
    assert.match(text,/1 are near-degenerate and unvalidated/);
    assert.match(text,/F32 cancellation is preserved/);
    assert.match(text,/No live geometry collection/);
    assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
  });
});

test('initial matrix evidence keeps scale-order and numeric scope explicit',async()=>{
  const report=fixture();
  report.resources={state:'stale',details:{parsed:1,resources:1,local_images:0,decoded_mips:0,error_count:0,external_references:0,
    initial_placements:{maps:2,placements:7,scale_order_changes:3,nonuniform_scale:4,negative_scale:1,zero_scale:1,engine_flag_4_placements:2,probed_points:35,strict_points:33,rounding_points:2}}};
  await inspect(embed(report),d=>{
    const text=d.querySelector('#placement-summary').textContent;
    assert.match(text,/stale/i);
    assert.match(text,/T\*S\*Rz\*Rx\*Ry/);
    assert.match(text,/3 matrices differ from scaling columns/);
    assert.match(text,/33 within 0.00001/);
    assert.match(text,/Initial transforms only; live mutation/);
    assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
  });
});

test('collision frame evidence separates authored modes, numeric limits and live records',async()=>{
  const report=fixture();
  report.resources={state:'stale',details:{parsed:1,resources:1,local_images:0,decoded_mips:0,error_count:0,external_references:0,
    collision_frames:{maps:2,placements:7,rigid_frames:7,general_frames:7,singular_general_frames:1,ill_conditioned_general_frames:1,validated_general_frames:5,strict_inverse_frames:10,rounding_inverse_frames:2,query_probes:70,strict_queries:58,rounding_queries:2,unvalidated_queries:10}}};
  await inspect(embed(report),d=>{
    const text=d.querySelector('#frame-summary').textContent;
    assert.match(text,/stale/i);
    assert.match(text,/both authored flag modes: 7 rigid preparations and 7 general preparations/);
    assert.match(text,/1 exact-zero and 1 ill-conditioned/);
    assert.match(text,/70 current-translation\/W=0 query probes/);
    assert.match(text,/2 rounding-bounded and 10 unvalidated/);
    assert.match(text,/provisional host reciprocal seed/);
    assert.match(text,/Live collected flags are not inferred/);
    assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
  });
  await inspect(embed(fixture()),d=>{
    assert.match(d.querySelector('#frame-summary').textContent,/has not been checked/);
  });
});

test('bounds evidence keeps incomplete component assemblies and strict tolerances visible',async()=>{
  const report=fixture();
  report.resources={state:'stale',details:{parsed:1,resources:1,local_images:0,decoded_mips:0,error_count:0,external_references:0,
    initial_bounds:{maps:2,base_bounds_probed:7,component_headers:4,ordinary_components_decoded:6,shape_components_decoded:2,shape_point_updates:5,empty_shape_aggregates:1,assembled_bounds_probed:5,strict_assembled_bounds:4,rounding_assembled_bounds:1,assembled_bounds_pending:2,unsupported_components:1,strict_bounds:6,rounding_bounds:1,unresolved_entity_references:2,unsupported_entity_types:3}}};
  await inspect(embed(report),d=>{
    const text=d.querySelector('#bounds-summary').textContent;
    assert.match(text,/stale/i);
    assert.match(text,/5 initial assemblies compared with independent f64 corner geometry/);
    assert.match(text,/2 assemblies remain pending/);
    assert.match(text,/2 slot-six aggregates decoded with 5 addressed point updates/);
    assert.match(text,/1 empty aggregates retain the native sentinel/);
    assert.match(text,/1 ordinary component types are unsupported/);
    assert.match(text,/Base-only comparisons: 6 strict and 1 rounding-bounded/);
    assert.match(text,/live candidate membership, mutable records, clearance and original-runtime parity remain unverified/);
    assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
  });
});

test('initialized collection keeps snapshot scope, ambiguity and contact limits explicit',async()=>{
  const report=fixture();
  report.resources={state:'stale',details:{parsed:1,resources:1,local_images:0,decoded_mips:0,error_count:0,external_references:0,
    candidate_hierarchies:{maps:2,zones:4,refreshed_hierarchies:4,nodes:11,branch_bounds_written:7,leaf_entries_refreshed:19,queries:9,stable_queries:7,ambiguous_queries:2,candidate_indices:17},
    static_geometry:{maps:2,queries:4,records_collected:13,maximum_records:6,sphere_rejections:5,bounds_rejections:2,full_rejections:1,frame_origins_prepared:13,inverse_preparations:3}}};
  await inspect(embed(report),d=>{
    const hierarchy=d.querySelector('#hierarchy-summary').textContent;
    assert.match(hierarchy,/stale/i);
    assert.match(hierarchy,/7 stable candidate-list agreements/);
    assert.match(hierarchy,/2 near-boundary cases/);
    assert.match(hierarchy,/Source membership flags stay unchanged/);
    const collection=d.querySelector('#collection-summary').textContent;
    assert.match(collection,/13 geometry records collected/);
    assert.match(collection,/1 full-buffer rejections/);
    assert.match(collection,/13 record query origins prepared with 3 consecutive-key inverse preparations/);
    assert.match(collection,/Records are not contacts/);
    assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
  });
});

for(const state of ['passed','failed','stale']) {
  test(`${state} world line diagnostics retain mode, zone-cap and reference limits`,async()=>{
    const report=fixture();
    report.resources={state,details:{world_line_requests:{maps:2,queries:9,nearest_queries:6,any_hit_queries:3,
      hits:5,misses:4,mode_comparisons:3,zones_selected:12,zone_capacity_reached:1,planes_prepared:23,planes_reused:7,
      triangles_tested:31,final_hits_validated:5,f64_plane_checks:3,f64_degenerate_triangles:1,f64_short_normal_triangles:1,
      maximum_f64_plane_distance:0.0000123}}};
    await inspect(embed(report),d=>{
      const text=d.querySelector('#world-line-summary').textContent;
      assert.match(text,new RegExp(state,'i'));
      assert.match(text,/9 authored segments/);
      assert.match(text,/6 nearest and 3 any-hit queries; 5 hits and 4 misses/);
      assert.match(text,/1 reached the native sixteen-zone stop/);
      assert.match(text,/5 final hits passed finite-value and source-provenance checks/);
      assert.match(text,/Distance is diagnostic, not a reference tolerance/);
      assert.match(text,/Any-hit results intentionally omit the world normal/);
      assert.match(text,/do not verify actor-line clearance/);
      assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
    });
  });
  test(`${state} world request diagnostics preserve cohort and live-state limits`,async()=>{
    const report=fixture();
    report.resources={state,details:{world_box_requests:{maps:2,sample_origins:3,single_queries:3,expanded_queries:3,
      zones_selected:9,duplicate_zones:2,records_collected:17,direct_single_contact_checks:3,expanded_record_sequence_checks:3,
      contacts_validated:11,contacts:11,record_capacity_rejections:1,contact_capacity_mesh_skips:2,contact_capacity_after_hit_full:3}}};
    await inspect(embed(report),d=>{
      const text=d.querySelector('#world-request-summary').textContent;
      assert.match(text,new RegExp(state,'i'));
      assert.match(text,/3 single-zone and 3 portal-expanded queries/);
      assert.match(text,/9 zone passes retain 2 duplicate occurrences/);
      assert.match(text,/11\/11 contacts passed finite-value and source-provenance checks/);
      assert.match(text,/1 record rejections, 2 mesh skips and 3 post-hit full events/);
      assert.match(text,/do not verify live manager state/);
      assert.match(text,/source capacities 64\/512 and filter slot 1\/flags 3/);
      assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
    });
  });
  test(`${state} static mesh contacts retain snapshot scope, capacity events and parity limits`,async()=>{
    const report=fixture();
    report.resources={state,details:{static_mesh_contacts:{maps:2,queries:5,records_collected:12,resident_mesh_loads:9,
      cache_generated:8,cache_reused:3,coarse_queries:4,coarse_rejections:1,coarse_maximum_fanout:7,
      batch_rejections:6,triangle_rejections:11,primitive_queries:23,contacts_appended:10,queries_with_contacts:3,
      contacts_validated:10,record_capacity_rejections:1,contact_capacity_mesh_skips:2,contact_capacity_after_hit_full:3}}};
    await inspect(embed(report),d=>{
      const text=d.querySelector('#mesh-contact-summary').textContent;
      assert.match(text,new RegExp(state,'i'));
      assert.match(text,/23 primitive queries produced 10 appended contacts across 3 requests/);
      assert.match(text,/maximum observed fanout 7/);
      assert.match(text,/1 record rejections, 2 mesh skips and 3 post-hit full events/);
      assert.match(text,/authored static snapshots/);
      assert.match(text,/original-runtime\/FP parity remain unverified/);
      assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
    });
  });
  test(`${state} evidence retains its label and never promotes runtime compatibility`,async()=>{
    const report=fixture();
    report.coverage={state,time_unix:1700000000,details:{totals:{lines:{percent:55.5,covered:111,count:200}}}};
    report.checks={state,details:{steps:[{passed:true},{passed:true},{passed:false}]}};
    report.compatibility[0].assets=state;
    report.compatibility[0].render=state==='passed'?'partial':state;
    await inspect(embed(report),d=>{
      assert.equal(d.querySelector('#coverage-number').textContent,'55.5%');
      assert.match(d.querySelector('#coverage-state').textContent,new RegExp(state,'i'));
      assert.equal(d.querySelector('#build-number').textContent,'2/4');
      assert.equal(d.querySelectorAll('#compatibility-rows .not_implemented').length,1);
      assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
    });
  });
}

test('search, domain and state filters compose and show a useful empty state',async()=>{
  await inspect(embed(fixture()),(d,w)=>{
    const set=(id,value)=>{const input=d.getElementById(id);input.value=value;input.dispatchEvent(new w.Event(id==='search'?'input':'change'));};
    set('search','GEOMETRY');
    assert.equal(d.querySelectorAll('#criteria tr').length,1);
    assert.match(d.querySelector('#criteria').textContent,/render.geometry/);
    set('domain','Rendering'); set('state','implemented');
    assert.match(d.querySelector('#criteria').textContent,/No matching criteria/);
    set('search',''); set('domain','');
    assert.match(d.querySelector('#criteria').textContent,/assets.read/);
    set('state','');
    assert.equal(d.querySelectorAll('#criteria tr').length,3);
  });
});

test('chart separates measured series and breaks coverage across stale observations',async()=>{
  const report=fixture();
  report.history=[
    {observed_unix:1700000000,parity_percent:0,implemented_percent:25,total:4,coverage_percent:50,coverage_state:'passed',coverage_measurement_version:2},
    {observed_unix:1700000010,parity_percent:0,implemented_percent:20,total:5,coverage_percent:50,coverage_state:'stale',coverage_measurement_version:2},
    {observed_unix:1700000020,parity_percent:0,implemented_percent:40,total:5,coverage_percent:60,coverage_state:'passed',coverage_measurement_version:2},
  ];
  await inspect(embed(report),d=>{
    assert.equal(d.querySelectorAll('circle.parity').length,3);
    assert.equal(d.querySelectorAll('circle.implementation').length,3);
    assert.equal(d.querySelectorAll('circle.coverage').length,2);
    assert.equal(d.querySelectorAll('polyline.coverage').length,0,'do not bridge missing/stale coverage');
    for(const dot of d.querySelectorAll('#history-chart circle')) {
      assert.ok(Number.isFinite(Number(dot.getAttribute('cx'))));
      assert.ok(Number.isFinite(Number(dot.getAttribute('cy'))));
      assert.ok(dot.querySelector('title').textContent.length>0);
    }
  });
});

test('production coverage v2 never plots or bridges legacy measurement snapshots',async()=>{
  const report=fixture();
  report.history=[
    {observed_unix:1700000000,parity_percent:0,implemented_percent:10,total:10,coverage_percent:99,coverage_state:'passed'},
    {observed_unix:1700000010,parity_percent:0,implemented_percent:20,total:10,coverage_percent:40,coverage_state:'passed',coverage_measurement_version:2},
    {observed_unix:1700000020,parity_percent:0,implemented_percent:30,total:10,coverage_percent:98,coverage_state:'passed',coverage_measurement_version:1},
    {observed_unix:1700000030,parity_percent:0,implemented_percent:40,total:10,coverage_percent:50,coverage_state:'passed',coverage_measurement_version:2},
    {observed_unix:1700000040,parity_percent:0,implemented_percent:50,total:10,coverage_percent:60,coverage_state:'passed',coverage_measurement_version:2},
    {observed_unix:1700000050,parity_percent:0,implemented_percent:60,total:10,coverage_percent:97,coverage_state:'passed',coverage_measurement_version:'2'},
  ];
  await inspect(embed(report),d=>{
    const dots=[...d.querySelectorAll('circle.coverage')];
    assert.equal(dots.length,3,'only numeric measurement version 2 is comparable');
    assert.deepEqual(dots.map(dot=>Number(dot.getAttribute('cx'))),[148,364,472]);
    assert.equal(d.querySelectorAll('polyline.coverage').length,1);
    assert.equal(d.querySelector('polyline.coverage').getAttribute('points'),'364,107.5 472,93');
    assert.equal(d.querySelectorAll('circle.parity').length,6,'legacy observations remain available for other series');
    assert.equal(d.querySelectorAll('circle.implementation').length,6);
    assert.match(d.querySelector('#coverage-method-note').textContent,/production measurement v2/);
    assert.match(d.querySelector('#coverage-method-note').textContent,/different test filter/);
  });
});

test('one history point stays centered with no invented trend',async()=>{
  const report=fixture();
  report.history=[{observed_unix:1700000000,parity_percent:0,total:3}];
  await inspect(embed(report),d=>{
    assert.equal(d.querySelector('circle.parity').getAttribute('cx'),'310');
    assert.equal(d.querySelectorAll('#history-chart polyline').length,0);
  });
});

test('roadmap text is displayed literally without interpreting HTML',async()=>{
  const report=fixture();
  const text='<img src=x onerror="window.injected=true">';
  report.criteria[0].title=text; report.next=text;
  await inspect(embed(report),(d,w)=>{
    assert.match(d.querySelector('#criteria').textContent,/<img src=x/);
    assert.equal(d.querySelector('#next-work').textContent,text);
    assert.equal(d.querySelectorAll('#criteria img,#next-work img').length,0);
    assert.equal(w.injected,undefined);
  });
});


function navigationOwnerFixture() {
  return {origin:'fresh_owner_snapshot',initialized:true,source_placements:5,eligible_placements:2,
    owned_wrappers:2,bound_wrappers:2,initialized_caches:2,vertices:9,faces:7,
    category_slots:30,nonempty_categories:2,category_members:3,boundary_pairs:8,
    bindings_sha256:'1'.repeat(64),categories_sha256:'2'.repeat(64),cache_sha256:'3'.repeat(64),
    state_sha256:'4'.repeat(64),original_history_verified:false,native_precision_verified:false,
    queries_executed:false,scope:'Authored navigation owners, integer cache and category membership'};
}

function withNavigationOwners(report) {
  withCompleteCatalog(report);
  const data=report.menu.details.game_session;
  for(const probe of [...data.difficulty_probes,data.cancel_reentry]) {
    probe.window.game_session.active.world_initialization.navigation=navigationOwnerFixture();
  }
  return report;
}

test('navigation shows actual owned bindings categories and caches without promoting gameplay readiness',async()=>{
  await inspect(embed(withNavigationOwners(gameSessionFixture())),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/Navigation initialized 2 owned wrappers and 2 bound caches from 2 eligible placements/);
    assert.match(text,/9 local vertices, 7 faces and 8 ordered boundary pairs/);
    assert.match(text,/3 face memberships occupy 2 of 30 category vectors/);
    assert.match(text,/No navigation queries ran; original owner history and native precision remain unverified/);
    assert.match(text,/world registration, player creation and camera output remain incomplete/);
    assert.equal(d.querySelector('#menu-number').textContent,'100%');
  });
});

test('navigation metadata fails closed for unsafe payloads count contradictions and readiness claims',async()=>{
  const changes=[
    nav=>{delete nav.cache_sha256;},nav=>{nav.pose=[[1,2,3]];},nav=>{nav.vertices=-1;},
    nav=>{nav.source_placements=1;},nav=>{nav.owned_wrappers=3;},nav=>{nav.bound_wrappers=1;},
    nav=>{nav.initialized_caches=1;},nav=>{nav.category_slots=29;},nav=>{nav.category_members=8;},
    nav=>{nav.nonempty_categories=0;},nav=>{nav.boundary_pairs=22;},nav=>{nav.initialized='true';},
    nav=>{nav.origin='original_runtime';},nav=>{nav.bindings_sha256='A'.repeat(64);},
    nav=>{nav.original_history_verified=true;},nav=>{nav.native_precision_verified=true;},
    nav=>{nav.queries_executed=true;},nav=>{nav.scope='';},
  ];
  for(const change of changes) {
    const report=withNavigationOwners(gameSessionFixture());
    change(report.menu.details.game_session.difficulty_probes[0].window.game_session.active.world_initialization.navigation);
    await inspect(embed(report),d=>{
      const text=d.querySelector('#game-session-summary').textContent;
      assert.match(text,/has not been recorded completely/);
      assert.doesNotMatch(text,/Navigation initialized|all 4 difficulties admitted/);
    });
  }
  for(const field of ['world_registered','player_created','camera_ready']) {
    const report=withNavigationOwners(gameSessionFixture());
    report.menu.details.game_session.difficulty_probes[0].window.game_session.active[field]=true;
    await inspect(embed(report),d=>{
      assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
    });
  }
});

test('navigation distinguishes absent partial empty and inconsistent owner initialization',async()=>{
  await inspect(embed(withWorldInitialization(gameSessionFixture())),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/Navigation owner\/cache initialization evidence is absent/);
  });
  const partial=withNavigationOwners(gameSessionFixture());
  for(const probe of [...partial.menu.details.game_session.difficulty_probes,partial.menu.details.game_session.cancel_reentry]) {
    const nav=probe.window.game_session.active.world_initialization.navigation;
    nav.initialized=false;nav.bound_wrappers=1;nav.initialized_caches=0;
  }
  await inspect(embed(partial),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/Navigation initialization remains incomplete with 2 owned wrappers and 0 bound caches/);
    assert.doesNotMatch(text,/Navigation initialized/);
  });
  const empty=withNavigationOwners(gameSessionFixture());
  for(const probe of [...empty.menu.details.game_session.difficulty_probes,empty.menu.details.game_session.cancel_reentry]) {
    const nav=probe.window.game_session.active.world_initialization.navigation;
    for(const key of ['source_placements','eligible_placements','owned_wrappers','bound_wrappers','initialized_caches','vertices','faces','category_slots','nonempty_categories','category_members','boundary_pairs']) nav[key]=0;
  }
  await inspect(embed(empty),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/Navigation initialized 0 owned wrappers and 0 bound caches from 0 eligible placements/);
  });
  for(const mutate of [
    world=>{delete world.navigation;},world=>{world.navigation=null;},
    world=>{world.navigation.initialized=false;},
  ]) {
    const inconsistent=withNavigationOwners(gameSessionFixture());
    mutate(inconsistent.menu.details.game_session.difficulty_probes[0].window.game_session.active.world_initialization);
    await inspect(embed(inconsistent),d=>{
      assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
    });
  }
});


function postTriggerResourceFixture() {
  return {origin:'source_hook_snapshot',provider:'synchronous_supplied_data_residency',
    operation_complete:true,selection_performed:true,selected_definition_bound:true,
    ordered_requests:2,resolved_requests:2,retained_files:1,retained_sections:1,
    retained_file_bytes:960,retained_section_bytes:240,tail_reset_applied:true,
    requests_sha256:'5'.repeat(64),residency_sha256:'6'.repeat(64),state_sha256:'7'.repeat(64),
    original_cursor_history_verified:false,original_rng_history_verified:false,
    original_stream_history_verified:false,native_stream_timing_verified:false,
    native_stream_readiness_verified:false,scope:'Authored synchronous ordered requests and shared residency'};
}

function withPostTriggerResources(report) {
  withNavigationOwners(report);
  const data=report.menu.details.game_session;
  for(const probe of [...data.difficulty_probes,data.cancel_reentry]) {
    const world=probe.window.game_session.active.world_initialization;
    world.post_trigger_resources=postTriggerResourceFixture();
    world.actor_definitions.selection_calls=3;
    world.actor_definitions.state_sha256='8'.repeat(64);
  }
  return report;
}

test('post-trigger residency shows ordered calls shared ownership and unverified native readiness',async()=>{
  await inspect(embed(withPostTriggerResources(gameSessionFixture())),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/Post-trigger supplied-data residency completed: 2 ordered requests, 2 resolved; 1 retained files \(960 payload bytes\) and 1 streamed sections \(240 covered section bytes\)/);
    assert.match(text,/The selected definition remains owned. The source tail reset was applied/);
    assert.match(text,/3 source selections and 1 two-step LCG draws/);
    assert.match(text,/Original cursor, RNG and stream histories, native stream timing and native stream readiness remain unverified/);
    assert.match(text,/world registration, player creation and camera output remain incomplete/);
    assert.equal(d.querySelector('#menu-number').textContent,'100%');
  });
});

test('post-trigger receipts reject closed-schema ownership completion and native-history contradictions',async()=>{
  const changes=[
    receipt=>{delete receipt.retained_sections;},receipt=>{receipt.section_payload=[1,2,3];},
    receipt=>{receipt.ordered_requests=3;},receipt=>{receipt.resolved_requests=1;},
    receipt=>{receipt.retained_files=3;},receipt=>{receipt.retained_sections=3;},receipt=>{receipt.retained_sections=0;},
    receipt=>{receipt.retained_file_bytes=-1;},receipt=>{receipt.retained_section_bytes=961;},
    receipt=>{receipt.operation_complete='true';},receipt=>{receipt.selection_performed=false;},
    receipt=>{receipt.selected_definition_bound=false;},receipt=>{receipt.tail_reset_applied=false;},
    receipt=>{receipt.origin='original_runtime';},receipt=>{receipt.provider='native_streaming';},
    receipt=>{receipt.requests_sha256='Z'.repeat(64);},receipt=>{receipt.residency_sha256=null;},
    receipt=>{receipt.original_cursor_history_verified=true;},receipt=>{receipt.original_rng_history_verified=true;},
    receipt=>{receipt.original_stream_history_verified=true;},receipt=>{receipt.native_stream_timing_verified=true;},
    receipt=>{receipt.native_stream_readiness_verified=true;},receipt=>{receipt.scope='';},
  ];
  for(const change of changes) {
    const report=withPostTriggerResources(gameSessionFixture());
    change(report.menu.details.game_session.difficulty_probes[0].window.game_session.active.world_initialization.post_trigger_resources);
    await inspect(embed(report),d=>{
      const text=d.querySelector('#game-session-summary').textContent;
      assert.match(text,/has not been recorded completely/);
      assert.doesNotMatch(text,/Post-trigger supplied-data residency completed|all 4 difficulties admitted/);
    });
  }
  for(const field of ['world_registered','player_created','camera_ready']) {
    const report=withPostTriggerResources(gameSessionFixture());
    report.menu.details.game_session.difficulty_probes[0].window.game_session.active[field]=true;
    await inspect(embed(report),d=>{
      assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
    });
  }
});

test('post-trigger misses base-only targets and source skips remain distinct and conservative',async()=>{
  await inspect(embed(withNavigationOwners(gameSessionFixture())),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/Post-trigger resource residency evidence is absent/);
  });
  const variants=[
    {operation_complete:false,resolved_requests:0,retained_sections:0,retained_section_bytes:0,tail_reset_applied:false},
    {retained_sections:0,retained_section_bytes:0},
    {selection_performed:false,selected_definition_bound:false,ordered_requests:0,resolved_requests:0,retained_files:0,retained_sections:0,retained_file_bytes:0,retained_section_bytes:0},
    {retained_section_bytes:0},
  ];
  for(const [index,change] of variants.entries()) {
    const report=withPostTriggerResources(gameSessionFixture());
    for(const probe of [...report.menu.details.game_session.difficulty_probes,report.menu.details.game_session.cancel_reentry]) {
      Object.assign(probe.window.game_session.active.world_initialization.post_trigger_resources,change);
    }
    await inspect(embed(report),d=>{
      const text=d.querySelector('#game-session-summary').textContent;
      if(index===0) {
        assert.match(text,/residency remains incomplete: 2 ordered requests, 0 resolved; 1 retained files/);
        assert.match(text,/source tail reset remains pending/);
        assert.doesNotMatch(text,/Source gates skipped/);
      } else if(index===1) {
        assert.match(text,/residency completed: 2 ordered requests, 2 resolved; 1 retained files/);
        assert.match(text,/0 streamed sections \(0 covered section bytes\)/);
      } else if(index===2) {
        assert.match(text,/residency completed: 0 ordered requests, 0 resolved; 0 retained files/);
        assert.match(text,/Source gates skipped selection and resource requests/);
      } else {
        assert.match(text,/residency completed: 2 ordered requests, 2 resolved; 1 retained files/);
        assert.match(text,/1 streamed sections \(0 covered section bytes\)/);
      }
      assert.match(text,/world registration, player creation and camera output remain incomplete/);
    });
  }
  for(const change of [
    world=>{delete world.post_trigger_resources;},world=>{world.post_trigger_resources=null;},
    world=>{world.post_trigger_resources.operation_complete=false;},
  ]) {
    const report=withPostTriggerResources(gameSessionFixture());
    change(report.menu.details.game_session.difficulty_probes[0].window.game_session.active.world_initialization);
    await inspect(embed(report),d=>{
      assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
    });
  }
});


function worldHooksFixture() {
  return {origin:'fresh_constructor_snapshot',lifetime:'native_window_controller',clock_provider:'host_frame_elapsed_f32',
    operation_complete:true,clock_reset_complete:true,clock_reads:16,constructor_clock_reads:16,
    completed_clock_resets:4,clock_frame_updates:9,clock_seconds_bits:0x3fa00000,clock_timescale_bits:0x3f800000,
    clock_snapshot_bound:true,scoped_buffers:24,buffers_cleared:22,buffers_preserved:2,
    backings_released:2,bytes_released:48,retained_backing_bytes:32,leaf_reset_complete:true,
    literal_noops_reached:2,binding_released:true,callback_installed:true,callback_invocations:0,
    clock_state_sha256:'8'.repeat(64),final_state_sha256:'9'.repeat(64),state_sha256:'a'.repeat(64),
    original_history_verified:false,native_clock_precision_verified:false,native_timing_verified:false,
    world_ready_emitted:false,scope:'Authored native-window clock snapshots and scoped owned resets'};
}

function withWorldHooks(report) {
  withPostTriggerResources(report);
  const data=report.menu.details.game_session;
  for(const [index,probe] of [...data.difficulty_probes,data.cancel_reentry].entries()) {
    const hooks=worldHooksFixture();
    // Distinct real attempts may see different elapsed time/reset counters and
    // all three valid digests, including the cancellation/re-entry attempt.
    hooks.clock_seconds_bits+=index*0x10000;
    hooks.clock_frame_updates+=index*7;
    hooks.completed_clock_resets+=index;
    hooks.clock_state_sha256=(index+1).toString(16).repeat(64);
    hooks.final_state_sha256=(index+6).toString(16).repeat(64);
    hooks.state_sha256=(index+10).toString(16).repeat(64);
    probe.window.game_session.active.world_initialization.world_hooks=hooks;
  }
  return report;
}

test('world hooks retain controller clock history across probes and cancel reentry without world readiness',async()=>{
  await inspect(embed(withWorldHooks(gameSessionFixture())),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/World hooks completed their scoped reset work: 24 buffers, 22 cleared and 2 preserved/);
    assert.match(text,/2 backings released \(48 bytes\), 32 backing bytes retained/);
    assert.match(text,/first probe's live host frame clock recorded 1.25 seconds over 9 updates/);
    assert.match(text,/16 constructor reads and 16 reset reads belong to 4 completed resets in the native-window controller lifetime/);
    assert.match(text,/Callback installed; 0 invocations. This reset segment emitted no world-ready event/);
    assert.match(text,/original history, native clock precision and native timing remain unverified/);
    assert.match(text,/world registration, player creation and camera output remain incomplete/);
  });
});

test('world hook receipts fail closed for clock provider precision ownership and private payload contradictions',async()=>{
  const changes=[
    hooks=>{delete hooks.constructor_clock_reads;},hooks=>{hooks.buffer_payload=[1,2,3];},
    hooks=>{hooks.clock_provider='mission_zero';},hooks=>{hooks.lifetime='mission_worker';},
    hooks=>{hooks.clock_reads=15;},hooks=>{hooks.constructor_clock_reads=0;},
    hooks=>{hooks.completed_clock_resets=1;},hooks=>{hooks.clock_snapshot_bound=false;},
    hooks=>{hooks.clock_reset_complete=false;},hooks=>{hooks.clock_seconds_bits=0x7fc00000;},
    hooks=>{hooks.clock_seconds_bits=0x7f800000;},hooks=>{hooks.clock_seconds_bits=0xbf800000;},
    hooks=>{hooks.clock_seconds_bits=0x100000000;},hooks=>{hooks.clock_timescale_bits=0;},
    hooks=>{hooks.buffers_cleared=21;},hooks=>{hooks.buffers_preserved=3;},hooks=>{hooks.scoped_buffers=23;},
    hooks=>{hooks.backings_released=23;},hooks=>{hooks.backings_released=0;},
    hooks=>{hooks.leaf_reset_complete=false;},hooks=>{hooks.literal_noops_reached=1;},
    hooks=>{hooks.callback_installed=false;},hooks=>{hooks.callback_invocations=1;},
    hooks=>{hooks.clock_state_sha256='X'.repeat(64);},hooks=>{hooks.final_state_sha256=null;},
    hooks=>{hooks.state_sha256='Q'.repeat(64);},hooks=>{hooks.original_history_verified=true;},
    hooks=>{hooks.native_clock_precision_verified=true;},hooks=>{hooks.native_timing_verified=true;},
    hooks=>{hooks.world_ready_emitted=true;},hooks=>{hooks.operation_complete='true';},
  ];
  for(const change of changes) {
    const report=withWorldHooks(gameSessionFixture());
    // Mutate a different attempt and the re-entry attempt too: validating only
    // the first snapshot would miss a bad live clock/digest after cancellation.
    const index=changes.indexOf(change)%2===0 ? 2 : 4;
    const probes=[...report.menu.details.game_session.difficulty_probes,report.menu.details.game_session.cancel_reentry];
    change(probes[index].window.game_session.active.world_initialization.world_hooks);
    await inspect(embed(report),d=>{
      const text=d.querySelector('#game-session-summary').textContent;
      assert.match(text,/has not been recorded completely/);
      assert.doesNotMatch(text,/World hooks completed|all 4 difficulties admitted/);
    });
  }
  for(const field of ['world_registered','player_created','camera_ready']) {
    const report=withWorldHooks(gameSessionFixture());
    report.menu.details.game_session.cancel_reentry.window.game_session.active[field]=true;
    await inspect(embed(report),d=>{
      assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
    });
  }
});

test('world hook absence partial reset and zero elapsed snapshots stay distinct',async()=>{
  await inspect(embed(withPostTriggerResources(gameSessionFixture())),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/World-hook lifecycle evidence is absent/);
  });
  const partial=withWorldHooks(gameSessionFixture());
  for(const probe of [...partial.menu.details.game_session.difficulty_probes,partial.menu.details.game_session.cancel_reentry]) {
    Object.assign(probe.window.game_session.active.world_initialization.world_hooks,{
      operation_complete:false,clock_reset_complete:false,clock_snapshot_bound:false,
      clock_reads:3,completed_clock_resets:1,leaf_reset_complete:false,callback_installed:false});
  }
  await inspect(embed(partial),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/World hooks remain incomplete/);
    assert.doesNotMatch(d.querySelector('#game-session-summary').textContent,/World hooks completed/);
  });
  const zero=withWorldHooks(gameSessionFixture());
  zero.menu.details.game_session.difficulty_probes[0].window.game_session.active.world_initialization.world_hooks.clock_seconds_bits=0;
  await inspect(embed(zero),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/clock recorded 0 seconds over 9 updates/);
  });
  for(const change of [world=>{delete world.world_hooks;},world=>{world.world_hooks=null;}]) {
    const report=withWorldHooks(gameSessionFixture());
    change(report.menu.details.game_session.cancel_reentry.window.game_session.active.world_initialization);
    await inspect(embed(report),d=>{
      assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
    });
  }
});


function loaderFixture() {
  return {origin:'fresh_constructor_snapshot',lifetime:'native_window_controller',provider:'scoped_supplied_audio_and_base_map',
    operation_complete:true,entry_phase:7,current_phase:21,pending_phase:null,pre_hooks_complete:true,
    maps_constructed:3,map_zones:9,map_component_slots:1,map_context_bindings:3,map_sections:11,
    map_sections_published:3,map_zones_published:2,map_bindings:2,
    checkpoint_bound:true,checkpoint_restore_active:false,checkpoint_payload_initialized:false,checkpoint_payload_bytes:0x37f8,checkpoint_skips:2,
    source_bank_calls:2,metadata_jobs_submitted:1,backend_commands:2,backend_sequence:0,
    retained_banks:2,retained_bank_bytes:960,sound_entries:7,wave_entries:9,
    sound_info_bound:true,sound_info_rows:8,sound_info_bytes:128,work_completed:6,work_pending:0,work_failed:0,
    barrier_promotions:12,resource_polls:12,work_busy_polls:3,phase8_byte:10,phase8_resets:4,
    loader_c4_bits:0x44bb8000,loader_c0_bits:0x3a83126f,catalog_phase18_entries:2,
    readiness_events:2,readiness_local_stores:4,readiness_owner_flags:0x108,readiness_peer_flags:0x108,
    readiness_user_flags:0x108,readiness_local_flags:0x109,world_ready_emitted:true,player_created:false,
    original_history_verified:false,native_sdk_dispatch_verified:false,native_work_timing_verified:false,banks_playback_verified:false,
    pre_state_sha256:'1'.repeat(64),maps_state_sha256:'2'.repeat(64),banks_state_sha256:'3'.repeat(64),
    phase_state_sha256:'4'.repeat(64),state_sha256:'5'.repeat(64),scope:'Authored actual provider/catalog/session loader checkpoint'};
}

function withCompletedLoader(report) {
  withWorldHooks(report);
  const data=report.menu.details.game_session;
  for(const [index,probe] of [...data.difficulty_probes,data.cancel_reentry].entries()) {
    const snapshot=probe.window.game_session.active,loader=loaderFixture();
    loader.maps_constructed=snapshot.map_instances;
    loader.source_bank_calls=1+index%3;loader.backend_commands=loader.source_bank_calls+(index===4?2:0);
    loader.metadata_jobs_submitted=index===0?1:0;loader.backend_sequence=index===0?0:0xfffffffa+index;
    loader.work_completed=loader.backend_commands+3+index;loader.work_busy_polls=loader.backend_commands+index;
    loader.phase8_resets+=index;
    for(const [offset,key] of ['pre_state_sha256','maps_state_sha256','banks_state_sha256','phase_state_sha256','state_sha256'].entries()) loader[key]=(index+offset+1).toString(16).repeat(64);
    snapshot.world_initialization.loader=loader;
    snapshot.stage='awaiting_player';snapshot.world_registered=true;
    snapshot.manager_ready_flags_raw=0x108;snapshot.user_ready_flags_raw=0x108;
  }
  return report;
}

test('completed loader shows source world readiness and partial base publication while awaiting a player',async()=>{
  await inspect(embed(withCompletedLoader(gameSessionFixture())),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/World initialization checkpoint preserved 35 original ordinal slots/);
    assert.match(text,/Loader completed the world-initialization checkpoint: 12 provider barriers promoted/);
    assert.match(text,/Fresh base publication exposes 3 of 11 logical sections and 2 of 9 zones, with 2 map bindings/);
    assert.match(text,/1 source bank calls and 1 backend commands retain 2 banks \(960 bytes\), 7 sound entries and 9 waves; SoundInfo owns 8 rows \(128 bytes\)/);
    assert.match(text,/2 identity-bound readiness events and 4 local mirror stores reached the admitted session/);
    assert.match(text,/World registration and matching-user readiness completed the loader checkpoint. Sessions await player creation/);
    assert.match(text,/player creation and camera output remain incomplete/);
    assert.match(text,/This reset segment emitted no world-ready event/);
    assert.match(text,/Original history, native SDK dispatch, native work timing and bank playback remain unverified/);
    assert.doesNotMatch(text,/manager and user readiness flags remain 0/);
  });
});

test('loader rejects closed-schema early phase readiness publication and privacy contradictions',async()=>{
  const changes=[
    loader=>{delete loader.work_completed;},loader=>{loader.bank_payload=[1,2,3];},
    loader=>{loader.current_phase=19;},loader=>{loader.pending_phase=21;},loader=>{loader.pre_hooks_complete=false;},
    loader=>{loader.map_sections_published=12;},loader=>{loader.map_zones_published=10;},loader=>{loader.map_context_bindings=5;},
    loader=>{loader.checkpoint_restore_active=true;},loader=>{loader.checkpoint_payload_bytes=0;},
    loader=>{loader.source_bank_calls=4;},loader=>{loader.metadata_jobs_submitted=2;},loader=>{loader.backend_sequence=0x100000000;},
    loader=>{loader.sound_info_bound=false;},loader=>{loader.work_pending=1;},loader=>{loader.work_failed=1;},
    loader=>{loader.work_completed=loader.backend_commands;},loader=>{loader.resource_polls=11;},
    loader=>{loader.phase8_resets=1;},loader=>{loader.loader_c4_bits=0;},loader=>{loader.catalog_phase18_entries=1;},
    loader=>{loader.readiness_events=1;},loader=>{loader.readiness_local_stores=2;},loader=>{loader.readiness_peer_flags=0;},
    loader=>{loader.world_ready_emitted=false;},loader=>{loader.player_created=true;},
    loader=>{loader.original_history_verified=true;},loader=>{loader.native_sdk_dispatch_verified=true;},
    loader=>{loader.native_work_timing_verified=true;},loader=>{loader.banks_playback_verified=true;},
    loader=>{loader.maps_state_sha256='X'.repeat(64);},
  ];
  for(const change of changes) {
    const report=withCompletedLoader(gameSessionFixture());
    change(report.menu.details.game_session.cancel_reentry.window.game_session.active.world_initialization.loader);
    await inspect(embed(report),d=>{
      const text=d.querySelector('#game-session-summary').textContent;
      assert.match(text,/has not been recorded completely/);
      assert.doesNotMatch(text,/Loader completed|all 4 difficulties admitted/);
    });
  }
  for(const [field,value] of [['stage','awaiting_world'],['world_registered',false],['manager_ready_flags_raw',0],['player_created',true],['camera_ready',true]]) {
    const report=withCompletedLoader(gameSessionFixture());
    report.menu.details.game_session.cancel_reentry.window.game_session.active[field]=value;
    await inspect(embed(report),d=>{
      assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
    });
  }
});

test('incomplete loader Wait28 retains its pending phase and legacy zero session flags',async()=>{
  const report=withCompletedLoader(gameSessionFixture());
  for(const probe of [...report.menu.details.game_session.difficulty_probes,report.menu.details.game_session.cancel_reentry]) {
    const snapshot=probe.window.game_session.active,loader=snapshot.world_initialization.loader;
    snapshot.stage='awaiting_world';snapshot.world_registered=false;snapshot.manager_ready_flags_raw=0;snapshot.user_ready_flags_raw=0;
    Object.assign(loader,{operation_complete:false,world_ready_emitted:false,current_phase:28,pending_phase:19,
      work_pending:1,barrier_promotions:11,resource_polls:11,readiness_events:0,readiness_local_stores:0,
      readiness_owner_flags:0,readiness_peer_flags:0,readiness_user_flags:0,readiness_local_flags:1});
  }
  await inspect(embed(report),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/Loader remains incomplete: 11 provider barriers promoted/);
    assert.match(text,/manager and user readiness flags remain 0/);
    assert.doesNotMatch(text,/matching-user readiness completed/);
  });
  report.menu.details.game_session.cancel_reentry.window.game_session.active.world_initialization.loader.pending_phase=null;
  await inspect(embed(report),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
  });
});


function playerPreludeFixture(catalogEntries=2) {
  return {origin:'fresh_constructor_snapshot',lifetime:'controller_arena',stage:'player_components_pending',
    pending_dependency:'tp_animation_command',operation_complete:false,attempt_bound:true,catalog_bound:true,
    operation_entries:0,creation_dispatches:1,catalog_entries:catalogEntries,selected_catalog_index:1,selected_source_ordinal:4,
    requested_start_point_before:1,requested_start_point_after:1,base_constructor_complete:true,
    handler_rng_reads:1,base_reset_calls:2,persistent_base_resets:2,handler_kind_bound:false,
    inventory_records:1,inventory_capacity:16,inventory_growth:16,world_items:1,world_actor_lists:1,
    world_membership_lists:10,actor_registrations:1,physics_bound:true,completion_published:false,
    player_created:false,camera_ready:false,original_history_verified:false,native_timing_verified:false,
    base_state_sha256:'1'.repeat(64),prelude_state_sha256:'2'.repeat(64),world_state_sha256:'3'.repeat(64),
    persistent_state_sha256:'4'.repeat(64),state_sha256:'5'.repeat(64),
    shared_count:1,shared_live_leases:1,shared_allocations:3,shared_native_bytes:63680,shared_host_capacity_bytes:64000,
    shared_reset_calls:1,shared_entry_noops:98,shared_cache_stores:1,shared_reset_complete:true,
    runtime_base_reset_complete:true,derived_reset_prefix_complete:true,
    shared_state_sha256:'6'.repeat(64),reset_state_sha256:'7'.repeat(64),
    scratch_prepared:true,character_bound:true,inventory_selection_complete:true,
    scratch_capacity:32,scratch_growth:16,scratch_records:0,scratch_native_bytes:4736,scratch_host_capacity_bytes:4800,scratch_reallocations:1,
    character_resource:0x01090013,character_key:0x4d0000a7,character_source_index:8,guns_resource:0x0109000f,guns_payload_bytes:1234,
    metadata_source_index:4,metadata_sorted_index:5,weapon_registry_keys:9,primary_order_keys:8,secondary_order_keys:3,
    inventory_primary_order_index:6,inventory_interface_callbacks:0,inventory_selected_record:0,
    scratch_state_sha256:'8'.repeat(64),loadout_state_sha256:'9'.repeat(64),
    constructor_flag:{origin:'captured_member_snapshot',raw:0,changed:false,interface_callbacks:0,original_history_verified:false},
    virtual_initialization:virtualPlayerFixture(),
    component_initialization:componentPlayerFixture(),
    scope:'Authored retained EXItem actor and capsule; basic return with unresolved scalar; PlayerNew remains incomplete'};
}
function tpCapturePlayerFixture() {
  return {physical_slots:4,controls:4,bones_per_control:2,auxiliary_words_per_control:4,native_header_bytes:0x54,
    native_backing_bytes_per_control:80,active_slots:1,capture_mask:15,preserve_mask:3,update_flags_after:3,
    frame_stamp_known:false,postprocess_executed:false,fallback:0x090006c0,fallback_writes:2,
    refresh:{...refreshPlayerFixture(),refresh_ordinal:3,allocation_ordinal:3},state_sha256:'c'.repeat(64)};
}
function tpChildPlayerFixture() {
  return {native_header_bytes:0x90,resource_reused:true,context_bound:true,generation_observed:0,cache_generation:0,
    metadata_cached:true,descriptor_index:2,body_bound:true,phase_zero_initialized:false,initializer_returned:false,
    factory_returned:false,layer_inserted:false,state_sha256:'d'.repeat(64)};
}
function tpCommandPlayerFixture() {
  return {stage:'child_initializer_pending',opcode:0x0c000002,set_reference:0x0a0000ab,set_index:0,set_groups:1,
    groups_visited:1,weight_writes:1,pending_group:0,pending_slot:1,child_reference:0x040000ab,factory_class:4,
    child_flags:32,child_divisor:3,mask:null,children_created:1,child_header:tpChildPlayerFixture(),command_returned:false,cleanup_complete:false,
    selector_acknowledged:false,state_sha256:'a'.repeat(64),layers_state_sha256:'b'.repeat(64)};
}
function tpSelectorPlayerFixture() {
  return {stage:'command_pending',requested:0x090000ac,previous:0,fallback:0x090006c0,
    tracks_player_modes:false,target_index:1,attempts:1,lookups:2,transition_rows:1,
    commands_issued:1,commands_executed:0,pending_ordinal:0,pending_opcode:0x0c000002,
    return_word:null,base_only:true,command_effects_complete:false,state_sha256:'d'.repeat(64)};
}
function initialActionPlayerFixture() {
  return {stage:'tp_command_pending',generic_request:0x0900071e,parameter_bits:0,direction_raw:255,
    weapon_key:0x020000ab,variant:0,remap_calls:3,map_rows_read:20,fp_mapped:0x090000ab,tp_mapped:0x090000ac,
    tp_secondary_mapped:0x090000ad,source_tables_reused:true,mode_controls_created:4,mode_history_updated:false,
    callback_returned:false,state_sha256:'a'.repeat(64),mapping_state_sha256:'b'.repeat(64),tp_capture:tpCapturePlayerFixture(),tp_selector:tpSelectorPlayerFixture(),tp_command:tpCommandPlayerFixture()};
}
function firstPersonPlayerFixture() {
  return {helper_returned:true,identity_registered:true,item_list_ordinal:1,ordered_links_bound:true,
    masks_configured:true,table_shared:true,pose_target_assigned:true,visual_postbind_skipped:true,
    skeleton:skeletonPlayerFixture(),bind_refresh:{...refreshPlayerFixture(),refresh_ordinal:2,allocation_ordinal:2},
    state_sha256:'f'.repeat(64)};
}
function callbacksPlayerFixture() {
  return {interface_bound:true,table_shared:true,callback_slots:4,noop_slots:2,
    pose_target_assigned:true,pose_postprocess_implemented:false,tp_helper_returned:true,
    events_executed:false,state_sha256:'e'.repeat(64)};
}
function postbindPlayerFixture() {
  return {wrapper_reused:true,secondary_skin_bound:true,skin_requests:1,texture_requests:3,texture_targets_resident:3,
    override_generation:1,override_capacity:0,override_growth:16,override_count:0,item_collection_bound:true,
    configured_headers:4,callback_bindings_complete:true,state_sha256:'d'.repeat(64)};
}
function attachmentPlayerFixture() {
  return {identity_registered:true,item_backlink_bound:true,item_list_member:true,item_list_ordinal:0,
    item_components:2,registry_identities:2,component_flag_8e:0,head_link_null:true,next_link_bound:true,state_sha256:'c'.repeat(64)};
}
function refreshPlayerFixture() {
  return {arena_bytes:0x4000,reserved_bytes:0x2200,reservation_start:0,cursor_after:0,
    live_reservations_after:0,refresh_ordinal:1,allocation_ordinal:1,regions_bound:4,
    flags_after:0,scope_released:true,state_sha256:'b'.repeat(64)};
}
function skeletonPlayerFixture() {
  return {native_runtime_bytes:0x164,native_storage_bytes:288,bones:2,roots:1,auxiliary_words:4,
    primary_bound:true,secondary_bound:true,animator_backlink_bound:true,component_resource_bound:true,
    frame_stamp_known:false,empty_refresh_complete:true,retained_skeleton_file_bytes:1024,
    retained_component_file_bytes:2048,state_sha256:'a'.repeat(64)};
}
function componentPlayerFixture() {
  return {stage:'tp_command_pending',pending_dependency:'tp_animation_command',
    native_component_bytes:0x1b8,constructed_slots:4,active_slots:1,retained_file_bytes:1024,retained_section_bytes:256,
    third_person_constructed:true,third_person_attached:true,first_person_constructed:true,skin_request_resolved:true,
    texture_dependencies_enumerated:true,component_binding_complete:true,
    texture_directory_outcome:'enumerated',texture_requests:3,texture_targets_resident:3,
    texture_residency_complete:true,texture_requests_state_sha256:'f'.repeat(64),preparation_state_sha256:'d'.repeat(64),
    target_state_sha256:'e'.repeat(64),skeleton_binding:skeletonPlayerFixture(),bind_refresh:refreshPlayerFixture(),attachment:attachmentPlayerFixture(),visual_postbind:postbindPlayerFixture(),callbacks:callbacksPlayerFixture(),first_person:firstPersonPlayerFixture(),initial_action:initialActionPlayerFixture(),scope:'Authored initial action; TP control dispatch pending'};
}
function virtualPlayerFixture() {
  return {stage:'player_basic_complete',pending_dependency:'tp_fp_components',
    item_registry_records:1,item_registry_capacity:16,item_registry_growth:16,item_registry_counter:1,
    item_registry_native_bytes:128,item_registry_host_capacity_bytes:512,origin_zone_queries:1,origin_zone_nodes:2,
    desired_membership:0x2d,item_membership:0x3d,item_flags:0xab,item_priority:255,
    factory_complete:true,item_registry_bound:true,actor_registered:true,handler_bound:true,origin_zone_queried:true,
    physics_constructed:true,physics_backlink_bound:true,physics_activated:true,initializer_complete:true,scalar_pending:true,scalar_value_known:false,
    item_state_sha256:'a'.repeat(64),physics_state_sha256:'b'.repeat(64),registry_state_sha256:'c'.repeat(64),
    scope:'Authored basic return and activation with deferred scalar; completed player remains unavailable'};
}
function withPlayerPrelude(report) {
  withCompletedLoader(report);
  for(const probe of [...report.menu.details.game_session.difficulty_probes,report.menu.details.game_session.cancel_reentry]) {
    const world=probe.window.game_session.active.world_initialization;
    world.actor_definitions.rng_draws=2;
    world.player_initialization=playerPreludeFixture(world.catalog_entries);
  }
  return report;
}

test('player physics prefix reports captured argument actual actor and capsule without readiness inflation',async()=>{
  await inspect(embed(withPlayerPrelude(gameSessionFixture())),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/Player basic initialization complete: captured constructor flag 0 \(unchanged\)/);
    assert.match(text,/Full virtual reset, character binding and inventory selection completed \(1 inventory record; capacity 16\)/);
    assert.match(text,/controller arena retains 1 EXItem object and 1 registered actor/);
    assert.match(text,/current item has a linked physics capsule/);
    assert.match(text,/numeric registry holds 1 record \(capacity 16, counter 1\); 1 constructor-origin query visited 2 nodes/);
    assert.match(text,/Physics activation and the basic session suffix completed.*handler scalar remains unresolved/);
    assert.match(text,/Scratch retains 4,736 bytes after 1 reallocation; 3 shared constructor allocations and 1 lease persist across cancellation/);
    assert.match(text,/Sessions await player creation/);
    assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
  });
  await inspect(embed(withCompletedLoader(gameSessionFixture())),d=>{
    assert.match(d.querySelector('#game-session-summary').textContent,/Player constructor evidence is absent/);
    assert.doesNotMatch(d.querySelector('#game-session-summary').textContent,/Player basic initialization complete/);
  });
});

test('player prelude rejects type count readiness digest and private-payload contradictions',async()=>{
  const changes=[
    player=>{delete player.persistent_base_resets;},player=>{player.native_bytes=[1,2,3];},
    player=>{player.operation_complete=true;},player=>{player.physics_bound=false;},player=>{player.player_created=true;},
    player=>{player.camera_ready=true;},player=>{player.completion_published=true;},player=>{player.handler_kind_bound=true;},
    player=>{player.attempt_bound=false;},player=>{player.catalog_bound=false;},player=>{player.base_constructor_complete=false;},
    player=>{player.catalog_entries=3;},player=>{player.selected_catalog_index=2;},player=>{player.selected_source_ordinal=-1;},
    player=>{player.handler_rng_reads=0;},player=>{player.base_reset_calls=1;},player=>{player.persistent_base_resets=0;},
    player=>{player.inventory_records=0;},player=>{player.inventory_capacity=0;},player=>{player.world_items=0;},
    player=>{player.actor_registrations=2;},player=>{player.stage='player_created';},player=>{player.pending_dependency='none';},
    player=>{player.original_history_verified=true;},player=>{player.native_timing_verified=true;},
    player=>{player.base_state_sha256='X'.repeat(64);},player=>{player.prelude_state_sha256='short';},
    player=>{delete player.shared_count;},player=>{player.shared_count=-1;},player=>{player.shared_live_leases=0;},
    player=>{player.shared_reset_calls=2;},player=>{player.shared_allocations=2;},player=>{player.shared_native_bytes=63679;},
    player=>{player.shared_host_capacity_bytes=63679;},player=>{player.shared_entry_noops=97;},player=>{player.shared_cache_stores=2;},
    player=>{player.shared_reset_complete=false;},player=>{player.runtime_base_reset_complete=false;},
    player=>{player.derived_reset_prefix_complete=false;},player=>{player.shared_state_sha256='short';},
    player=>{player.reset_state_sha256='X'.repeat(64);},player=>{player.shared_payload=[1,2,3];},
    player=>{player.scratch_prepared=false;},player=>{player.character_bound=false;},player=>{player.inventory_selection_complete=false;},
    player=>{player.scratch_capacity=31;},player=>{player.scratch_growth=0;},player=>{player.scratch_records=1;},
    player=>{player.scratch_native_bytes=4735;},player=>{player.scratch_host_capacity_bytes=4735;},player=>{player.scratch_reallocations=2;},
    player=>{player.character_resource=0;},player=>{player.character_key=0x4d000000;},player=>{player.character_source_index=-1;},
    player=>{player.guns_resource=0;},player=>{player.guns_payload_bytes=0;},player=>{player.weapon_registry_keys=5;},
    player=>{player.primary_order_keys=6;},player=>{player.secondary_order_keys=0;},player=>{player.inventory_primary_order_index=8;},
    player=>{player.inventory_interface_callbacks=1;},player=>{player.inventory_selected_record=1;},
    player=>{player.scratch_state_sha256='X'.repeat(64);},player=>{player.loadout_state_sha256='short';},
    player=>{player.pose_bits=[0,0,0,0];},player=>{player.scratch_payload=[1,2,3];},
    player=>{player.constructor_flag=null;},player=>{player.virtual_initialization=null;},
  ];
  for(const change of changes) {
    const report=withPlayerPrelude(gameSessionFixture());
    change(report.menu.details.game_session.cancel_reentry.window.game_session.active.world_initialization.player_initialization);
    await inspect(embed(report),d=>{
      const text=d.querySelector('#game-session-summary').textContent;
      assert.match(text,/has not been recorded completely/);
      assert.doesNotMatch(text,/Player basic initialization complete|all 4 difficulties admitted/);
    });
  }
});

test('player prelude stale mixed missing and unready world records cannot promote progress',async()=>{
  for(const state of ['stale','failed']) {
    const report=withPlayerPrelude(gameSessionFixture(state));
    await inspect(embed(report),d=>{
      assert.doesNotMatch(d.querySelector('#game-session-summary').textContent,/Player basic initialization complete|all 4 difficulties admitted/);
    });
  }
  for(const change of [world=>{world.player_initialization=null;},world=>{delete world.player_initialization;},
    world=>{world.loader.operation_complete=false;}]) {
    const report=withPlayerPrelude(gameSessionFixture());
    change(report.menu.details.game_session.cancel_reentry.window.game_session.active.world_initialization);
    await inspect(embed(report),d=>{
      assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/);
    });
  }
});

test('completed reset requires new scratch and loadout evidence without reusing the older partial schema',async()=>{
  for(const field of ['scratch_prepared','scratch_capacity','character_key','metadata_sorted_index','scratch_state_sha256','loadout_state_sha256','constructor_flag','virtual_initialization']) {
    const report=withPlayerPrelude(gameSessionFixture());
    delete report.menu.details.game_session.cancel_reentry.window.game_session.active.world_initialization.player_initialization[field];
    await inspect(embed(report),d=>{
      const text=d.querySelector('#game-session-summary').textContent;
      assert.match(text,/has not been recorded completely/);
      assert.doesNotMatch(text,/Player basic initialization complete|Full virtual reset, character binding and inventory selection completed/);
      assert.equal(d.querySelector('#menu-number').textContent,'100%','launch acceptance remains independent');
    });
  }
});

test('shared constructor reuse permits retained cancellation leases and completed reset without reallocating or creating a player',async()=>{
  for(const resets of [2,3,4]) {
    const report=withPlayerPrelude(gameSessionFixture());
    const data=report.menu.details.game_session;
    const first=data.difficulty_probes[0].window.game_session.active.world_initialization.player_initialization;
    first.shared_host_capacity_bytes=70000;
    const player=data.cancel_reentry.window.game_session.active.world_initialization.player_initialization;
    Object.assign(player,{shared_count:2,shared_live_leases:2,shared_reset_calls:2,persistent_base_resets:resets,
      shared_host_capacity_bytes:72000,shared_state_sha256:'a'.repeat(64),reset_state_sha256:'b'.repeat(64),
      base_state_sha256:'c'.repeat(64),prelude_state_sha256:'d'.repeat(64),world_state_sha256:'e'.repeat(64),
      persistent_state_sha256:'f'.repeat(64),state_sha256:'0'.repeat(64)});
    await inspect(embed(report),d=>{
      const text=d.querySelector('#game-session-summary').textContent;
      assert.match(text,/all 4 difficulties admitted/);
      assert.match(text,/3 shared constructor allocations and 1 lease persist across cancellation/);
      assert.match(text,/handler scalar remains unresolved; reading it stops explicitly/);
      assert.match(text,/TP\/FP initialization, player completion and camera remain pending/);
      assert.equal(player.shared_allocations,3);
      assert.equal(player.shared_reset_complete,true);
      assert.equal(player.scratch_reallocations,1);
      assert.equal(d.querySelector('#menu-number').textContent,'100%','the historical launch denominator stays separate');
    });
    player.shared_live_leases=1;
    await inspect(embed(report),d=>assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/));
  }
});

test('virtual player ownership fails closed on broken backlinks activation registry errors or source payload exports',async()=>{
  const virtualChanges=[
    state=>{state.factory_complete=false;},state=>{state.item_registry_bound=false;},state=>{state.actor_registered=false;},
    state=>{state.handler_bound=false;},state=>{state.origin_zone_queried=false;},state=>{state.physics_constructed=false;},
    state=>{state.physics_backlink_bound=false;},state=>{state.physics_activated=false;},state=>{state.initializer_complete=false;},state=>{state.scalar_pending=false;},state=>{state.scalar_value_known=true;},
    state=>{state.item_registry_records=0;},state=>{state.item_registry_capacity=0;},state=>{state.item_registry_counter=0;},
    state=>{state.item_registry_native_bytes=127;},state=>{state.item_registry_host_capacity_bytes=127;},
    state=>{state.origin_zone_queries=0;},state=>{state.origin_zone_nodes=0;},state=>{state.desired_membership=0x2f;},
    state=>{state.item_membership=0x2d;},state=>{state.item_flags=0x0b;},state=>{state.item_priority=0;},
    state=>{state.stage='complete';},state=>{state.pending_dependency='none';},state=>{state.item_state_sha256='X'.repeat(64);},
    state=>{state.physics_state_sha256='short';},state=>{delete state.registry_state_sha256;},state=>{state.capsule=[1,2,3];},
  ];
  const flagChanges=[
    flag=>{flag.origin='player_994';},flag=>{flag.raw=256;},flag=>{flag.changed=1;},flag=>{flag.interface_callbacks=1;},
    flag=>{flag.original_history_verified=true;},flag=>{flag.native_fields=[0,0,0,0];},
  ];
  for(const [key,changes] of [['virtual_initialization',virtualChanges],['constructor_flag',flagChanges]]) {
    for(const change of changes) {
      const report=withPlayerPrelude(gameSessionFixture());
      change(report.menu.details.game_session.cancel_reentry.window.game_session.active.world_initialization.player_initialization[key]);
      await inspect(embed(report),d=>{
        const text=d.querySelector('#game-session-summary').textContent;
        assert.match(text,/has not been recorded completely/);
        assert.doesNotMatch(text,/Player basic initialization complete|current item has a linked physics capsule/);
      });
    }
  }
});

test('unknown current constructor argument preserves reset and earlier actors without claiming a current physics owner',async()=>{
  const report=withPlayerPrelude(gameSessionFixture());
  const data=report.menu.details.game_session;
  for(const probe of [...data.difficulty_probes,data.cancel_reentry]) {
    Object.assign(probe.window.game_session.active.world_initialization.player_initialization,{
      stage:'player_reset_complete',pending_dependency:'constructor_source_flag',constructor_flag:null,
      virtual_initialization:null,component_initialization:null,physics_bound:false,world_items:1,actor_registrations:1,
    });
  }
  await inspect(embed(report),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/Player virtual reset complete: the current constructor flag is unknown/);
    assert.match(text,/controller arena retains 1 EXItem object and 1 registered actor/);
    assert.match(text,/This attempt has no current item or physics owner/);
    assert.doesNotMatch(text,/Player basic initialization complete|current item has a linked physics capsule/);
    assert.equal(d.querySelector('#menu-number').textContent,'100%');
  });
  data.cancel_reentry.window.game_session.active.world_initialization.player_initialization.physics_bound=true;
  await inspect(embed(report),d=>assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/));
});

test('retained registry can grow across cancellation while physics stays unactivated and raw flag bytes stay intact',async()=>{
  const report=withPlayerPrelude(gameSessionFixture());
  const data=report.menu.details.game_session;
  for(const probe of [...data.difficulty_probes,data.cancel_reentry]) {
    const player=probe.window.game_session.active.world_initialization.player_initialization;
    Object.assign(player,{world_items:3,actor_registrations:2});
    Object.assign(player.constructor_flag,{raw:255,changed:true});
    Object.assign(player.virtual_initialization,{item_registry_records:2,item_registry_counter:3,
      item_registry_host_capacity_bytes:1024,item_state_sha256:'d'.repeat(64),registry_state_sha256:'e'.repeat(64)});
  }
  await inspect(embed(report),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/captured constructor flag 255 \(updated\)/);
    assert.match(text,/controller arena retains 3 EXItem objects and 2 registered actors/);
    assert.match(text,/numeric registry holds 2 records \(capacity 16, counter 3\)/);
    assert.match(text,/TP\/FP initialization, player completion and camera remain pending/);
    assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
  });
  data.cancel_reentry.window.game_session.active.world_initialization.player_initialization.virtual_initialization.item_registry_records=4;
  await inspect(embed(report),d=>assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/));
});


test('TP skeleton binding keeps shared refresh attachment and FP explicit',async()=>{
  const report=withPlayerPrelude(gameSessionFixture());
  await inspect(embed(report),d=>{
    const text=d.querySelector('#game-session-summary').textContent;
    assert.match(text,/Third-person constructor retained: 4 animation slots, 1 active/);
    assert.match(text,/skin request retains 1,024 file bytes and 256 logical section bytes/);
    assert.match(text,/Initial action selected and all three mappings resolved through the retained inventory and supplied tables. TP captured four independent controls from primary locals and auxiliary storage; the active slot count remains one/);
    assert.match(text,/First command opcode 0xc000002 retained; execution pending. Command body retained 1 group weight write; awaiting phase-zero initialization. Factory class 4. One child header retained; no layer insertion or command return. Mode history and command return remain pending. Pose postprocessing and frame scheduling remain incomplete/);
  });
  const changes=[state=>{state.third_person_constructed=false;},state=>{state.third_person_attached=false;},
    state=>{state.first_person_constructed=false;},state=>{state.texture_dependencies_enumerated=false;},
    state=>{state.component_binding_complete=false;},state=>{state.skin_request_resolved=false;},
    state=>{state.texture_directory_outcome='unknown';},state=>{state.texture_requests=4097;},
    state=>{state.texture_targets_resident=2;},state=>{state.texture_residency_complete=false;},state=>{state.texture_requests_state_sha256='short';},
    state=>{state.active_slots=4;},state=>{state.constructed_slots=1;},state=>{state.native_component_bytes=0;},
    state=>{state.retained_section_bytes=2048;},state=>{state.target_state_sha256='short';},state=>{state.source_rows=[1,2,3];}];
  for (const key of ['primary_bound','secondary_bound','animator_backlink_bound','component_resource_bound'])
    changes.push(state=>{state.skeleton_binding[key]=false;});
  for (const [key,value] of Object.entries({empty_refresh_complete:false,frame_stamp_known:true,bones:129,roots:3,
    auxiliary_words:4097,native_runtime_bytes:0,native_storage_bytes:287,retained_skeleton_file_bytes:0,
    retained_component_file_bytes:0,state_sha256:'short',pose:[0,0,0,1]}))
    changes.push(state=>{state.skeleton_binding[key]=value;});
  changes.push(state=>{delete state.skeleton_binding;});
  for (const [key,value] of Object.entries({arena_bytes:0,reserved_bytes:0,reservation_start:0x1e00,cursor_after:16,
    live_reservations_after:1,refresh_ordinal:0,allocation_ordinal:0,regions_bound:3,flags_after:0x4000,
    scope_released:false,state_sha256:'short',payload:[1,2,3]}))
    changes.push(state=>{state.bind_refresh[key]=value;});
  changes.push(state=>{delete state.bind_refresh;});
  for (const [key,value] of Object.entries({identity_registered:false,item_backlink_bound:false,item_list_member:false,
    item_list_ordinal:1,item_components:0,registry_identities:0,component_flag_8e:1,head_link_null:false,next_link_bound:false,
    state_sha256:'short',source_pointer:123}))
    changes.push(state=>{state.attachment[key]=value;});
  changes.push(state=>{delete state.attachment;});
  for (const [key,value] of Object.entries({wrapper_reused:false,secondary_skin_bound:false,skin_requests:0,
    texture_requests:2,texture_targets_resident:2,override_generation:0,override_capacity:1,override_growth:0,
    override_count:1,item_collection_bound:false,configured_headers:1,callback_bindings_complete:false,state_sha256:'short',raw_bones:[1,2,3]}))
    changes.push(state=>{state.visual_postbind[key]=value;});
  changes.push(state=>{delete state.visual_postbind;});
  for (const [key,value] of Object.entries({interface_bound:false,table_shared:false,callback_slots:3,noop_slots:0,
    pose_target_assigned:false,pose_postprocess_implemented:true,tp_helper_returned:false,events_executed:true,
    state_sha256:'short',raw_code:[1,2,3]})) changes.push(state=>{state.callbacks[key]=value;});
  for (const key of Object.keys(callbacksPlayerFixture())) changes.push(state=>{delete state.callbacks[key];});
  changes.push(state=>{delete state.callbacks;});
  for (const [key,value] of Object.entries({helper_returned:false,identity_registered:false,item_list_ordinal:0,
    ordered_links_bound:false,masks_configured:false,table_shared:false,pose_target_assigned:false,
    visual_postbind_skipped:false,skeleton:null,bind_refresh:null,state_sha256:'short',pose:[1,2,3]}))
    changes.push(state=>{state.first_person[key]=value;});
  for (const key of Object.keys(firstPersonPlayerFixture())) changes.push(state=>{delete state.first_person[key];});
  changes.push(state=>{delete state.first_person;});
  changes.push(state=>{state.first_person.bind_refresh.refresh_ordinal=1;});
  changes.push(state=>{state.first_person.bind_refresh.allocation_ordinal=1;});
  changes.push(state=>{state.first_person.skeleton.frame_stamp_known=true;});
  for (const [key,value] of Object.entries({stage:'complete',generic_request:0,parameter_bits:1,direction_raw:1,
    weapon_key:-1,variant:2,remap_calls:2,map_rows_read:12289,fp_mapped:0,tp_mapped:0,tp_secondary_mapped:0,
    source_tables_reused:false,mode_controls_created:0,mode_history_updated:true,callback_returned:true,
    state_sha256:'short',mapping_state_sha256:'short',source_table:[1,2,3]})) changes.push(state=>{state.initial_action[key]=value;});
  for (const key of Object.keys(initialActionPlayerFixture())) changes.push(state=>{delete state.initial_action[key];});
  changes.push(state=>{delete state.initial_action;});
  for (const [key,value] of Object.entries({physical_slots:1,controls:2,bones_per_control:129,auxiliary_words_per_control:4097,
    native_header_bytes:0,native_backing_bytes_per_control:79,active_slots:4,capture_mask:3,preserve_mask:15,
    update_flags_after:11,frame_stamp_known:true,postprocess_executed:true,fallback:0,fallback_writes:1,
    refresh:null,state_sha256:'short',pose:[1,2,3]})) changes.push(state=>{state.initial_action.tp_capture[key]=value;});
  for (const key of Object.keys(tpCapturePlayerFixture())) changes.push(state=>{delete state.initial_action.tp_capture[key];});
  changes.push(state=>{delete state.initial_action.tp_capture;});
  for (const [key,value] of Object.entries({stage:'complete',requested:0,previous:1,fallback:0,
    tracks_player_modes:null,target_index:null,attempts:0,lookups:3,transition_rows:0,commands_issued:0,
    commands_executed:1,pending_ordinal:1,pending_opcode:null,return_word:0,base_only:false,
    command_effects_complete:true,state_sha256:'short',payload:[1,2,3]})) changes.push(state=>{state.initial_action.tp_selector[key]=value;});
  for (const key of Object.keys(tpSelectorPlayerFixture())) changes.push(state=>{delete state.initial_action.tp_selector[key];});
  changes.push(state=>{delete state.initial_action.tp_selector;});
  changes.push(state=>{state.initial_action.stage='tp_setter_tail_pending';});
  for (const [key,value] of Object.entries({stage:'complete',opcode:0,set_reference:null,set_index:32767,set_groups:0,
    groups_visited:2,weight_writes:2,pending_group:1,pending_slot:4,child_reference:null,factory_class:3,child_flags:65536,
    child_divisor:-1,mask:1,children_created:0,command_returned:true,cleanup_complete:true,selector_acknowledged:true,
    state_sha256:'short',layers_state_sha256:'short',payload:[1,2,3]})) changes.push(state=>{state.initial_action.tp_command[key]=value;});
  for(const key of Object.keys(tpCommandPlayerFixture())) changes.push(state=>{delete state.initial_action.tp_command[key];});
  changes.push(state=>{state.initial_action.tp_command=null;});
  for (const [key,value] of Object.entries({native_header_bytes:128,resource_reused:false,context_bound:false,generation_observed:-1,cache_generation:7,metadata_cached:false,descriptor_index:null,body_bound:false,phase_zero_initialized:true,initializer_returned:true,factory_returned:true,layer_inserted:true,state_sha256:'short',source:[1,2,3]})) changes.push(state=>{state.initial_action.tp_command.child_header[key]=value;});
  for (const key of Object.keys(tpChildPlayerFixture())) changes.push(state=>{delete state.initial_action.tp_command.child_header[key];});
  changes.push(state=>{state.initial_action.tp_capture.refresh.refresh_ordinal=2;});
  changes.push(state=>{state.initial_action.tp_capture.refresh.allocation_ordinal=2;});
  changes.push(state=>{state.initial_action.tp_capture.bones_per_control=3;state.initial_action.tp_capture.native_backing_bytes_per_control=112;});
  for(const change of changes) {
    const invalid=withPlayerPrelude(gameSessionFixture());
    change(invalid.menu.details.game_session.cancel_reentry.window.game_session.active.world_initialization.player_initialization.component_initialization);
    await inspect(embed(invalid),d=>assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/));
  }
});


test('live selector returns report only the pending setter tail and unknown opcodes stay pending',async()=>{
  for (const missingTarget of [false,true]) {
    const report=withPlayerPrelude(gameSessionFixture());
    for(const probe of [...report.menu.details.game_session.difficulty_probes,report.menu.details.game_session.cancel_reentry]) {
      const player=probe.window.game_session.active.world_initialization.player_initialization;
      player.pending_dependency='tp_mode_setter_tail';
      const component=player.component_initialization;
      component.stage='tp_setter_tail_pending';component.pending_dependency='tp_mode_setter_tail';
      component.initial_action.stage=component.stage;
      component.initial_action.tp_command=null;
      Object.assign(component.initial_action.tp_selector,{stage:'returned',commands_issued:0,pending_ordinal:null,
        pending_opcode:null,return_word:missingTarget ? 0 : 0x090000ab,
        ...(missingTarget ? {target_index:null,attempts:0,lookups:1,transition_rows:0} : {})});
    }
    await inspect(embed(report),d=>{
      const text=d.querySelector('#game-session-summary').textContent;
      assert.match(text,/Selector returned; its setter tail is pending/);
      assert.match(text,/Mode history and command return remain pending/);
      assert.doesNotMatch(text,/First command opcode|has not been recorded completely/);
      assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
    });
    report.menu.details.game_session.cancel_reentry.window.game_session.active.world_initialization.player_initialization.pending_dependency='tp_animation_command';
    await inspect(embed(report),d=>assert.match(d.querySelector('#game-session-summary').textContent,/has not been recorded completely/));
  }
  const report=withPlayerPrelude(gameSessionFixture());
  for(const probe of [...report.menu.details.game_session.difficulty_probes,report.menu.details.game_session.cancel_reentry]) {
    const action=probe.window.game_session.active.world_initialization.player_initialization.component_initialization.initial_action;
    action.tp_selector.pending_opcode=0x0c0000ff;
    action.tp_command={...tpCommandPlayerFixture(),stage:'opcode_pending',opcode:0x0c0000ff,set_reference:null,set_index:null,set_groups:null,
      groups_visited:0,weight_writes:0,pending_group:null,pending_slot:null,child_reference:null,factory_class:null,child_flags:null,child_divisor:null,child_header:null,children_created:0};
  }
  await inspect(embed(report),d=>assert.match(d.querySelector('#game-session-summary').textContent,/First command opcode 0xc0000ff retained; execution pending/));
});


test('command cleanup and mask boundaries stay pending without factory or selector completion',async()=>{
  const noChild={pending_group:null,pending_slot:null,child_reference:null,factory_class:null,child_flags:null,child_divisor:null,child_header:null,children_created:0};
  const noSet={set_reference:null,set_index:null,set_groups:null,groups_visited:0,weight_writes:0};
  for(const [body,dependency] of [
    [{...tpCommandPlayerFixture(),...noChild,stage:'cleanup_pending'},'set cleanup'],
    [{...tpCommandPlayerFixture(),...noChild,...noSet,stage:'mask_control_pending',opcode:0x0c000005,mask:15},'control capture'],
    [{...tpCommandPlayerFixture(),...noChild,...noSet,stage:'mask_clear_pending',opcode:0x0c000005,mask:0},'mask clear'],
    [{...tpCommandPlayerFixture(),...noChild,...noSet,stage:'opcode_pending',opcode:0x0c000004},'opcode provider'],
  ]) {
    const report=withPlayerPrelude(gameSessionFixture());
    for(const probe of [...report.menu.details.game_session.difficulty_probes,report.menu.details.game_session.cancel_reentry]) {
      const action=probe.window.game_session.active.world_initialization.player_initialization.component_initialization.initial_action;
      action.tp_selector.pending_opcode=body.opcode;action.tp_command={...body};
    }
    await inspect(embed(report),d=>{
      const text=d.querySelector('#game-session-summary').textContent;
      assert.ok(text.includes(`awaiting ${dependency}.`));
      assert.match(text,/No child was created or command returned/);
      assert.doesNotMatch(text,/has not been recorded completely/);
      assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
    });
  }
});


test('retained script header distinguishes pending interpretation from a native no-body return',async()=>{
  for (const cached of [true,false]) {
    const report=withPlayerPrelude(gameSessionFixture());
    for(const probe of [...report.menu.details.game_session.difficulty_probes,report.menu.details.game_session.cancel_reentry]) {
      const body=probe.window.game_session.active.world_initialization.player_initialization.component_initialization.initial_action.tp_command;
      body.stage='child_effects_pending';Object.assign(body.child_header,{body_bound:false,initializer_returned:true,factory_returned:true,
        metadata_cached:cached,descriptor_index:cached ? 2 : null,cache_generation:cached ? 0 : 0xffffffff});
    }
    await inspect(embed(report),d=>{
      const text=d.querySelector('#game-session-summary').textContent;
      assert.match(text,/awaiting child effects/);assert.match(text,/One child header retained; no layer insertion or command return/);
      assert.doesNotMatch(text,/has not been recorded completely/);
      assert.equal(d.querySelector('#parity-count').textContent,'0 of 3 tracked criteria verified');
    });
  }
});
