use kompact::prelude::SerId;

/* serids for Partitioning Actor messages */
pub const PARTITIONING_ID: SerId = 45;

pub const ATOMICREG_ID: SerId = 46;

pub const PING_ID: SerId = 50;
pub const PONG_ID: SerId = 51;
pub const STATIC_PING_ID: SerId = 52;
pub const STATIC_PONG_ID: SerId = 53;

pub const SW_SOURCE_ID: SerId = 54;
pub const SW_SINK_ID: SerId = 55;
pub const SW_WINDOWER_ID: SerId = 56;

pub const RAFT_ID: SerId = 57;
pub const BLE_ID: SerId = 58;
pub const ATOMICBCAST_ID: SerId = 59;
pub const PAXOS_ID: SerId = 60;
pub const RECONFIG_ID: SerId = 61;
pub const TEST_SEQ_ID: SerId = 62;
pub const STOP_ID: SerId = 63;
pub const VR_ID: SerId = 65;
pub const MP_ID: SerId = 66;

pub const MP_LEADER_ID: SerId = 67;
pub const MP_ACCEPTOR_ID: SerId = 68;
pub const MP_REPLICA_ID: SerId = 69;
pub const MP_BATCHER_ID: SerId = 70;
pub const MP_PROXY_LEADER_ID: SerId = 71;
pub const MP_PARTICIPANT_ID: SerId = 72;

#[cfg(feature = "simulate_partition")]
pub const PARTITIONING_EXP_ID: SerId = 64;
