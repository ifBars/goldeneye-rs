use super::{Attempt, AttemptKind, Budget, Inputs, Provider, Selection, Status, Work};
use crate::animation::mode::plan::{charge, command_plan};
use anyhow::{Context, Result};

pub fn select(provider: &mut impl Provider, inputs: Inputs, budget: Budget) -> Result<Selection> {
    let mut selection = Selection {
        inputs,
        status: Status::ResourceUnavailable,
        target_index: None,
        attempts: Vec::new(),
        return_word: 0,
        work: Work::default(),
    };
    if !inputs.resource_available {
        return Ok(selection);
    }
    charge(&mut selection.work.lookups, 1, budget.lookups, "lookup")?;
    let Some(target) = provider.resolve_mode(inputs.requested)? else {
        selection.status = Status::TargetMissing;
        return Ok(selection);
    };
    let target_word = u32::try_from(target).context("target mode index exceeds source word")?;
    selection.target_index = Some(target);
    for (kind, reference) in [
        (AttemptKind::Previous, inputs.previous),
        (AttemptKind::Fallback, inputs.fallback),
    ] {
        let attempt = attempt(
            provider,
            kind,
            reference,
            target_word,
            budget,
            &mut selection.work,
        )?;
        selection.return_word = if attempt.matched_transition.is_some() {
            attempt
                .source_hash
                .context("matched transition is missing source hash")?
        } else {
            0
        };
        selection.attempts.push(attempt);
        if selection.return_word != 0 {
            break;
        }
    }
    selection.status = Status::AttemptsComplete;
    Ok(selection)
}

fn attempt(
    provider: &mut impl Provider,
    kind: AttemptKind,
    reference: u32,
    target: u32,
    budget: Budget,
    work: &mut Work,
) -> Result<Attempt> {
    let mut attempt = Attempt {
        kind,
        reference,
        source_index: None,
        source_hash: None,
        matched_transition: None,
        commands: Vec::new(),
    };
    charge(&mut work.lookups, 1, budget.lookups, "lookup")?;
    let Some(index) = provider.resolve_mode(reference)? else {
        return Ok(attempt);
    };
    attempt.source_index = Some(index);
    let mode = provider.mode(index)?;
    attempt.source_hash = Some(mode.hash);
    for ordinal in 0..mode.transition_count {
        charge(
            &mut work.transition_rows,
            1,
            budget.transition_rows,
            "transition row",
        )?;
        if provider.transition_target(index, ordinal)? != target {
            continue;
        }
        attempt.matched_transition = Some(ordinal);
        let commands = provider.commands(index, ordinal)?;
        charge(
            &mut work.commands,
            commands.len(),
            budget.commands,
            "command",
        )?;
        let mut plans = Vec::with_capacity(commands.len());
        for command in commands {
            plans.push(command_plan(command, budget, work)?);
        }
        attempt.commands = plans;
        break;
    }
    Ok(attempt)
}
