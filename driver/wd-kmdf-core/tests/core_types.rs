use wd_kmdf_core::{
    ByteRing, FixedPacket, FixedPacketError, FixedReinjectionTable, GlueIoResult, GlueIoStatus,
    HandleState, ReinjectionError, ReinjectionToken,
};

#[test]
fn handle_state_runs_expected_lifecycle() {
    let mut state = HandleState::opening();

    assert_eq!(state, HandleState::Opening);
    state.mark_running().expect("opening -> running should succeed");
    state.shutdown_recv().expect("running -> recv shutdown should succeed");
    state.shutdown_send().expect("recv shutdown -> send shutdown should succeed");
    state.close().expect("send shutdown -> closed should succeed");
    assert!(state.is_closed(), "state should report closed");
}

#[test]
fn glue_io_result_keeps_c_abi_shape() {
    let result = GlueIoResult {
        status: GlueIoStatus::InvalidHandle,
        bytes_written: 0,
    };

    assert_eq!(core::mem::size_of_val(&result), 8);
    assert_eq!(GlueIoStatus::InvalidHandle as u32, 10);
}

#[test]
fn reinjection_token_and_error_are_no_std_friendly() {
    let token = ReinjectionToken::new(77);

    assert_eq!(token.raw(), 77);
    assert_eq!(format!("{}", ReinjectionError::UnknownToken), "unknown reinjection token");
}

#[test]
fn fixed_reinjection_table_is_one_shot_and_evicts_oldest_when_full() {
    let mut table = FixedReinjectionTable::<2>::new();
    let first = table.issue_for_network_packet(11);
    let second = table.issue_for_network_packet(22);
    let third = table.issue_for_network_packet(33);

    assert_eq!(table.consume(second).expect("second token should still exist"), 22);
    assert_eq!(table.consume(third).expect("third token should still exist"), 33);
    assert_eq!(table.consume(first), Err(ReinjectionError::UnknownToken));
}

#[test]
fn byte_ring_drops_oldest_frame_when_capacity_is_hit() {
    let mut ring = ByteRing::<2, 8>::new();

    ring.push(&[1, 2]).expect("first frame should fit");
    ring.push(&[3, 4, 5]).expect("second frame should fit");
    ring.push(&[9]).expect("third frame should evict oldest");

    let mut output = [0u8; 8];
    let first_len = ring
        .pop_into(&mut output)
        .expect("pop should not fail")
        .expect("first stored frame should exist");
    assert_eq!(&output[..first_len], &[3, 4, 5]);
    let second_len = ring
        .pop_into(&mut output)
        .expect("pop should not fail")
        .expect("second stored frame should exist");
    assert_eq!(&output[..second_len], &[9]);
    assert_eq!(ring.pop_into(&mut output), Ok(None));
}

#[test]
fn byte_ring_reports_small_output_buffer_without_consuming_frame() {
    let mut ring = ByteRing::<1, 8>::new();
    ring.push(&[1, 2, 3, 4]).expect("frame should fit");

    let mut too_small = [0u8; 2];
    let err = ring
        .pop_into(&mut too_small)
        .expect_err("too-small output should be rejected");
    assert_eq!(
        err,
        wd_kmdf_core::ByteRingError::OutputTooSmall {
            required: 4,
            provided: 2,
        }
    );

    let mut output = [0u8; 8];
    let written = ring
        .pop_into(&mut output)
        .expect("second pop should not fail")
        .expect("frame should still be present");
    assert_eq!(&output[..written], &[1, 2, 3, 4]);
}

#[test]
fn fixed_packet_copies_input_and_exposes_slice_view() {
    let packet = FixedPacket::<8>::copy_from_slice(&[1, 2, 3, 4])
        .expect("packet should fit fixed storage");

    assert_eq!(packet.len(), 4);
    assert_eq!(packet.as_slice(), &[1, 2, 3, 4]);
}

#[test]
fn fixed_packet_rejects_payload_larger_than_capacity() {
    let err = FixedPacket::<4>::copy_from_slice(&[1, 2, 3, 4, 5])
        .expect_err("oversized packet should be rejected");

    assert_eq!(
        err,
        FixedPacketError::PacketTooLarge {
            required: 5,
            capacity: 4,
        }
    );
}
