use crate::bench::atomic_broadcast::{messages::Proposal, multi_paxos::util::Round};
use std::fmt::Debug;

/// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/MultiPaxos.proto

pub type ProposalType = Vec<u8>;

#[derive(Debug, Clone, Copy)]
pub struct Phase1a {
    pub src: u64,
    pub round: u64,
    pub chosen_watermark: u64,
}

#[derive(Debug, Clone)]
pub enum CommandBatchOrNoop {
    CommandBatch(Vec<ProposalType>),
    Noop,
}

#[derive(Debug, Clone)]
pub struct Phase1bSlotInfo {
    pub slot: u64,
    pub vote_round: u64,
    pub vote_value: CommandBatchOrNoop,
}

#[derive(Debug, Clone)]
pub struct Phase1b {
    // pub group_index: u64,
    pub acceptor_index: u64,
    pub round: u64,
    pub info: Vec<Phase1bSlotInfo>,
}

#[derive(Debug, Clone)]
pub struct Phase2a {
    pub src: u64,
    pub slot: u64,
    pub round: u64,
    pub command_batch_or_noop: CommandBatchOrNoop,
}

#[derive(Debug, Clone, Copy)]
pub struct Phase2b {
    // pub group_index: u64,
    pub acceptor_index: u64,
    pub slot: u64,
    pub round: u64,
}

#[derive(Debug, Clone)]
pub struct Chosen {
    pub slot: u64,
    pub command_batch_or_noop: CommandBatchOrNoop,
}

#[derive(Debug, Clone, Copy)]
pub struct Nack {
    pub round: u64,
}
#[derive(Debug, Clone, Copy)]
pub struct ChosenWatermark {
    pub slot: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct Recover {
    pub slot: u64,
}

#[derive(Debug, Clone)]
pub enum ProxyLeaderInbound {
    Phase2a(Phase2a),
    Phase2b(Phase2b),
}

#[derive(Debug, Clone)]
pub enum LeaderInbound {
    Phase1b(Phase1b),
    Nack(Nack),
    ChosenWatermark(ChosenWatermark),
    Recover(Recover),
    /*** For batcher ***/
    ClientRequestBatch(Vec<Proposal>),
    LeaderInfoRequestBatcher(u64), // pid is needed for simulate partition experiments
}

#[derive(Debug, Clone)]
pub enum AcceptorInbound {
    Phase1a(Phase1a),
    Phase2a(Phase2a),
}

#[derive(Debug, Clone)]
pub enum ReplicaInbound {
    Chosen(Chosen),
}

#[derive(Debug, Clone)]
pub enum BatcherInbound {
    NotLeaderBatcher(Vec<Proposal>),
    LeaderInfoReplyBatcher(Round),
}
