use super::messages::*;
use crate::{bench::atomic_broadcast::messages::Proposal, serialiser_ids};
use kompact::prelude::*;

pub struct LeaderInboundSer;
pub struct AcceptorInboundSer;
pub struct ReplicaInboundSer;
pub struct BatcherInboundSer;
pub struct ProxyLeaderInboundSer;

const LI_P1B: u8 = 0;
const LI_NACK: u8 = 1;
const LI_CW: u8 = 2;
const LI_R: u8 = 3;
const LI_CRB: u8 = 4;
const LI_LIRB: u8 = 5;

const AI_P1A: u8 = 0;
const AI_P2A: u8 = 1;

const BI_NLP: u8 = 0;
const BI_LIRB: u8 = 1;

const PI_P2A: u8 = 0;
const PI_P2B: u8 = 1;

fn serialise_command_batch_or_noop(c: &CommandBatchOrNoop, buf: &mut dyn BufMut) {
    match c {
        CommandBatchOrNoop::CommandBatch(c) => {
            buf.put_u8(1);
            buf.put_u32(c.len() as u32);
            for cmd in c {
                let data = cmd.as_slice();
                buf.put_u32(data.len() as u32);
                buf.put_slice(data);
            }
        }
        CommandBatchOrNoop::Noop => buf.put_u8(0),
    }
}

fn deserialise_command_batch_or_noop(buf: &mut dyn Buf) -> CommandBatchOrNoop {
    match buf.get_u8() {
        0 => CommandBatchOrNoop::Noop,
        1 => {
            let len = buf.get_u32();
            let mut batch = Vec::with_capacity(len as usize);
            for _ in 0..len {
                let data_len = buf.get_u32() as usize;
                let mut data = vec![0; data_len];
                buf.copy_to_slice(&mut data);
                batch.push(data);
            }
            CommandBatchOrNoop::CommandBatch(batch)
        }
        _ => unimplemented!("unexpected command batch value"),
    }
}

fn serialise_phase1b_slot_info(p1b_slot_info: &Phase1bSlotInfo, buf: &mut dyn BufMut) {
    buf.put_u64(p1b_slot_info.slot);
    buf.put_u64(p1b_slot_info.vote_round);
    serialise_command_batch_or_noop(&p1b_slot_info.vote_value, buf);
}

fn deserialise_phase1b_slot_info(buf: &mut dyn Buf) -> Phase1bSlotInfo {
    let slot = buf.get_u64();
    let vote_round = buf.get_u64();
    let vote_value = deserialise_command_batch_or_noop(buf);
    Phase1bSlotInfo {
        slot,
        vote_round,
        vote_value,
    }
}

impl Serialisable for LeaderInbound {
    fn ser_id(&self) -> u64 {
        serialiser_ids::MP_LEADER_ID
    }

    fn size_hint(&self) -> Option<usize> {
        None
    }

    fn serialise(&self, buf: &mut dyn BufMut) -> Result<(), SerError> {
        match self {
            LeaderInbound::Phase1b(p1b) => {
                buf.put_u8(LI_P1B);
                buf.put_u64(p1b.round);
                buf.put_u64(p1b.acceptor_index);
                buf.put_u32(p1b.info.len() as u32);
                for info in &p1b.info {
                    serialise_phase1b_slot_info(info, buf);
                }
            }
            LeaderInbound::Nack(n) => {
                buf.put_u8(LI_NACK);
                buf.put_u64(n.round);
            }
            LeaderInbound::ChosenWatermark(c) => {
                buf.put_u8(LI_CW);
                buf.put_u64(c.slot);
            }
            LeaderInbound::Recover(r) => {
                buf.put_u8(LI_R);
                buf.put_u64(r.slot);
            }
            LeaderInbound::ClientRequestBatch(c) => {
                buf.put_u8(LI_CRB);
                buf.put_u32(c.len() as u32);
                for proposal in c {
                    buf.put_u32(proposal.data.len() as u32);
                    buf.put_slice(proposal.data.as_slice())
                }
            }
            LeaderInbound::LeaderInfoRequestBatcher(src) => {
                buf.put_u8(LI_LIRB);
                buf.put_u64(*src)
            }
        }
        Ok(())
    }

    fn local(self: Box<Self>) -> Result<Box<dyn Any + Send>, Box<dyn Serialisable>> {
        todo!()
    }
}

impl Deserialiser<LeaderInbound> for LeaderInboundSer {
    const SER_ID: SerId = serialiser_ids::MP_LEADER_ID;

