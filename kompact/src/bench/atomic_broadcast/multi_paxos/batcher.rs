use super::serializers::BatcherInboundSer;
use crate::bench::atomic_broadcast::{
    messages::Proposal,
    multi_paxos::{
        messages::{BatcherInbound, LeaderInbound},
        util::{ClassicRoundRobin, Round},
    },
};
use hashbrown::HashMap;
use kompact::prelude::*;
use std::time::Duration;

#[derive(ComponentDefinition)]
pub struct Batcher {
    ctx: ComponentContext<Self>,
    growing_batch: Vec<Proposal>,
    pending_resend_batches: Vec<Vec<Proposal>>,
    leaders: HashMap<u64, ActorPath>,
    round: Round,
    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Batcher.scala#L92
    round_system: ClassicRoundRobin,
    batch_timer: Option<ScheduledTimer>,
    /// index is needed for when simulating partition and sending the LeaderInfoRequestBatcher message
    index: u64,
    #[cfg(feature = "simulate_partition")]
    disconnected_peers: Vec<u64>,
    #[cfg(feature = "simulate_partition")]
    lagging_delay: Option<Duration>,
}

impl Batcher {
    pub fn with(index: u64, leaders: HashMap<u64, ActorPath>) -> Self {
        let n = leaders.len() as u64;
        Self {
            ctx: ComponentContext::uninitialised(),
            growing_batch: vec![],
            pending_resend_batches: vec![],
            leaders,
            round: 0,
            round_system: ClassicRoundRobin { n },
            batch_timer: None,
            index,
            #[cfg(feature = "simulate_partition")]
            disconnected_peers: vec![],
            #[cfg(feature = "simulate_partition")]
            lagging_delay: None,
        }
    }

    #[cfg(feature = "simulate_partition")]
    pub fn set_lagging_delay(&mut self, d: Duration) {
        self.lagging_delay = Some(d);
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Batcher.scala#L165
    fn handle_not_leader_batcher(&mut self, notleader_client_request_batch: Vec<Proposal>) {
        self.pending_resend_batches
            .push(notleader_client_request_batch);
        for (pid, leader) in &self.leaders {
            #[cfg(feature = "simulate_partition")]
            if self.disconnected_peers.contains(pid) {
                continue;
            }
            leader
                .tell_serialised(LeaderInbound::LeaderInfoRequestBatcher(self.index), self)
                .expect("Failed to serialize LeaderInfoRequestBatcher");
        }
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Batcher.scala#L177
    fn handle_leader_info(&mut self, leader_info_round: Round) {
        if leader_info_round <= self.round {
            debug!(
                self.ctx.log(),
                "A batcher received a LeaderInfoReplyBatcher message with round {:?} but is already in round {:?}. The LeaderInfoReplyBatcher message  must be stale, so we are ignoring it.",
                leader_info_round,
                self.round
            );
            return;
        }

        let old_round = self.round;
        let new_round = leader_info_round;
        self.round = leader_info_round;
        if self.round_system.leader(old_round) != self.round_system.leader(new_round) {
            let leader_index = &self.round_system.leader(new_round);
            #[cfg(feature = "simulate_partition")]
            {
                if self.disconnected_peers.contains(leader_index) {
                    self.pending_resend_batches.clear();
                    return;
                }
            }
            let leader = self
                .leaders
                .get(leader_index)
                .expect("Did not find actorpath for leader");
            for batch in std::mem::take(&mut self.pending_resend_batches) {
                leader
                    .tell_serialised(LeaderInbound::ClientRequestBatch(batch), self)
                    .expect("Failed to send message");
            }
            // pending_resend_batches is cleared by std::mem::take
        }
    }

    fn start_batch_timer(&mut self) {
        let timer = self.schedule_periodic(
            Duration::from_millis(0),
            Duration::from_millis(1),
            |c, _| {
                if c.growing_batch.is_empty() {
                    return Handled::Ok;
                }
                let leader_index = &c.round_system.leader(c.round);
                #[cfg(feature = "simulate_partition")]
                {
                    if c.disconnected_peers.contains(leader_index) {
                        c.growing_batch.clear();
                        return Handled::Ok;
                    }
                }
                let leader = c.leaders.get(leader_index).expect("No leader");
                leader
                    .tell_serialised(
                        LeaderInbound::ClientRequestBatch(std::mem::take(&mut c.growing_batch)),
                        c,
                    )
                    .expect("Failed to send message");
                // growing_batch is cleared by std::mem::take
                Handled::Ok
            },
        );
        self.batch_timer = Some(timer);
    }

    fn stop_batch_timer(&mut self) {
        if let Some(timer) = self.batch_timer.take() {
            self.cancel_timer(timer);
        }
    }

    #[cfg(feature = "simulate_partition")]
    pub fn disconnect_peers(&mut self, peers: Vec<u64>, delay: bool, lagging_peer: Option<u64>) {
        if let Some(lp) = lagging_peer {
            // disconnect from lagging peer first
            self.disconnected_peers.push(lp);
            let a = peers.clone();
            let lagging_delay = self.lagging_delay.expect("No lagging delay");
            self.schedule_once(lagging_delay, move |c, _| {
                for pid in a {
                    c.disconnected_peers.push(pid);
                }
                Handled::Ok
            });
        } else {
            if delay {
                let lagging_delay = self.lagging_delay.expect("No lagging delay");
                self.schedule_once(lagging_delay, move |c, _| {
                    c.disconnected_peers = peers;
                    Handled::Ok
                });
            } else {
                self.disconnected_peers = peers;
            }
        }
    }

    #[cfg(feature = "simulate_partition")]
    pub fn recover_peers(&mut self) {
        self.disconnected_peers.clear();
    }
}

impl Actor for Batcher {
    type Message = Proposal;

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Batcher.scala#L148
    fn receive_local(&mut self, msg: Self::Message) -> Handled {
        self.growing_batch.push(msg);
        // the batch will be sent when the timer is triggered.
        Handled::Ok
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Batcher.scala#L124
    fn receive_network(&mut self, msg: NetMessage) -> Handled {
        let NetMessage { data, .. } = msg;
        match_deser! {data {
            msg(b): BatcherInbound [using BatcherInboundSer] => {
                match b {
                    BatcherInbound::NotLeaderBatcher(b) => self.handle_not_leader_batcher(b),
                    BatcherInbound::LeaderInfoReplyBatcher(l) => self.handle_leader_info(l),
                }
            },
            err(e) => error!(self.ctx.log(), "Error deserialising msg: {:?}", e),
            default(_) => unimplemented!("Should be Batcher message!")
        }
        }
        Handled::Ok
    }
}

impl ComponentLifecycle for Batcher {
    fn on_start(&mut self) -> Handled {
        self.start_batch_timer();
        Handled::Ok
    }

    fn on_kill(&mut self) -> Handled {
        self.stop_batch_timer();
        Handled::Ok
    }
}
