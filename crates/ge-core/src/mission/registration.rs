//! Registration ordering and row flags from an explicit preference snapshot.
//!
//! Discovery pairs remain separate from this plan. This models no resource
//! loading, map-instance construction, or completed world registration. Source
//! offsets name preference groups and row bytes whose wider roles remain open.
use super::residency::Request;
use anyhow::{Context, Result, ensure};

/// Caller-supplied state, not an inferred native initializer/default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preferences {
    pub prefixes_98c: Vec<u32>,
    pub prefixes_9ac: Vec<u32>,
    /// Preserved snapshot data. This group is not consulted by this planner.
    pub prefixes_9cc: Vec<u32>,
    pub filter_9ec: Vec<u32>,
    pub filter_a2c: Vec<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub rows: usize,
    /// Host ownership limit across all five preference vectors, including the
    /// preserved unused group. This preflight is separate from source lookup.
    pub preference_words: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterSet {
    At9ec,
    AtA2c,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    source_index: usize,
    request: Request,
    flags_raw: [u8; 3],
    filter: Option<FilterSet>,
}

impl Row {
    pub fn source_index(&self) -> usize {
        self.source_index
    }

    pub fn request(&self) -> Request {
        self.request
    }

    /// Native row bytes +0x10, +0x11, +0x12, without invented flag meanings.
    pub fn flags_raw(&self) -> [u8; 3] {
        self.flags_raw
    }

    pub fn filter(&self) -> Option<FilterSet> {
        self.filter
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationPlan {
    incoming: Vec<Request>,
    rows: Vec<Row>,
    preferences: Preferences,
}

impl RegistrationPlan {
    pub fn build(incoming: &[Request], preferences: Preferences, budget: Budget) -> Result<Self> {
        ensure!(
            !incoming.is_empty(),
            "empty registration plan would address a missing native row"
        );
        ensure!(
            incoming.len() <= budget.rows,
            "registration row budget exhausted"
        );
        let preference_words = [
            preferences.prefixes_98c.len(),
            preferences.prefixes_9ac.len(),
            preferences.prefixes_9cc.len(),
            preferences.filter_9ec.len(),
            preferences.filter_a2c.len(),
        ]
        .into_iter()
        .try_fold(0usize, |total, count| total.checked_add(count))
        .context("registration preference size overflow")?;
        ensure!(
            preference_words <= budget.preference_words,
            "registration preference budget exhausted"
        );
        let mut rows: Vec<_> = incoming
            .iter()
            .copied()
            .enumerate()
            .map(|(source_index, request)| Row {
                source_index,
                request,
                flags_raw: [0xff, 0, 0],
                filter: None,
            })
            .collect();
        if rows.len() == 1 {
            // Native singleton handling does not search any preference group.
            rows[0].flags_raw = [0xff, 1, 1];
        } else {
            let first = first_match(&rows, &preferences.prefixes_98c).context(
                "registration has no preferred row; native negative-index access is unsupported",
            )?;
            if first != 1 {
                let moved = rows.remove(first);
                rows.insert(1, moved);
            }
            // Rescan after reordering: the newly first match need not be the
            // record moved to index one when multiple rows match this group.
            let at_9ac = first_match(&rows, &preferences.prefixes_9ac);
            let at_98c = first_match(&rows, &preferences.prefixes_98c);
            for (index, row) in rows.iter_mut().enumerate() {
                row.flags_raw[2] = 1;
                if Some(index) == at_98c {
                    row.flags_raw[1] = 1;
                    row.filter = at_9ac.map(|_| FilterSet::At9ec);
                } else if Some(index) != at_9ac {
                    row.filter = Some(FilterSet::AtA2c);
                }
            }
        }
        Ok(Self {
            incoming: incoming.to_vec(),
            rows,
            preferences,
        })
    }

    pub fn incoming(&self) -> &[Request] {
        &self.incoming
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn preferences(&self) -> &Preferences {
        &self.preferences
    }

    pub fn filter_values(&self, filter: FilterSet) -> &[u32] {
        match filter {
            FilterSet::At9ec => &self.preferences.filter_9ec,
            FilterSet::AtA2c => &self.preferences.filter_a2c,
        }
    }

    /// A filter selector resolves to an owned vector in this plan, never a
    /// reconstructed native pointer. Some(empty) stays distinct from None.
    pub fn row_filter(&self, index: usize) -> Result<Option<&[u32]>> {
        let row = self
            .rows
            .get(index)
            .context("registration row index is absent")?;
        Ok(row.filter.map(|filter| self.filter_values(filter)))
    }
}

fn first_match(rows: &[Row], prefixes: &[u32]) -> Option<usize> {
    // Rows are outermost. Preference order cannot select a later row over an
    // earlier matching row. Only the file's low half must be nonzero.
    rows.iter().position(|row| {
        row.request.file & 0xffff != 0
            && prefixes
                .iter()
                .any(|prefix| prefix & 0xffff_0000 == row.request.file & 0xffff_0000)
    })
}

#[cfg(test)]
#[path = "tests/registration.rs"]
mod tests;
