use super::{messages::LeaderInbound, serializers::LeaderInboundSer};
use crate::bench::atomic_broadcast::{
    messages::{AtomicBroadcastMsg, Proposal},
    multi_paxos::{
        messages::{
            AcceptorInbound, BatcherInbound, ChosenWatermark, CommandBatchOrNoop, Nack, Phase1a,
            Phase1b, Phase1bSlotInfo, Phase2a, ProxyLeaderInbound, Recover,
        },
        util::{ClassicRoundRobin, LeaderElectionPort, Round, Slot},
    },
};
use hashbrown::HashMap;
use kompact::prelude::*;
use std::{cmp::max, time::Duration};

/// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Leader.scala#L33
pub struct LeaderOptions {
    pub(crate) resend_phase1as_period: Duration,
    //pub(crate) flush_phase2as_every_n: u64, // flushes on every phase2a
    pub(crate) noop_flush_period: Duration,
}

/// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Leader.scala#L117-L133
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum State {
    Inactive,
    Phase1,
    Phase2,
}

#[derive(ComponentDefinition)]
pub struct Leader {
    ctx: ComponentContext<Self>,
    options: LeaderOptions,
    state: State,
    index: u64, // pid
    round_system: ClassicRoundRobin,
    round: Round,
    next_slot: Slot,
    chosen_watermark: Slot,
    majority: usize,
    acceptors: HashMap<u64, ActorPath>,
    /// phase 1 variables
    // removed unnecessary variables related to flexible/compartmentalization
    phase1bs: HashMap<u64, Phase1b>,
    pending_client_request_batches: Vec<Vec<Proposal>>,
    resend_phase1as: Option<ScheduledTimer>,
    /// phase 2 variables
    noop_flush_timer: Option<ScheduledTimer>,
    proxy_leader: ActorRef<ProxyLeaderInbound>,
    leader_election_port: RequiredPort<LeaderElectionPort>,
    cached_client: ActorPath,
    #[cfg(feature = "simulate_partition")]
    disconnected_peers: Vec<u64>,
    #[cfg(feature = "simulate_partition")]
    lagging_delay: Option<Duration>,
    #[cfg(feature = "simulate_partition")]
    pub local_batcher: Option<ActorPath>,
}
/*
fn system_time_millis_as_u64() -> u64 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_millis() as u64
}*/

impl Leader {
    pub(crate) fn with(
        pid: u64,
        options: LeaderOptions,
        acceptors: HashMap<u64, ActorPath>,
        proxy_leader: ActorRef<ProxyLeaderInbound>,
        cached_client: ActorPath,
    ) -> Self {
        let num_leaders = acceptors.len() as u64;
        let state = if pid == num_leaders {
            State::Phase1
            // phase1 is started when component starts.
        } else {
            State::Phase2
        };
        let round_system = ClassicRoundRobin { n: num_leaders };
        let round = round_system.next_classic_round(num_leaders, 0);
        Self {
            ctx: ComponentContext::uninitialised(),
            options,
            state,
            index: pid,
            round_system,
            round,
            next_slot: 0,
            chosen_watermark: 0,
            majority: (num_leaders / 2 + 1) as usize,
            acceptors,
            phase1bs: HashMap::default(),
            pending_client_request_batches: vec![],
            resend_phase1as: None,
            noop_flush_timer: None,
            proxy_leader,
            leader_election_port: RequiredPort::uninitialised(),
            cached_client,
            #[cfg(feature = "simulate_partition")]
            disconnected_peers: vec![],
            #[cfg(feature = "simulate_partition")]
            lagging_delay: None,
            #[cfg(feature = "simulate_partition")]
            local_batcher: None,
        }
    }

