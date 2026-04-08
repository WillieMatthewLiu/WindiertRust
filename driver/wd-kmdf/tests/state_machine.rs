use wd_kmdf::HandleState;

#[test]
fn handle_state_machine_enforces_shutdown_order() {
    let mut state = HandleState::opening();
    state.mark_running().unwrap();
    state.shutdown_recv().unwrap();
    state.shutdown_send().unwrap();
    state.close().unwrap();
    assert!(state.is_closed());
}

#[test]
fn handle_state_rejects_invalid_transition() {
    let mut state = HandleState::opening();
    assert!(state.shutdown_recv().is_err());
}
