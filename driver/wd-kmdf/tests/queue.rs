use wd_kmdf::filter_eval::DriverEvent;
use wd_kmdf::queue::EventQueue;

#[test]
fn queue_drops_oldest_when_capacity_is_hit() {
    let mut queue = EventQueue::new(2);
    queue.push(DriverEvent::reflect_open(1));
    queue.push(DriverEvent::reflect_close(1));
    queue.push(DriverEvent::reflect_open(2));

    assert_eq!(queue.len(), 2);
}