    #[cfg(feature = "simulate_partition")]
    pub fn set_lagging_delay(&mut self, d: Duration) {
        self.lagging_delay = Some(d);
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Leader.scala#L222
    fn make_resend_phase1as_timer(&mut self, phase1a: Phase1a) -> ScheduledTimer {
        self.schedule_once(self.options.resend_phase1as_period, move |c, _| {
            for (pid, acceptor) in &c.acceptors {
                #[cfg(feature = "simulate_partition")]
                {
                    if c.disconnected_peers.contains(pid) {
                        continue;
                    }
                }
                acceptor
                    .tell_serialised(AcceptorInbound::Phase1a(phase1a), c)
                    .expect("Failed to send message");
            }
            Handled::Ok
        })
    }

    fn stop_resend_phase1as(&mut self) {
        if let Some(resend_phase1_as) = self.resend_phase1as.take() {
            self.cancel_timer(resend_phase1_as);
        }
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Leader.scala#L240
    fn make_noop_flush_timer(&mut self) -> ScheduledTimer {
        self.schedule_once(self.options.noop_flush_period, move |c, _| {
            match c.state {
                State::Inactive | State::Phase1 => {
                    error!(
                        c.ctx.log(),
                        "A leader tried to flush a noop but is not in Phase 2. It's state is {:?}",
                        c.state
                    );
                }
                State::Phase2 => {}
            }
            let phase2a = Phase2a {
                src: c.index,
                slot: c.next_slot,
                round: c.round,
                command_batch_or_noop: CommandBatchOrNoop::Noop,
            };
            c.proxy_leader.tell(ProxyLeaderInbound::Phase2a(phase2a));
            c.next_slot = c.next_slot + 1;
            Handled::Ok
        })
    }

    fn stop_noop_flush_timer(&mut self) {
        if let Some(noop_flush_timer) = self.noop_flush_timer.take() {
            self.cancel_timer(noop_flush_timer);
        }
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Leader.scala#L306
    fn max_phase1b_slot(phase1b: &Phase1b) -> Option<Slot> {
        if phase1b.info.is_empty() {
            None // return None instead of -1 since we use u64
        } else {
            Some(phase1b.info.iter().map(|x| x.slot).max().unwrap())
        }
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Leader.scala#L318
    fn safe_value(phase1bs: &[Phase1b], slot: Slot) -> CommandBatchOrNoop {
        let slot_infos: Vec<Phase1bSlotInfo> = phase1bs
            .iter()
            .flat_map(|x| x.info.clone())
            .filter(|x| x.slot == slot)
            .collect();
        if slot_infos.is_empty() {
            CommandBatchOrNoop::Noop
        } else {
            slot_infos
                .iter()
                .max_by(|x, y| x.vote_round.cmp(&y.vote_round))
                .unwrap()
                .vote_value
                .clone()
        }
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Leader.scala#L331
    fn process_client_request_batch(&mut self, client_request_batch: Vec<Proposal>) {
        match self.state {
            State::Inactive | State::Phase1 => {
                error!(self.ctx.log(), "A leader tried to process a client request batch but is not in Phase 2. It is in state {:?}", self.state);
            }
            State::Phase2 => {}
        }
        let phase2a = Phase2a {
            src: self.index,
            slot: self.next_slot,
            round: self.round,
            command_batch_or_noop: CommandBatchOrNoop::CommandBatch(
                client_request_batch
                    .iter()
                    .map(|x| x.data.clone())
                    .collect(),
            ),
        };
        self.proxy_leader
            .tell(ProxyLeaderInbound::Phase2a(phase2a.clone()));
        self.next_slot = self.next_slot + 1;
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Leader.scala#L409
    fn start_phase1(&mut self, round: Round, chosen_watermark: Slot) -> State {
        // info!(self.ctx.log(), "Starting phase1 in round {}", round);
        let phase1a = Phase1a {
            src: self.index,
            round,
            chosen_watermark,
        };
        for (pid, acceptor) in &self.acceptors {
            #[cfg(feature = "simulate_partition")]
            {
                if self.disconnected_peers.contains(pid) {
                    continue;
                }
            }
            acceptor
                .tell_serialised(AcceptorInbound::Phase1a(phase1a.clone()), self)
                .expect("Failed to send message");
        }
        self.pending_client_request_batches = vec![];
        self.resend_phase1as = Some(self.make_resend_phase1as_timer(phase1a));
        self.phase1bs.clear();
        State::Phase1
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Leader.scala#L432
    fn leader_change(&mut self, is_new_leader: bool) {
        match (self.state, is_new_leader) {
            (State::Inactive, false) => {
                // Do nothing.
            }
            (State::Phase1, false) => {
                self.stop_resend_phase1as();
                self.state = State::Inactive;
            }
            (State::Phase2, false) => {
                self.stop_noop_flush_timer();
                self.state = State::Inactive;
            }
            (State::Inactive, true) => {
                self.round = self.round_system.next_classic_round(self.index, self.round);
                self.state = self.start_phase1(self.round, self.chosen_watermark);
            }
            (State::Phase1, true) => {
                self.stop_resend_phase1as();
                self.round = self.round_system.next_classic_round(self.index, self.round);
                self.state = self.start_phase1(self.round, self.chosen_watermark);
            }
            (State::Phase2, true) => {
                self.stop_noop_flush_timer();
                self.round = self.round_system.next_classic_round(self.index, self.round);
                self.state = self.start_phase1(self.round, self.chosen_watermark);
            }
        }
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Leader.scala#L504
    fn handle_phase1b(&mut self, phase1b: Phase1b) {
        match self.state {
            State::Inactive | State::Phase2 => {
                debug!(self.ctx.log(), "A leader received a Phase1b message in round {:?} but is in round {:?}. The Phase1b is being ignored.", phase1b.round, self.round);
            }
            State::Phase1 => {
                if phase1b.round != self.round {
                    debug!(self.ctx.log(), "A leader received a Phase1b message in round {:?} but is in round {:?}. The Phase1b is being ignored.", phase1b.round, self.round);
                    return;
                }
                self.phase1bs.insert(phase1b.acceptor_index, phase1b);
                if self.phase1bs.len() < self.majority {
                    return;
                }
                #[cfg(feature = "simulate_partition")]
                self.local_batcher
                    .as_ref()
                    .unwrap()
                    .tell_serialised(BatcherInbound::LeaderInfoReplyBatcher(self.round), self)
                    .unwrap();
                self.cached_client
                    .tell_serialised(AtomicBroadcastMsg::Leader(self.index, self.round), self)
                    .expect("Failed to send new leader message to client");
                let max_slot = self
                    .phase1bs
                    .values()
                    .map(|x| Self::max_phase1b_slot(x))
                    .max()
                    .expect("No max slot");

                // check if some instead of using -1
                if let Some(max_slot) = max_slot {
                    for slot in self.chosen_watermark..=max_slot {
                        let phase2a = Phase2a {
                            src: self.index,
                            slot,
                            round: self.round,
                            command_batch_or_noop: Self::safe_value(
                                self.phase1bs
                                    .values()
                                    .cloned()
                                    .collect::<Vec<Phase1b>>()
                                    .as_slice(),
                                slot,
                            ),
                        };
                        self.proxy_leader
                            .tell(ProxyLeaderInbound::Phase2a(phase2a.clone()));
                    }
                    self.next_slot = max_slot + 1;
                }

                self.stop_resend_phase1as();
                self.state = State::Phase2;
                self.make_noop_flush_timer();

                for proposal in std::mem::take(&mut self.pending_client_request_batches) {
                    self.process_client_request_batch(proposal);
                }
            }
        }
    }

    /*
    /// Not used since we use batcher
    fn handle_client_request(&mut self, client_request: Proposal) {
        match self.state {
            State::Inactive => {
                todo!("send back proposal to client for retry")
            }
            State::Phase1 => {
                self.pending_client_request_batches
                    .push(vec![client_request]);
            }
            State::Phase2 => {
                self.process_client_request_batch(vec![client_request]);
            }
        }
    }
    */

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Leader.scala#L606
    fn handle_client_request_batch(
        &mut self,
        batcher: ActorPath,
        client_request_batch: Vec<Proposal>,
    ) {
        match self.state {
            State::Inactive => {
                batcher
                    .tell_serialised(BatcherInbound::NotLeaderBatcher(client_request_batch), self)
                    .expect("Failed to send message");
            }
            State::Phase1 => {
                self.pending_client_request_batches
                    .push(client_request_batch);
            }
            State::Phase2 => {
                self.process_client_request_batch(client_request_batch);
            }
        }
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Leader.scala#L636
    fn handle_leader_info_request_batcher(&self, src: u64, batcher: ActorPath) {
        #[cfg(feature = "simulate_partition")]
        if self.disconnected_peers.contains(&src) {
            return;
        }
        match self.state {
            State::Inactive => {
                // We're inactive, so we ignore the leader info request. The active
                // leader will respond to the request.
            }
            _ => {
                batcher
                    .tell_serialised(BatcherInbound::LeaderInfoReplyBatcher(self.round), self)
                    .expect("Failed to send message");
            }
        }
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Leader.scala#L672
    fn handle_nack(&mut self, nack: Nack) {
        if nack.round <= self.round {
            debug!(self.ctx.log(), "A leader received a Nack message in round {:?} but is in round {:?}. The Nack is being ignored.", nack.round, self.round);
            return;
        }
        match self.state {
            State::Inactive => {
                self.round = nack.round;
            }
            State::Phase1 => {
                // info!(self.ctx.log(), "got NACK in Phase1 round {}, nack_round: {}", self.round, nack.round);
                self.round = self.round_system.next_classic_round(self.index, nack.round);
                self.leader_change(true);
            }
            State::Phase2 => {
                // info!(self.ctx.log(), "got NACK in Phase2 round {}, nack_round: {}", self.round, nack.round);
                self.round = self.round_system.next_classic_round(self.index, nack.round);
                self.leader_change(true);
            }
        }
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Leader.scala#L699
    fn handle_chosen_watermark(&mut self, msg: ChosenWatermark) {
        self.chosen_watermark = max(self.chosen_watermark, msg.slot);
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Leader.scala#L706
    fn handle_recover(&mut self, _recover: Recover) {
        match self.state {
            State::Inactive => {
                // Do nothing. The active leader will recover.
            }
            State::Phase1 | State::Phase2 => {
                self.leader_change(true);
            }
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

impl ComponentLifecycle for Leader {
    fn on_start(&mut self) -> Handled {
        if self.state == State::Phase1 {
            self.start_phase1(self.round, self.chosen_watermark);
        }
        Handled::Ok
    }

    fn on_kill(&mut self) -> Handled {
        Handled::Ok
    }
}

impl Require<LeaderElectionPort> for Leader {
    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Leader.scala#L193-L203
    fn handle(&mut self, leader_index: <LeaderElectionPort as Port>::Indication) -> Handled {
        self.leader_change(leader_index == self.index);
        Handled::Ok
    }
}

impl Actor for Leader {
    type Message = ();

    fn receive_local(&mut self, _msg: Self::Message) -> Handled {
        Handled::Ok
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Leader.scala#L462
    fn receive_network(&mut self, msg: NetMessage) -> Handled {
        let NetMessage { data, sender, .. } = msg;
        match_deser! {data {
            msg(l): LeaderInbound [using LeaderInboundSer] => {
                match l {
                    LeaderInbound::Phase1b(p1b) => self.handle_phase1b(p1b),
                    LeaderInbound::Nack(nack) => self.handle_nack(nack),
                    LeaderInbound::ChosenWatermark(c) => self.handle_chosen_watermark(c),
                    LeaderInbound::Recover(r) => self.handle_recover(r),
                    LeaderInbound::ClientRequestBatch(p) => self.handle_client_request_batch(sender, p),
                    LeaderInbound::LeaderInfoRequestBatcher(src) => self.handle_leader_info_request_batcher(src, sender),
                }
            },
            err(e) => error!(self.ctx.log(), "Error deserialising msg: {:?}", e),
            default(_) => unimplemented!("Should be LeaderInbound message!")
        }
        }
        Handled::Ok
    }
}
