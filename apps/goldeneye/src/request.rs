//! Static single-map / explicit-context navigation probes; no actor controller.
use crate::path::PathData;
use anyhow::{Context, Result, ensure};
use ge_assets::{
    edb::Edb,
    navigation::{MapNavigation, NavigationBinding},
    resource::GameAssets,
};
use ge_core::navigation::{
    ActorBoundaryBand, ActorBoundarySample, NavigationFrame, NavigationRequestState,
    NavigationRouteState, WorldGoalPathRequest, WorldPathRequest, acquire_navigation,
    actor_boundary_gate,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(clap::Args)]
pub struct Options {
    disc: PathBuf,
    resource: String,
    /// Explicit shared frame for root entities, mutually exclusive with static map bindings.
    #[arg(
        long,
        num_args = 3,
        allow_hyphen_values = true,
        required_unless_present = "map",
        conflicts_with = "map"
    )]
    translation: Option<Vec<f32>>,
    /// Bind placed navmeshes from this map hash, using recovered static single-map rules.
    #[arg(long,value_parser=crate::parse_hash)]
    map: Option<u32>,
    #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
    start: Vec<f32>,
    /// Optional later authored positions; repeat once per remaining dispatch call.
    #[arg(long = "next-start", num_args = 3, allow_hyphen_values = true)]
    next_starts: Vec<f32>,
    #[arg(long, num_args = 3, allow_hyphen_values = true, required = true)]
    goal: Vec<f32>,
    /// Optional authored predicate goals for the acquired mesh; requires an explicit heuristic point.
    #[arg(long = "accept-triangle", requires = "heuristic_point")]
    accepted: Vec<usize>,
    /// Explicit mesh-local squared-distance heuristic point for predicate route mode.
    #[arg(long, num_args = 3, allow_hyphen_values = true, requires = "accepted")]
    heuristic_point: Option<Vec<f32>>,
    /// Apply actor target-directed edge checks in predicate mode; full goal policy remains authored.
    #[arg(long, requires = "heuristic_point")]
    actor_edges: bool,
    /// Explicit hexadecimal actor flags. Both 0x100 and 0x200 are needed to process.
    #[arg(long, value_parser=crate::parse_hash, required=true)]
    flags_raw: u32,
    /// Number of dispatch calls (1..=16), not game ticks. Acquisition defers a query.
    #[arg(long, default_value_t = 2)]
    steps: usize,
    #[arg(long, default_value_t = 100_000)]
    max_expansions: usize,
    /// Check the selected binding's ordered boundary cache using raw input XZ coordinates.
    #[arg(long)]
    boundary_check: bool,
    /// Probe one observed actor range band: 4.5, 3.4, 3.45 or 3.6. Inputs are authored snapshots.
    #[arg(long, value_parser=parse_actor_band, requires="actor_mode")]
    actor_boundary_band: Option<ActorBoundaryBand>,
    /// Explicit decimal mode snapshot (+0x78c); only zero/one reach the geometric gates.
    #[arg(long, requires = "actor_boundary_band")]
    actor_mode: Option<u32>,
    #[arg(long)]
    pub output: Option<PathBuf>,
}

