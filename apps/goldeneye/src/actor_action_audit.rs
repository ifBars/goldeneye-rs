//! Authored action snapshots; independent angle and f64 displacement diagnostics, not runtime parity.
use super::*;
use ge_core::navigation::{
    ActorTargetActionState, ActorTargetOwner, ActorTargetRecord, ActorTargetRequest,
    actor_target_gate, run_actor_target_action,
};
#[derive(Default, Serialize)]
pub(super) struct Audit {
    calls: usize,
    aligned_calls: usize,
    gated_calls: usize,
    updates: usize,
    noops: usize,
    errors: usize,
    motion_validated: usize,
    angle_probes: usize,
    angle_validated: usize,
    angle_ambiguous: usize,
    max_motion_error: f64,
}
impl Audit {
    pub(super) fn include(&mut self, s: Self) {
        macro_rules! sum {($($f:ident),*)=>{$(self.$f+=s.$f;)*};}
        sum!(
            calls,
            aligned_calls,
            gated_calls,
            updates,
            noops,
            errors,
            motion_validated,
            angle_probes,
            angle_validated,
            angle_ambiguous
        );
        self.max_motion_error = self.max_motion_error.max(s.max_motion_error);
    }
}
impl PathData {
    pub(super) fn probe_actor_action(
        &self,
        request: ActorTargetRequest,
        frame: NavigationFrame,
    ) -> Result<Audit> {
        let mut a = Audit::default();
        let local = request.owner_frame.position_to_local(request.target)?;
        let base = f64::from(local[0]).atan2(f64::from(local[2])) as f32;
        let tau = f32::from_bits(0x40c90fdb);
        let bearing = (f64::from(request.target[0])
            - f64::from(request.owner_frame.translation[0]))
        .atan2(f64::from(request.target[2]) - f64::from(request.owner_frame.translation[2]));
        for index in [0, 1] {
            for mode in [0, 1] {
                let (lower, upper) = if index == 0 {
                    (f32::from_bits(0x3f490fdb), f32::from_bits(0x3f490fdb))
                } else {
                    (f32::from_bits(0x3edf66f3), f32::from_bits(0x3e5f66f3))
                };
                let (lower, upper) = if mode == 0 {
                    (lower, upper)
                } else {
                    (upper, lower)
                };
                for offset in [0., 0.2, -0.2, 0.4, -0.4, 0.8, -0.8, tau * 3., -upper, lower] {
                    let yaw = base + offset;
                    let gate = actor_target_gate(
                        ActorTargetOwner {
                            frame: request.owner_frame,
                            yaw,
                        },
                        request.target,
                        index,
                        mode,
                    )?;
                    let mut difference = (bearing - f64::from(yaw)) % f64::from(tau);
                    if difference > f64::from(tau) * 0.5 {
                        difference -= f64::from(tau);
                    }
                    if difference < -f64::from(tau) * 0.5 {
                        difference += f64::from(tau);
                    }
                    a.angle_probes += 1;
                    if (difference - f64::from(upper)).abs() < 1e-5
                        || (difference + f64::from(lower)).abs() < 1e-5
                    {
                        a.angle_ambiguous += 1;
                    } else {
                        ensure!(
                            gate.allowed
                                == (difference < f64::from(upper)
                                    && difference > -f64::from(lower)),
                            "actor angular gate differs from independent f64 bearing/modulo conditions"
                        );
                        a.angle_validated += 1;
                    }
                }
            }
        }
        let owner = ActorTargetOwner {
            frame: request.owner_frame,
            yaw: base,
        };
        let before = ActorTargetActionState {
            state_raw: 0x12345678,
            active_index: 0,
            mode_raw: 0,
            target: request.target,
            records: vec![ActorTargetRecord {
                position: request.active_position,
                motion: [7., 8., 9.],
            }],
        };
        let mut state = before.clone();
        let mut selected = None;
        let result = run_actor_target_action(&mut state, owner, request.max_face_tests, |r| {
            ensure!(
                r.target == request.target && r.active_position == request.active_position,
                "actor action query inputs changed"
            );
            let q = self.query_actor_target(r, || Ok(Some(frame)), 100_000, 1024)?;
            selected = q.selection()?;
            Ok(selected)
        });
        a.calls += 1;
        a.aligned_calls = 1;
        match result {
            Ok(result) => {
                ensure!(result.gate.allowed, "aligned actor action was gated");
                if let Some(update) = result.update {
                    a.updates = 1;
                    ensure!(
                        result.return_code == 2
                            && state.state_raw == 2
                            && state.records[0].motion == update.motion,
                        "actor selected update state changed"
                    );
                    ensure!(
                        state.records[0].position == before.records[0].position
                            && state.target == before.target,
                        "actor update moved its position/target"
                    );
                    let s = selected.context("actor update missing selected center")?;
                    for i in 0..3 {
                        let expected = (f64::from(s.raw_center[i])
                            + f64::from(s.frame.translation[i])
                            - f64::from(request.active_position[i]))
                            * 0.5
                            + if i == 1 { 1. } else { 0. };
                        let error = (f64::from(update.motion[i]) - expected).abs();
                        a.max_motion_error = a.max_motion_error.max(error);
                        ensure!(
                            error <= 1e-4 * (1. + expected.abs()),
                            "actor motion differs from independent f64 displacement approximation"
                        );
                    }
                    a.motion_validated = 1;
                } else {
                    ensure!(
                        result.return_code == 1 && state == before,
                        "actor no-selection call changed state"
                    );
                    a.noops = 1;
                }
            }
            Err(e)
                if e.to_string().contains("no supported forward crossing")
                    || e.to_string().contains("actor goal face budget exhausted")
                    || e.to_string()
                        .contains("navigation path expansion budget exhausted")
                    || e.to_string()
                        .contains("unsupported zero/underflow actor target motion length") =>
            {
                ensure!(state == before, "host actor action error changed state");
                a.errors = 1;
            }
            Err(e) => return Err(e),
        }
        let mut gated = before.clone();
        let result = run_actor_target_action(
            &mut gated,
            ActorTargetOwner {
                yaw: base + f32::from_bits(0x40490fdb),
                ..owner
            },
            request.max_face_tests,
            |_| anyhow::bail!("gated actor action evaluated its query"),
        )?;
        ensure!(
            !result.gate.allowed && result.return_code == 1 && gated == before,
            "gated actor action changed state"
        );
        a.gated_calls = 1;
        a.calls += 1;
        a.noops += 1;
        Ok(a)
    }
}
