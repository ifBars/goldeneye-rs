use super::*;
use crate::{
    animation::scratch::evaluation::Step,
    mission::player::components::temporary::prepare::{Pause, Prepare, Return},
};

// Authored owner-bound skin reads; no original skin data.
struct Reads {
    lookup: SkinLookup,
    body: SkinBody,
    generation: u32,
    calls: Vec<&'static str>,
}
impl SkinSource for Reads {
    fn lookup(&mut self, _: ResourceBinding) -> Result<SkinLookup> {
        self.calls.push("lookup");
        Ok(self.lookup)
    }
    fn generation(&mut self, _: ResourceBinding) -> Result<u32> {
        self.calls.push("generation");
        Ok(self.generation)
    }
    fn body(&mut self, _: SkinDescriptor) -> Result<SkinBody> {
        self.calls.push("body");
        Ok(self.body)
    }
}
const PRIMARY: ResourceBinding = ResourceBinding {
    file: 0x010a_0037,
    reference: 0x0d00_016f,
};
fn present() -> SkinLookup {
    SkinLookup::Present(SkinDescriptor {
        file: PRIMARY.file,
        reference: PRIMARY.reference,
        generation: 3,
    })
}
fn reads(lookup: SkinLookup, body: SkinBody) -> Reads {
    Reads {
        lookup,
        body,
        generation: 3,
        calls: Vec::new(),
    }
}

#[test]
fn completed_lookup_is_retained_across_a_body_pause_and_generation_is_rechecked() {
    let mut prepare = Prepare::new(PRIMARY);
    let mut source = reads(present(), SkinBody::Pending);
    assert_eq!(prepare.advance(&mut source, 0).unwrap(), Step::Pending);
    assert_eq!(prepare.pause(), Some(Pause::Body));
    source.body = SkinBody::Resident;
    assert_eq!(prepare.advance(&mut source, 0).unwrap(), Step::Done(()));
    assert_eq!(source.calls, ["lookup", "body", "generation", "body"]);
    assert_eq!((prepare.lookups(), prepare.body_reads()), (1, 2));
    assert_eq!(prepare.returned(), Some(Return::AuxiliaryZero));
    assert_eq!(prepare.pause(), None);
    assert!(prepare.advance(&mut source, 0).is_err());
}

#[test]
fn unknown_lookup_is_not_absent_and_nonzero_auxiliary_pauses_after_body() {
    let mut prepare = Prepare::new(PRIMARY);
    let mut source = reads(SkinLookup::Pending, SkinBody::Resident);
    assert_eq!(prepare.advance(&mut source, 0).unwrap(), Step::Pending);
    assert_eq!(prepare.pause(), Some(Pause::Lookup));
    assert_eq!(prepare.descriptor(), None);
    assert_eq!(prepare.returned(), None);
    source.lookup = present();
    assert_eq!(prepare.advance(&mut source, 2).unwrap(), Step::Pending);
    assert_eq!(prepare.pause(), Some(Pause::Auxiliary));
    assert!(prepare.body_resident());
    assert_eq!(prepare.auxiliary_words(), Some(2));
    // Resuming the unimplemented clear re-reads neither lookup nor body.
    let calls = source.calls.len();
    assert_eq!(prepare.advance(&mut source, 2).unwrap(), Step::Pending);
    assert_eq!(source.calls.len(), calls);
}

#[test]
fn foreign_descriptor_fails_before_body_read() {
    let mut prepare = Prepare::new(PRIMARY);
    let mut source = reads(
        SkinLookup::Present(SkinDescriptor {
            file: PRIMARY.file + 1,
            reference: PRIMARY.reference,
            generation: 3,
        }),
        SkinBody::Resident,
    );
    assert!(prepare.advance(&mut source, 0).is_err());
    assert_eq!(source.calls, ["lookup"]);
    assert_eq!(prepare.descriptor(), None);
}