pub fn run(options: &Options) -> Result<Value> {
    ensure!(
        (1..=16).contains(&options.steps),
        "--steps must be in 1..=16"
    );
    ensure!(
        options.max_expansions > 0,
        "--max-expansions must be positive"
    );
    let vector = |v: &[f32]| -> Result<[f32; 3]> {
        let v: [f32; 3] = v.try_into().context("expected three coordinates")?;
        ensure!(
            v.into_iter().all(f32::is_finite),
            "request coordinates must be finite"
        );
        Ok(v)
    };
    let translation = options.translation.as_deref().map(vector).transpose()?;
    let start = vector(&options.start)?;
    let goal = vector(&options.goal)?;
    let heuristic_point = options.heuristic_point.as_deref().map(vector).transpose()?;
    let starts = if options.next_starts.is_empty() {
        vec![start; options.steps]
    } else {
        ensure!(
            options.next_starts.len() == 3 * (options.steps - 1),
            "--next-start must be supplied exactly once per remaining dispatch call"
        );
        std::iter::once(Ok(start))
            .chain(options.next_starts.chunks_exact(3).map(vector))
            .collect::<Result<Vec<_>>>()?
    };
    let edb = GameAssets::open(&options.disc, "Filelist.bin")?.load_edb(&options.resource)?;
    let (residents, slots, candidates, map_navigation) = prepare(&edb, options.map, translation)?;
    let instances = slots
        .iter()
        .map(|s| residents[s.resident].instance(s.frame))
        .collect::<Vec<_>>();
    let mut state = NavigationRequestState {
        flags_raw: options.flags_raw,
        binding: None,
    };
    let (mut acquisition, mut last_query) = (None, None);
    let mut route_state = NavigationRouteState::default();
    let mut dispatches = Vec::new();
    for &start in &starts {
        let before_flags = state.flags_raw;
        let event=state.dispatch(|| {
            let result=acquire_navigation(&instances,start,30.)?;
            let binding=result.selected.as_ref().map(|s|s.instance);
            acquisition=Some(result);
            Ok(binding)
        }, |binding| {
            let slot=slots.get(binding).context("cached binding outside candidate set")?;
            let data=&residents[slot.resident];
            if let Some(point)=heuristic_point {
                let request=WorldGoalPathRequest {frame:slot.frame,start,max_expansions:options.max_expansions};
                let query=data.regenerate_goal_route(&mut route_state,request,&options.accepted,point,options.actor_edges)?;
                last_query=Some(json!({"candidate":binding,"query_mode":"predicate","edge_policy":if options.actor_edges {"actor_target_direction"}else{"default_immediate_backtrack"},"request":request,"nominal_goal":goal,"goal_policy":{"kind":"authored_face_set","accepted":options.accepted},"heuristic_point":point,"result":query.result,"waypoints":query.waypoints}));
                Ok(query.success)
            } else {
                let request=WorldPathRequest {frame:slot.frame,start,goal,max_expansions:options.max_expansions};
                let query=data.regenerate_route(&mut route_state,request)?;
                last_query=Some(json!({"candidate":binding,"request":request,"result":query.result,"waypoints":query.waypoints}));
                Ok(query.success)
            }
        })?;
        dispatches.push(json!({"start":start,"before_flags":before_flags,"processed":event.processed(),"event":event,"after_flags":state.flags_raw,"binding":state.binding,"route_state":route_state}));
    }
    let start = starts[options.steps - 1];
    let boundary_check = if options.boundary_check {
        state.binding.map(|binding| -> Result<Value> {
            let slot = slots.get(binding).context("boundary binding outside candidate set")?;
            let hit = residents[slot.resident].boundary_check(start, goal)?;
            Ok(json!({"candidate":binding,"clear":hit.is_none(),"first_blocker":hit,
                "coordinate_rule":"raw input XZ against local vertices; no frame conversion; Y ignored"}))
        }).transpose()?
    } else {
        None
    };
    let boundary_caches = residents
        .iter()
        .map(PathData::boundary_stats)
        .collect::<Vec<_>>();
    let actor_boundary = options.actor_boundary_band.map(|band| -> Result<Value> {
        let (mut samples,mut boundary_queries)=(0,0);
        let mut first_blocker=None;
        let result=actor_boundary_gate(band,options.actor_mode.context("actor mode required")?,state.binding.is_some(),|| {
            samples+=1; Ok(ActorBoundarySample {actor:start,target:goal})
        },|a,b| {
            boundary_queries+=1;
            let slot=slots.get(state.binding.context("actor boundary binding missing")?).context("actor boundary binding outside candidates")?;
            first_blocker=residents[slot.resident].boundary_check(a,b)?;
            Ok(first_blocker.is_none())
        })?;
        Ok(json!({"band":band,"mode":options.actor_mode,"accepted":result.accepted(),"result":result,"position_samples":samples,"boundary_queries":boundary_queries,"first_blocker":first_blocker,"position_source":"authored stable start/goal snapshots; not recorded actor or target records"}))
    }).transpose()?;
    Ok(
        json!({"resource_hash":edb.header.hash,"route_mode":if heuristic_point.is_some(){"predicate"}else{"ordinary"},"candidate_context":if options.map.is_some(){"static_single_map"}else{"explicit_roots"},"map_navigation":map_navigation,"resident_meshes":residents.len(),"candidates":candidates,"state":state,"route_state":route_state,"dispatches":dispatches,"last_acquisition":acquisition,"last_query":last_query,"boundary_caches":boundary_caches,"boundary_check_requested":options.boundary_check,"boundary_check":boundary_check,"actor_boundary":actor_boundary,"reference_compared":false,
        "scope":"Deferred dispatch with persistent ordinary/predicate routes and authored positions/policies. Static-map mode reconstructs initial registrations and translations for one map. Acquisition uses distance 30. Predicate routing binds only start and uses the selected raw center through subtract/add translation for waypoints; the nominal goal remains separate. Optional actor edges use observed flag/stored-normal/direction gates with the explicit raw point. Misses retain lists with distinct wrapper status. Boundary and actor-band probes remain independent. No action scheduling or movement simulation. Full actor goals, actual point sources, loaded-map precedence, mutable state, steering, movement and original/Wii FP parity remain unverified."}),
    )
}

