use super::*;
#[test]
fn intrusive_links_distinguish_unlinked_nodes_and_reject_wrong_slot_or_nonhead_sources() {
    let mut ids = Ids::from_constructor(OwnerId::new(4).unwrap(), 8);
    let a = ids.allocate(Kind::Control).unwrap();
    let b = ids.allocate(Kind::Script).unwrap();
    let c = ids.allocate(Kind::Script).unwrap();
    assert!(ids.links(a).is_none());
    ids.link_head(2, a, None).unwrap();
    assert!(ids.link_head(1, b, Some(a)).is_err());
    assert!(ids.links(b).is_none());
    assert_eq!(ids.links(a).unwrap().previous, None);
    ids.link_head(2, b, Some(a)).unwrap();
    assert_eq!(ids.links(a).unwrap().previous, Some(b));
    assert!(ids.link_head(2, c, Some(a)).is_err());
    assert!(ids.links(c).is_none());
    ids.link_head(2, c, Some(b)).unwrap();
    assert_eq!(ids.links(c).unwrap().next, Some(b));
    assert!(ids.link_head(2, c, Some(b)).is_err());
}

#[test]
fn unlink_patches_neighbors_head_to_tail_and_retired_ids_are_never_reused() {
    let mut ids = Ids::from_constructor(OwnerId::new(4).unwrap(), 8);
    let tail = ids.allocate(Kind::Script).unwrap();
    let middle = ids.allocate(Kind::Script).unwrap();
    let head = ids.allocate(Kind::Script).unwrap();
    ids.link_head(0, tail, None).unwrap();
    ids.link_head(0, middle, Some(tail)).unwrap();
    ids.link_head(0, head, Some(middle)).unwrap();
    // Linked nodes cannot be retired; unlink returns the pre-patch links.
    assert!(ids.retire(middle).is_err());
    let links = ids.unlink(middle).unwrap();
    assert_eq!((links.previous, links.next), (Some(head), Some(tail)));
    assert_eq!(ids.links(head).unwrap().next, Some(tail));
    assert_eq!(ids.links(tail).unwrap().previous, Some(head));
    assert!(ids.links(middle).is_none());
    assert!(ids.unlink(middle).is_err());
    assert_eq!(ids.retire(middle).unwrap(), Kind::Script);
    assert_eq!(ids.kind(middle), None);
    assert!(ids.retire(middle).is_err());
    let unlinked_head = ids.unlink(head).unwrap();
    assert_eq!(unlinked_head.next, Some(tail));
    assert_eq!(ids.links(tail).unwrap().previous, None);
    let fresh = ids.allocate(Kind::Script).unwrap();
    assert!(fresh.get() > head.get());
}
