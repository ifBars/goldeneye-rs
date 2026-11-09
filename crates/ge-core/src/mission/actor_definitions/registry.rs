use std::{cmp::Ordering, num::NonZeroU64};

use anyhow::{Context, Result, ensure};

use super::types::{
    Budget, DefaultReason, Duplicate, Group, LookupBudget, Primary, Record, RecordId, Selection,
    SpecialKind,
};

#[derive(Debug)]
pub struct Registry {
    input_rows: usize,
    records: Vec<Record>,
    duplicates: Vec<Duplicate>,
    // Source group creation order: first encounter in sorted primary storage.
    groups: Vec<Group>,
    // Host-only unsigned key index; never changes group/member storage order.
    group_index: Vec<(u16, usize)>,
    member_count: usize,
}

impl Registry {
    pub fn build(input: Vec<Primary>, budget: Budget) -> Result<Self> {
        let input_rows = input.len();
        ensure!(
            input_rows <= budget.rows,
            "actor primary row budget exceeded"
        );
        let mut arrivals = Vec::new();
        arrivals.try_reserve_exact(input_rows)?;
        arrivals.extend(input.into_iter().enumerate());
        arrivals.sort_unstable_by_key(|(arrival, row)| (row.words[0], *arrival));

        let mut records: Vec<Record> = Vec::new();
        let mut duplicates = Vec::new();
        records.try_reserve_exact(input_rows)?;
        duplicates.try_reserve_exact(input_rows)?;
        for (arrival, row) in arrivals {
            if let Some(retained) = records.last().filter(|r| r.key() == row.words[0]) {
                duplicates.push(Duplicate {
                    key: row.words[0],
                    retained_source_index: retained.source_index,
                    rejected_source_index: row.source_index,
                    arrival,
                });
                continue;
            }
            let one_based = records.len().checked_add(1).context("record ID overflow")?;
            let id =
                RecordId(NonZeroU64::new(u64::try_from(one_based)?).context("zero record ID")?);
            records.push(Record {
                id,
                source_index: row.source_index,
                words: row.words,
            });
        }
        duplicates.sort_unstable_by_key(|duplicate| duplicate.arrival);

        let group_capacity = records.len().min(budget.groups);
        let mut groups: Vec<Group> = Vec::new();
        let mut group_index: Vec<(u16, usize)> = Vec::new();
        groups.try_reserve_exact(group_capacity)?;
        group_index.try_reserve_exact(group_capacity)?;
        let mut member_count = 0usize;
        for record in &records {
            if record.group_word() == u32::MAX {
                continue;
            }
            let key = record.group_word() as u16;
            let group = match group_index.binary_search_by_key(&key, |entry| entry.0) {
                Ok(index) => group_index[index].1,
                Err(index) => {
                    ensure!(groups.len() < budget.groups, "actor group budget exceeded");
                    let group = groups.len();
                    groups.push(Group {
                        key,
                        count: 0,
                        members: Vec::new(),
                        cursor: 0,
                    });
                    group_index.insert(index, (key, group));
                    group
                }
            };
            member_count = member_count
                .checked_add(1)
                .context("member count overflow")?;
            ensure!(
                member_count <= budget.members,
                "actor total member budget exceeded"
            );
            groups[group].count = groups[group]
                .count
                .checked_add(1)
                .context("group count overflow")?;
        }
        for group in &mut groups {
            group.members.try_reserve_exact(group.count)?;
        }
        for record in &records {
            if record.group_word() == u32::MAX {
                continue;
            }
            let key = record.group_word() as u16;
            let index = group_index
                .binary_search_by_key(&key, |entry| entry.0)
                .map_err(|_| anyhow::anyhow!("group missing during actor registry build"))?;
            groups[group_index[index].1].members.push(record.id);
        }
        Ok(Self {
            input_rows,
            records,
            duplicates,
            groups,
            group_index,
            member_count,
        })
    }

    pub fn input_rows(&self) -> usize {
        self.input_rows
    }
    pub fn record_count(&self) -> usize {
        self.records.len()
    }
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }
    pub fn member_count(&self) -> usize {
        self.member_count
    }
    pub fn records(&self) -> &[Record] {
        &self.records
    }
    pub fn groups(&self) -> &[Group] {
        &self.groups
    }
    pub fn duplicates(&self) -> &[Duplicate] {
        &self.duplicates
    }

    pub fn record(&self, id: RecordId) -> Result<&Record> {
        let index = usize::try_from(id.get() - 1)?;
        self.records
            .get(index)
            .context("actor record ID is outside this registry")
    }

    /// Lookup work is logarithmic and capped by host pointer width by default.
    pub fn selection_mut(&mut self, reference: u32) -> Result<Selection> {
        self.selection_with_budget(
            reference,
            LookupBudget {
                comparisons: usize::BITS as usize,
            },
        )
    }

    /// Errors leave group cursors unchanged. A missing source dependency is an
    /// explicit outcome, never an invented default/special record or NULL.
    pub fn selection_with_budget(
        &mut self,
        reference: u32,
        budget: LookupBudget,
    ) -> Result<Selection> {
        let low = reference as u16;
        if reference >> 16 != 0x4d00 || low == 0 {
            return Ok(Selection::RequiresDefault {
                reason: DefaultReason::InvalidReference { reference },
            });
        }
        let special = match reference {
            0x4d000102 => Some(SpecialKind::Embedded102),
            0x4d0001ac => Some(SpecialKind::Static1ac),
            _ => None,
        };
        if let Some(kind) = special {
            return Ok(Selection::RequiresSpecial { kind });
        }
        if low < 0x100 {
            let Some(index) = bounded_find(self.group_index.len(), budget, |index| {
                self.group_index[index].0.cmp(&low)
            })?
            else {
                return Ok(Selection::RequiresDefault {
                    reason: DefaultReason::MissingGroup { key: low },
                });
            };
            let group_index = self.group_index[index].1;
            let group = &self.groups[group_index];
            ensure!(
                group.count != 0 && group.members.len() == group.count,
                "actor group has invalid member storage"
            );
            let member_index = if group.cursor >= group.count {
                0
            } else {
                group.cursor
            };
            let record = group.members[member_index];
            self.record(record)?;
            let next = member_index
                .checked_add(1)
                .context("actor group cursor overflow")?;
            self.groups[group_index].cursor = next;
            Ok(Selection::Selected {
                record,
                group: Some(low),
                member_index: Some(member_index),
            })
        } else {
            let index = bounded_find(self.records.len(), budget, |index| {
                self.records[index].key().cmp(&reference)
            })?;
            Ok(match index {
                Some(index) => Selection::Selected {
                    record: self.records[index].id,
                    group: None,
                    member_index: None,
                },
                None => Selection::RequiresDefault {
                    reason: DefaultReason::MissingRecord { reference },
                },
            })
        }
    }
}

fn bounded_find(
    len: usize,
    budget: LookupBudget,
    mut compare: impl FnMut(usize) -> Ordering,
) -> Result<Option<usize>> {
    let (mut lower, mut upper, mut comparisons) = (0, len, 0usize);
    while lower < upper {
        ensure!(
            comparisons < budget.comparisons,
            "actor lookup comparison budget exceeded"
        );
        comparisons += 1;
        // Lower midpoint matches the source inclusive-end unsigned search.
        let middle = lower + (upper - lower - 1) / 2;
        match compare(middle) {
            Ordering::Equal => return Ok(Some(middle)),
            Ordering::Less => lower = middle + 1,
            Ordering::Greater => upper = middle,
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "tests/registry.rs"]
mod tests;
