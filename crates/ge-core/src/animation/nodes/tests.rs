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