fn parse_actor_band(value: &str) -> std::result::Result<ActorBoundaryBand, String> {
    match value {
        "4.5" => Ok(ActorBoundaryBand::Radius45),
        "3.4" => Ok(ActorBoundaryBand::Radius34),
        "3.45" => Ok(ActorBoundaryBand::Radius345),
        "3.6" => Ok(ActorBoundaryBand::Radius36),
        _ => Err("expected one observed actor band: 4.5, 3.4, 3.45, 3.6".into()),
    }
}

struct Slot {
    resident: usize,
    frame: NavigationFrame,
}
type Prepared = (Vec<PathData>, Vec<Slot>, Vec<Value>, Option<MapNavigation>);
fn prepare(edb: &Edb, map: Option<u32>, translation: Option<[f32; 3]>) -> Result<Prepared> {
    let mut residents = Vec::new();
    let mut candidates = Vec::new();
    let mut slots = Vec::new();
    if let Some(map) = map {
        ensure!(
            translation.is_none(),
            "map mode cannot use an authored translation"
        );
        let navigation = edb.map_navigation(map)?;
        let mut cache = BTreeMap::new();
        for (manager_slot, registration) in navigation.registrations.iter().enumerate() {
            let NavigationBinding::Ready {
                resolved_placement,
                entity_index,
                translation,
            } = registration.binding
            else {
                continue;
            };
            let resident = if let Some(&r) = cache.get(&entity_index) {
                r
            } else {
                let graph = edb.entity_graph_at(entity_index)?;
                let index = residents.len();
                residents.push(PathData::new(&graph)?);
                cache.insert(entity_index, index);
                index
            };
            let frame = NavigationFrame { translation };
            candidates.push(json!({"manager_slot":manager_slot,"source_placement":registration.source_placement,"resolved_placement":resolved_placement,"instance_key":registration.instance_key,"directory_index":entity_index,"frame":frame,"resident_mesh":resident}));
            slots.push(Slot { resident, frame });
        }
        return Ok((residents, slots, candidates, Some(navigation)));
    }
    let frame = NavigationFrame {
        translation: translation.context("choose --map or --translation")?,
    };
    for (index, entity) in edb.entities()?.iter().enumerate() {
        if entity.entity_type != 0x607 {
            continue;
        }
        let graph = edb.entity_graph_at(index)?;
        candidates.push(json!({"directory_index":index,"entity_hash":entity.hash,"frame":frame,"triangles":graph.triangles.len()}));
        slots.push(Slot {
            resident: residents.len(),
            frame,
        });
        residents.push(PathData::new(&graph)?);
    }
    Ok((residents, slots, candidates, None))
}

#[cfg(test)]
#[path = "tests/request.rs"]
mod tests;