    fn deserialise(buf: &mut dyn Buf) -> Result<LeaderInbound, SerError> {
        let leader_inbound = match buf.get_u8() {
            LI_P1B => {
                let round = buf.get_u64();
                let acceptor_index = buf.get_u64();
                let info_len = buf.get_u32() as usize;
                let mut p1b_infos = Vec::with_capacity(info_len);
                for _ in 0..info_len {
                    let p1b_info = deserialise_phase1b_slot_info(buf);
                    p1b_infos.push(p1b_info);
                }
                LeaderInbound::Phase1b(Phase1b {
                    acceptor_index,
                    round,
                    info: p1b_infos,
                })
            }
            LI_NACK => {
                let round = buf.get_u64();
                LeaderInbound::Nack(Nack { round })
            }
            LI_CW => {
                let slot = buf.get_u64();
                LeaderInbound::ChosenWatermark(ChosenWatermark { slot })
            }
            LI_R => {
                let slot = buf.get_u64();
                LeaderInbound::Recover(Recover { slot })
            }
            LI_CRB => {
                let len = buf.get_u32() as usize;
                let mut proposals = Vec::with_capacity(len);
                for _ in 0..len {
                    let data_len = buf.get_u32() as usize;
                    let mut data = vec![0; data_len];
                    buf.copy_to_slice(data.as_mut_slice());
                    proposals.push(Proposal { data });
                }
                LeaderInbound::ClientRequestBatch(proposals)
            }
            LI_LIRB => {
                let src = buf.get_u64();
                LeaderInbound::LeaderInfoRequestBatcher(src)
            }
            e => {
                return Err(SerError::InvalidType(
                    format!("Found unkown id: {} but expected LeaderInbound", e).into(),
                ));
            }
        };
        Ok(leader_inbound)
    }
}

impl Serialisable for AcceptorInbound {
    fn ser_id(&self) -> u64 {
        serialiser_ids::MP_ACCEPTOR_ID
    }

    fn size_hint(&self) -> Option<usize> {
        None
    }

    fn serialise(&self, buf: &mut dyn BufMut) -> Result<(), SerError> {
        match self {
            AcceptorInbound::Phase1a(p1a) => {
                buf.put_u8(AI_P1A);
                buf.put_u64(p1a.src);
                buf.put_u64(p1a.round);
                buf.put_u64(p1a.chosen_watermark);
            }
            AcceptorInbound::Phase2a(p2a) => {
                buf.put_u8(AI_P2A);
                buf.put_u64(p2a.src);
                buf.put_u64(p2a.round);
                buf.put_u64(p2a.slot);
                serialise_command_batch_or_noop(&p2a.command_batch_or_noop, buf);
            }
        }
        Ok(())
    }

    fn local(self: Box<Self>) -> Result<Box<dyn Any + Send>, Box<dyn Serialisable>> {
        todo!()
    }
}

impl Deserialiser<AcceptorInbound> for AcceptorInboundSer {
    const SER_ID: SerId = serialiser_ids::MP_ACCEPTOR_ID;

    fn deserialise(buf: &mut dyn Buf) -> Result<AcceptorInbound, SerError> {
        let ap = match buf.get_u8() {
            AI_P1A => {
                let src = buf.get_u64();
                let round = buf.get_u64();
                let chosen_watermark = buf.get_u64();
                AcceptorInbound::Phase1a(Phase1a {
                    src,
                    round,
                    chosen_watermark,
                })
            }
            AI_P2A => {
                let src = buf.get_u64();
                let round = buf.get_u64();
                let slot = buf.get_u64();
                let command_batch_or_noop = deserialise_command_batch_or_noop(buf);
                AcceptorInbound::Phase2a(Phase2a {
                    src,
                    round,
                    slot,
                    command_batch_or_noop,
                })
            }
            e => {
                return Err(SerError::InvalidType(
                    format!("Found unkown id: {} but expected AcceptorInbound", e).into(),
                ));
            }
        };
        Ok(ap)
    }
}

impl Serialisable for ReplicaInbound {
    fn ser_id(&self) -> u64 {
        serialiser_ids::MP_REPLICA_ID
    }

    fn size_hint(&self) -> Option<usize> {
        None
    }

    fn serialise(&self, buf: &mut dyn BufMut) -> Result<(), SerError> {
        match self {
            ReplicaInbound::Chosen(c) => {
                buf.put_u64(c.slot);
                serialise_command_batch_or_noop(&c.command_batch_or_noop, buf);
            }
        }
        Ok(())
    }

    fn local(self: Box<Self>) -> Result<Box<dyn Any + Send>, Box<dyn Serialisable>> {
        todo!()
    }
}

impl Deserialiser<ReplicaInbound> for ReplicaInboundSer {
    const SER_ID: SerId = serialiser_ids::MP_REPLICA_ID;

