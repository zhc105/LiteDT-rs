use std::time::Instant;

struct PacketEntry {
    delivered: u32,
    flow: u32,
    seq: Seq32,
    is_app_limited: bool,
    retrans_round: u32,
    sent_time: Instant,
    rto_time: Instant ,
    delivered_time: Option<Instant>,
    first_tx_time: Option<Instant>,
}
