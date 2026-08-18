use dry_core::{NodeId, ProvenanceMap, SegmentSpan};

#[test]
fn test_provenance_map_insertion_and_lookup() {
    let mut map = ProvenanceMap::new();

    let node_flange = NodeId::new("node_flange_01");
    let node_pocket = NodeId::new("node_pocket_02");

    map.insert(node_flange.clone(), SegmentSpan::new(0, 10));
    map.insert(node_pocket.clone(), SegmentSpan::new(10, 35));

    assert_eq!(map.get_span(&node_flange), Some(SegmentSpan::new(0, 10)));
    assert_eq!(map.get_span(&node_pocket), Some(SegmentSpan::new(10, 35)));

    // Finding node by segment index
    assert_eq!(map.find_node_for_segment(5), Some(&node_flange));
    assert_eq!(map.find_node_for_segment(10), Some(&node_pocket));
    assert_eq!(map.find_node_for_segment(34), Some(&node_pocket));
    assert_eq!(map.find_node_for_segment(35), None);
}