    fn deserialise(buf: &mut dyn Buf) -> Result<ReplicaInbound, SerError> {
        let slot = buf.get_u64();
        let command_batch_or_noop = deserialise_command_batch_or_noop(buf);
        Ok(ReplicaInbound::Chosen(Chosen {
            slot,
            command_batch_or_noop,
        }))
    }
}

impl Serialisable for BatcherInbound {
    fn ser_id(&self) -> u64 {
        serialiser_ids::MP_BATCHER_ID
    }

    fn size_hint(&self) -> Option<usize> {
        None
    }

    fn serialise(&self, buf: &mut dyn BufMut) -> Result<(), SerError> {
        match self {
            BatcherInbound::NotLeaderBatcher(proposals) => {
                buf.put_u8(BI_NLP);
                buf.put_u32(proposals.len() as u32);
                for proposal in proposals {
                    let data_len = proposal.data.len();
                    buf.put_u32(data_len as u32);
                    buf.put_slice(&proposal.data);
                }
            }
            BatcherInbound::LeaderInfoReplyBatcher(l) => {
                buf.put_u8(BI_LIRB);
                buf.put_u64(*l);
            }
        }
        Ok(())
    }

    fn local(self: Box<Self>) -> Result<Box<dyn Any + Send>, Box<dyn Serialisable>> {
        todo!()
    }
}

impl Deserialiser<BatcherInbound> for BatcherInboundSer {
    const SER_ID: SerId = serialiser_ids::MP_BATCHER_ID;

    fn deserialise(buf: &mut dyn Buf) -> Result<BatcherInbound, SerError> {
        let b = match buf.get_u8() {
            BI_NLP => {
                let proposals_len = buf.get_u32() as usize;
                let mut proposals = Vec::with_capacity(proposals_len);
                for _ in 0..proposals_len {
                    let data_len = buf.get_u32() as usize;
                    let mut data = vec![0; data_len];
                    buf.copy_to_slice(data.as_mut_slice());
                    proposals.push(Proposal { data })
                }
                BatcherInbound::NotLeaderBatcher(proposals)
            }
            BI_LIRB => {
                let round = buf.get_u64();
                BatcherInbound::LeaderInfoReplyBatcher(round)
            }
            e => {
                return Err(SerError::InvalidType(
                    format!("Found unkown id: {} but expected BatcherInbound", e).into(),
                ));
            }
        };
        Ok(b)
    }
}

impl Serialisable for ProxyLeaderInbound {
    fn ser_id(&self) -> u64 {
        serialiser_ids::MP_PROXY_LEADER_ID
    }

    fn size_hint(&self) -> Option<usize> {
        None
    }

    fn serialise(&self, buf: &mut dyn BufMut) -> Result<(), SerError> {
        match self {
            ProxyLeaderInbound::Phase2a(p2a) => {
                buf.put_u8(PI_P2A);
                buf.put_u64(p2a.src);
                buf.put_u64(p2a.slot);
                buf.put_u64(p2a.round);
                serialise_command_batch_or_noop(&p2a.command_batch_or_noop, buf);
            }
            ProxyLeaderInbound::Phase2b(p2b) => {
                buf.put_u8(PI_P2B);
                buf.put_u64(p2b.slot);
                buf.put_u64(p2b.round);
                buf.put_u64(p2b.acceptor_index);
            }
        }
        Ok(())
    }

    fn local(self: Box<Self>) -> Result<Box<dyn Any + Send>, Box<dyn Serialisable>> {
        todo!()
    }
}

impl Deserialiser<ProxyLeaderInbound> for ProxyLeaderInboundSer {
    const SER_ID: SerId = serialiser_ids::MP_PROXY_LEADER_ID;

    fn deserialise(buf: &mut dyn Buf) -> Result<ProxyLeaderInbound, SerError> {
        let p = match buf.get_u8() {
            PI_P2A => {
                let src = buf.get_u64();
                let slot = buf.get_u64();
                let round = buf.get_u64();
                let command_batch_or_noop = deserialise_command_batch_or_noop(buf);
                ProxyLeaderInbound::Phase2a(Phase2a {
                    src,
                    slot,
                    round,
                    command_batch_or_noop,
                })
            }
            PI_P2B => {
                let slot = buf.get_u64();
                let round = buf.get_u64();
                let acceptor_index = buf.get_u64();
                ProxyLeaderInbound::Phase2b(Phase2b {
                    slot,
                    round,
                    acceptor_index,
                })
            }
            e => {
                return Err(SerError::InvalidType(
                    format!("Found unkown id: {} but expected ProxyLeaderInbound", e).into(),
                ));
            }
        };
        Ok(p)
    }
}
