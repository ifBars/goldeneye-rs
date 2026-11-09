//! Ordered mission-resource discovery before world registration.
//!
//! The host acknowledges actual retained resources and supplies each linked-map
//! lookup result. Discovery keeps source file-only deduplication and always
//! requests coded map index zero, independently of the returned link's map.
//! Reaching a discovery stop does not mean that a gameplay world is ready.
use anyhow::{Result, bail, ensure};

pub const MAP_REFERENCE: u32 = 0x8500_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub files: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    pub file: u32,
    pub map_reference: u32,
}

impl Request {
    fn for_file(file: u32) -> Self {
        Self {
            file,
            map_reference: MAP_REFERENCE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    pub file: u32,
    /// Retained in the callback contract, but ignored when selecting the next
    /// request and testing previously appended files.
    pub map: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    NoLink,
    RepeatedFile { file: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    FileBudgetExhausted { limit: usize },
    ResourceUnavailable { file: u32 },
    LinkLookupFailed { file: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    WaitingForResource(Request),
    WaitingForLink(Request),
    Complete(Completion),
    Failed(Failure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovery {
    budget: Budget,
    state: State,
    entries: Vec<Request>,
}

impl Discovery {
    pub fn new(root_file: u32, budget: Budget) -> Result<Self> {
        ensure!(
            budget.files > 0,
            "mission residency file budget excludes the root resource"
        );
        Ok(Self {
            budget,
            state: State::WaitingForResource(Request::for_file(root_file)),
            entries: Vec::new(),
        })
    }

    pub fn state(&self) -> State {
        self.state
    }

    /// Source discovery order, containing only resources acknowledged as ready
    /// by the host. This is not the final world manager registration order.
    pub fn entries(&self) -> &[Request] {
        &self.entries
    }

    /// The host calls this only after retaining the requested resource and
    /// resolving its requested coded map. Append precedes linked-map lookup.
    pub fn resource_ready(&mut self, request: Request) -> Result<()> {
        self.expect(State::WaitingForResource(request))?;
        if self.entries.len() >= self.budget.files {
            self.state = State::Failed(Failure::FileBudgetExhausted {
                limit: self.budget.files,
            });
            bail!("mission residency file budget exhausted");
        }
        self.entries.push(request);
        self.state = State::WaitingForLink(request);
        Ok(())
    }

    /// Resolve only the current retained resource's lookup. Missing matches and
    /// repeated files are different source stop reasons. A missing file-property
    /// sentinel remains an ordinary unseen file request; it is not a stop.
    pub fn linked(&mut self, request: Request, link: Option<Link>) -> Result<State> {
        self.expect(State::WaitingForLink(request))?;
        let Some(link) = link else {
            self.state = State::Complete(Completion::NoLink);
            return Ok(self.state);
        };
        if self.entries.iter().any(|entry| entry.file == link.file) {
            self.state = State::Complete(Completion::RepeatedFile { file: link.file });
            return Ok(self.state);
        }
        if self.entries.len() >= self.budget.files {
            self.state = State::Failed(Failure::FileBudgetExhausted {
                limit: self.budget.files,
            });
            bail!("mission residency file budget exhausted");
        }
        self.state = State::WaitingForResource(Request::for_file(link.file));
        Ok(self.state)
    }

    pub fn resource_failed(&mut self, request: Request) -> Result<()> {
        self.expect(State::WaitingForResource(request))?;
        self.state = State::Failed(Failure::ResourceUnavailable { file: request.file });
        Ok(())
    }

    pub fn link_failed(&mut self, request: Request) -> Result<()> {
        self.expect(State::WaitingForLink(request))?;
        self.state = State::Failed(Failure::LinkLookupFailed { file: request.file });
        Ok(())
    }

    fn expect(&self, expected: State) -> Result<()> {
        ensure!(
            self.state == expected,
            "mission residency callback does not match its pending state/request"
        );
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/residency.rs"]
mod tests;
