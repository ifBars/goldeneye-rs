//! Source class2/subtype12 constructor and its four ordered map-init scans.
//! Supplied providers own the actual selected-level context, shared RNG, and
//! actor records. A provider's absence is not a fabricated source NULL.
use anyhow::{Result, ensure};

use super::{Factory, MapContext, Reference, Trigger, TriggerId, Unsupported};
use crate::mission::{actor_definitions::RecordId, catalog::LevelRow, source_rng};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    pub trigger: TriggerId,
    pub map: MapContext,
    pub ordinal: u16,
    pub zone: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderOutcome<T> {
    Ready(T),
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderStage {
    DescriptorFallback,
    LevelSelector,
    Definition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructorStage {
    Provider(ProviderStage),
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionBranch {
    AbsentContext,
    Direct { selector: u8 },
    Random { selector: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelSelection {
    pub word: u32,
    pub branch: SelectionBranch,
    pub sentinel_fallback: bool,
}

impl LevelSelection {
    pub fn validate(self) -> Result<()> {
        if let SelectionBranch::Direct { selector } | SelectionBranch::Random { selector } =
            self.branch
        {
            ensure!(
                (1..=6).contains(&selector),
                "level selector is outside owned six-word context"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelOutcome {
    Selected(LevelSelection),
    RequiresRng,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelContext {
    Absent,
    Selected {
        descriptor_fallback: u32,
        selector_words: [u32; 6],
    },
}

impl LevelContext {
    pub fn from_level_row(row: Option<&LevelRow>) -> Self {
        match row {
            None => Self::Absent,
            Some(row) => {
                let words = row.words_raw();
                Self::Selected {
                    descriptor_fallback: words[0x84 / 4],
                    selector_words: std::array::from_fn(|i| words[0x8c / 4 + i]),
                }
            }
        }
    }

    pub fn descriptor_fallback(self) -> u32 {
        match self {
            Self::Absent => 0x4d00_0002,
            Self::Selected {
                descriptor_fallback,
                ..
            } => descriptor_fallback,
        }
    }

    pub fn select(
        self,
        token: u32,
        rng: Option<&mut source_rng::SourceRngOwner>,
        budget: source_rng::Budget,
    ) -> Result<LevelOutcome> {
        let Self::Selected { selector_words, .. } = self else {
            return Ok(LevelOutcome::Selected(LevelSelection {
                word: 0x4b00_000d,
                branch: SelectionBranch::AbsentContext,
                sentinel_fallback: false,
            }));
        };
        let valid = super::numeric::reference(token, 0x4b30);
        let (selector, branch) = if valid != 0 {
            let selector = valid & 0xffff;
            ensure!(
                (1..=6).contains(&selector),
                "level selector exceeds owned source context"
            );
            let selector = selector as u8;
            (selector, SelectionBranch::Direct { selector })
        } else {
            let Some(rng) = rng else {
                return Ok(LevelOutcome::RequiresRng);
            };
            let selector = (rng.draw_u32(budget)? % 6 + 1) as u8;
            (selector, SelectionBranch::Random { selector })
        };
        let raw = selector_words[usize::from(selector - 1)];
        Ok(LevelOutcome::Selected(LevelSelection {
            word: if raw == u32::MAX { 0x4b00_000d } else { raw },
            branch,
            sentinel_fallback: raw == u32::MAX,
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapInitialization {
    NotRun,
    Blocked { scan: u8, ordinal: Option<u32> },
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScanReference {
    Null,
    Target { trigger: TriggerId, subtype: u32 },
    Blocked { ordinal: u32 },
    Unresolved,
}

#[derive(Debug, Clone)]
pub struct State {
    stage: ConstructorStage,
    current_rotation_bits: [u32; 4],
    rotation_repaired: bool,
    level_selection: Option<LevelSelection>,
    definition: Option<RecordId>,
    radius_squared_bits: Option<u32>,
    word_74: u32,
    word_78: u32,
    map_initialization: MapInitialization,
    scan_matches: [Option<TriggerId>; 4],
}

impl State {
    fn fresh(rotation: [u32; 4], word_74: u32, word_78: u32) -> Self {
        let (current_rotation_bits, rotation_repaired) = repair_rotation(rotation);
        Self {
            stage: ConstructorStage::Provider(ProviderStage::DescriptorFallback),
            current_rotation_bits,
            rotation_repaired,
            level_selection: None,
            definition: None,
            radius_squared_bits: None,
            word_74,
            word_78,
            map_initialization: MapInitialization::NotRun,
            scan_matches: [None; 4],
        }
    }

    pub fn stage(&self) -> ConstructorStage {
        self.stage
    }
    pub fn current_rotation_bits(&self) -> [u32; 4] {
        self.current_rotation_bits
    }
    pub fn rotation_repaired(&self) -> bool {
        self.rotation_repaired
    }
    pub fn level_selection(&self) -> Option<LevelSelection> {
        self.level_selection
    }
    pub fn definition(&self) -> Option<RecordId> {
        self.definition
    }
    pub fn radius_squared_bits(&self) -> Option<u32> {
        self.radius_squared_bits
    }
    pub fn word_74(&self) -> u32 {
        self.word_74
    }
    pub fn word_78(&self) -> u32 {
        self.word_78
    }
    /// Source constructor explicitly initializes byte+0x11c, not its padding.
    pub fn byte_11c(&self) -> u8 {
        0
    }
    pub fn map_initialization(&self) -> MapInitialization {
        self.map_initialization
    }
    pub fn scan_matches(&self) -> [Option<TriggerId>; 4] {
        self.scan_matches
    }

    pub(super) fn initialize_map(&mut self, references: &[ScanReference; 7]) {
        for (scan, subtype) in [0x1f00_00a5, 0x1f00_00a9, 0x1f00_00b3, 0x1f00_00a4]
            .into_iter()
            .enumerate()
        {
            let mut found = None;
            for reference in references {
                match *reference {
                    ScanReference::Null => {}
                    ScanReference::Target {
                        trigger,
                        subtype: actual,
                    } => {
                        if actual == subtype {
                            found = Some(trigger);
                            break;
                        }
                    }
                    ScanReference::Blocked { ordinal } => {
                        self.map_initialization = MapInitialization::Blocked {
                            scan: scan as u8,
                            ordinal: Some(ordinal),
                        };
                        return;
                    }
                    ScanReference::Unresolved => {
                        self.map_initialization = MapInitialization::Blocked {
                            scan: scan as u8,
                            ordinal: None,
                        };
                        return;
                    }
                }
            }
            self.scan_matches[scan] = found;
        }
        self.map_initialization = MapInitialization::Complete;
    }
}

fn repair_rotation(mut rotation: [u32; 4]) -> ([u32; 4], bool) {
    let pi = f32::from_bits(0x4049_0fdb);
    let epsilon = f32::from_bits(0x3a83_126f);
    if (f32::from_bits(rotation[0]) - pi).abs() <= epsilon
        && (f32::from_bits(rotation[2]) - pi).abs() <= epsilon
    {
        rotation[0] = 0;
        rotation[2] = 0;
        rotation[1] = (pi - f32::from_bits(rotation[1])).to_bits();
        (rotation, true)
    } else {
        (rotation, false)
    }
}

impl Trigger {
    pub(super) fn initialize_actor12(
        &mut self,
        factory: &mut impl Factory,
    ) -> Result<Option<Unsupported>> {
        self.actor12 = Some(State::fresh(
            self.common.rotation_bits,
            self.common.word_74_raw,
            self.common.word_78_raw,
        ));
        let request = Request {
            trigger: self.id,
            map: self.map,
            ordinal: self.ordinal,
            zone: self.zone,
        };
        self.runtime_numeric[0] = super::numeric::reference(self.common.numeric[0], 0x4d00);
        if self.runtime_numeric[0] == 0 {
            match factory.actor12_descriptor_fallback(request)? {
                ProviderOutcome::Ready(word) => self.runtime_numeric[0] = word,
                ProviderOutcome::Unavailable => {
                    return Ok(Some(Unsupported::Actor12Provider {
                        trigger: self.id,
                        stage: ProviderStage::DescriptorFallback,
                    }));
                }
            }
        }
        for (index, prefix) in [
            (1, 0x5220),
            (2, 0x5230),
            (3, 0x5200),
            (4, 0x0b00),
            (5, 0x5460),
        ] {
            self.runtime_numeric[index] =
                super::numeric::reference(self.common.numeric[index], prefix);
        }
        let selector = super::numeric::reference(self.common.numeric[6], 0x4b30);
        self.actor12.as_mut().expect("actor state").stage =
            ConstructorStage::Provider(ProviderStage::LevelSelector);
        let selection = match factory.actor12_level_selector(request, selector)? {
            ProviderOutcome::Ready(selection) => {
                selection.validate()?;
                selection
            }
            ProviderOutcome::Unavailable => {
                return Ok(Some(Unsupported::Actor12Provider {
                    trigger: self.id,
                    stage: ProviderStage::LevelSelector,
                }));
            }
        };
        self.runtime_numeric[6] = selection.word;
        self.runtime_numeric[7] &= 0x3f;
        self.runtime_numeric[10] = super::numeric::reference(self.common.numeric[10], 0x4200);
        let state = self.actor12.as_mut().expect("actor state");
        state.level_selection = Some(selection);
        state.word_74 = super::numeric::reference(self.common.word_74_raw, 0x01b4);
        state.word_78 = super::numeric::reference(self.common.word_78_raw, 0x0400);
        let raw = self.runtime_numeric[5];
        let scaled = if raw == 0 {
            0.0
        } else {
            super::numeric::signed_tenths(raw)
        };
        state.radius_squared_bits = Some(if scaled == 0.0 {
            f32::MAX.to_bits()
        } else {
            let repeated = super::numeric::signed_tenths(raw);
            (repeated * repeated).to_bits()
        });
        state.stage = ConstructorStage::Provider(ProviderStage::Definition);
        let reference = self.runtime_numeric[0];
        match factory.actor12_definition(request, reference)? {
            ProviderOutcome::Ready(definition) => {
                let state = self.actor12.as_mut().expect("actor state");
                state.definition = Some(definition);
                state.stage = ConstructorStage::Complete;
                Ok(None)
            }
            ProviderOutcome::Unavailable => Ok(Some(Unsupported::Actor12Provider {
                trigger: self.id,
                stage: ProviderStage::Definition,
            })),
        }
    }
}

pub(super) fn scan_reference(
    reference: Reference,
    subtype: impl FnOnce(TriggerId) -> Result<u32>,
) -> Result<ScanReference> {
    Ok(match reference {
        Reference::Null => ScanReference::Null,
        Reference::Trigger(trigger) => ScanReference::Target {
            trigger,
            subtype: subtype(trigger)?,
        },
        Reference::Blocked { ordinal } => ScanReference::Blocked { ordinal },
        Reference::Unresolved => ScanReference::Unresolved,
    })
}

#[cfg(test)]
#[path = "tests/actor12_scans.rs"]
mod tests;
