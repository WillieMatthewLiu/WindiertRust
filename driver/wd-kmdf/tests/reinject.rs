use wd_kmdf::ReinjectionTable;

#[test]
fn reinjection_token_is_one_shot() {
    let mut table = ReinjectionTable::default();
    let token = table.issue_for_network_packet(7);

    assert!(table.consume(token).is_ok());
    assert!(table.consume(token).is_err());
}
