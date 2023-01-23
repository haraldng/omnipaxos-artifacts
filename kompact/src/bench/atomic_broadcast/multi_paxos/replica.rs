use super::{messages::ReplicaInbound, serializers::ReplicaInboundSer};
use crate::bench::atomic_broadcast::{
    messages::{AtomicBroadcastMsg, ProposalResp},
    multi_paxos::{
        messages::{Chosen, ChosenWatermark, CommandBatchOrNoop, LeaderInbound, Recover},
        util::{BufferMap, LeaderElectionPort},
    },
};
use hashbrown::HashMap;
use kompact::prelude::*;
use rand::{thread_rng, Rng};
use std::time::Duration;

/// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Replica.scala#L34
pub struct ReplicaOptions {
    pub(crate) log_grow_size: usize,
    pub(crate) send_chosen_watermark_every_n_entries: u64,
    pub(crate) recover_log_entry_min_period: Duration,
    pub(crate) recover_log_entry_max_period: Duration,
}

impl ReplicaOptions {
    fn get_random_timeout_period(&self) -> Duration {
        let mut rng = thread_rng();
        let min = self.recover_log_entry_min_period.as_millis();
        let max = self.recover_log_entry_max_period.as_millis();
        let rnd: u128 = rng.gen_range(min, max);
        Duration::from_millis(rnd as u64)
    }
}
#[derive(ComponentDefinition)]
pub struct Replica {
    ctx: ComponentContext<Self>,
    index: u64,
    log: BufferMap,
    executed_watermark: u64,
    num_chosen: u64,
    recover_timer: Option<ScheduledTimer>,
    options: ReplicaOptions,
    num_replicas: u64,
    leaders: HashMap<u64, ActorPath>,
    leader_election_port: RequiredPort<LeaderElectionPort>,
    cached_client: ActorPath,
    current_leader: u64,
    current_round: u64,
    #[cfg(feature = "simulate_partition")]
    disconnected_peers: Vec<u64>,
    #[cfg(feature = "simulate_partition")]
    lagging_delay: Option<Duration>,
}

impl Replica {
    pub(crate) fn with(
        pid: u64,
        options: ReplicaOptions,
        num_replicas: u64,
        leaders: HashMap<u64, ActorPath>,
        cached_client: ActorPath,
    ) -> Self {
        let initial_leader = leaders.len() as u64;
        Self {
            ctx: ComponentContext::uninitialised(),
            index: pid,
            log: BufferMap::with(options.log_grow_size),
            executed_watermark: 0,
            num_chosen: 0,
            recover_timer: None,
            options,
            num_replicas,
            leaders,
            leader_election_port: RequiredPort::uninitialised(),
            cached_client,
            current_leader: initial_leader,
            current_round: 0,
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

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Replica.scala#L394
    fn execute_log(&mut self) -> Vec<ProposalResp> {
        let mut client_replies = vec![];
        loop {
            match self.log.get(self.executed_watermark) {
                None => {
                    // We have to execute the log in prefix order, so if we hit an empty
                    // slot, we have to stop executing.
                    return client_replies;
                }
                Some(command_batch_or_noop) => {
                    let _slot = self.executed_watermark;
                    // ignore actual command execution
                    if self.index == self.current_leader {
                        // only leader responds to client
                        match command_batch_or_noop {
                            CommandBatchOrNoop::CommandBatch(command_batch) => {
                                let mut resp = command_batch
                                    .iter()
                                    .map(|data| {
                                        ProposalResp::with(
                                            data.clone(),
                                            self.current_leader,
                                            self.current_round,
                                        )
                                    })
                                    .collect();
                                client_replies.append(&mut resp);
                            }
                            CommandBatchOrNoop::Noop => {
                                debug!(
                                    self.ctx.log(),
                                    "Decided Noop in slot {}", self.executed_watermark
                                );
                            }
                        }
                    }
                    self.executed_watermark = self.executed_watermark + 1;
                    let modulo = self.executed_watermark
                        % self.options.send_chosen_watermark_every_n_entries;
                    let div = self.executed_watermark
                        / self.options.send_chosen_watermark_every_n_entries;
                    if modulo == 0 && div % self.num_replicas == self.index {
                        for (pid, leader) in &self.leaders {
                            #[cfg(feature = "simulate_partition")]
                            {
                                if self.disconnected_peers.contains(pid) {
                                    continue;
                                }
                            }
                            let c = ChosenWatermark {
                                slot: self.executed_watermark,
                            };
                            leader
                                .tell_serialised(LeaderInbound::ChosenWatermark(c), self)
                                .expect("Failed to send message");
                        }
                    }
                }
            }
        }
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Replica.scala#L572
    fn handle_chosen(&mut self, chosen: Chosen) {
        let is_recover_timer_running = self.num_chosen != self.executed_watermark;
        let old_executed_watermark = self.executed_watermark;

        match self.log.get(chosen.slot) {
            Some(_) => {
                // We've already received a Chosen message for this slot. We ignore the message.
                return;
            }
            None => {
                self.log.put(chosen.slot, chosen.command_batch_or_noop);
                self.num_chosen = self.num_chosen + 1;
            }
        }
        let client_reply_batch = self.execute_log();
        for reply in client_reply_batch {
            self.cached_client
                .tell_serialised(AtomicBroadcastMsg::ProposalResp(reply), self)
                .expect("Failed to send message");
        }
        let should_recover_timer_be_running = self.num_chosen != self.executed_watermark;
        let should_recover_timer_be_reset = old_executed_watermark != self.executed_watermark;
        /*
        info!(self.ctx.log(), "Chosen={}, watermark={}, old_watermark={}\nis_recover_timer_running: {}, should_recover_timer_be_running: {}, should_recover_timer_be_reset: {}",
            self.num_chosen, self.executed_watermark, old_executed_watermark,
            is_recover_timer_running, should_recover_timer_be_running, should_recover_timer_be_reset
        );
        */
        if is_recover_timer_running {
            match (
                should_recover_timer_be_running,
                should_recover_timer_be_reset,
            ) {
                (true, true) => {
                    // info!(self.ctx.log(), "RESETTING recover timer because chosen: current leader: {}, round: {}", self.current_leader, self.current_round);
                    self.reset_recover_timer()
                }
                (true, false) => {} // Do nothing.
                (false, _) => {
                    // info!(self.ctx.log(), "STOPPING recover timer because chosen: current leader: {}, round: {}", self.current_leader, self.current_round);
                    self.stop_recover_timer()
                }
            }
        } else if should_recover_timer_be_running {
            // info!(self.ctx.log(), "Starting recovery timer");
            self.start_recover_timer();
        }
    }

    fn start_recover_timer(&mut self) {
        let random_duration = self.options.get_random_timeout_period();
        self.schedule_periodic(random_duration, random_duration, |c, _| {
            // info!(c.ctx.log(), "Recovering");
            let r = Recover {
                slot: c.executed_watermark,
            };
            for (pid, leader) in &c.leaders {
                #[cfg(feature = "simulate_partition")]
                {
                    if c.disconnected_peers.contains(pid) {
                        continue;
                    }
                }
                leader
                    .tell_serialised(LeaderInbound::Recover(r), c)
                    .expect("Failed to send Recover");
            }
            Handled::Ok
        });
    }

    fn stop_recover_timer(&mut self) {
        if let Some(timer) = std::mem::take(&mut self.recover_timer) {
            self.cancel_timer(timer);
        }
    }

    fn reset_recover_timer(&mut self) {
        self.stop_recover_timer();
        self.start_recover_timer();
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

impl Actor for Replica {
    type Message = ();

    fn receive_local(&mut self, _msg: Self::Message) -> Handled {
        Handled::Ok
    }

    /// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/multipaxos/Replica.scala#L537
    fn receive_network(&mut self, msg: NetMessage) -> Handled {
        let NetMessage { data, .. } = msg;
        match_deser! {data {
            msg(r): ReplicaInbound [using ReplicaInboundSer] => {
                match r {
                    ReplicaInbound::Chosen(c) => self.handle_chosen(c),
                }
            },
            err(e) => error!(self.ctx.log(), "Error deserialising msg: {:?}", e),
            default(_) => unimplemented!("Should be Replica message!")
        }
        }
        Handled::Ok
    }
}

ignore_lifecycle!(Replica);

impl Require<LeaderElectionPort> for Replica {
    fn handle(&mut self, leader: <LeaderElectionPort as Port>::Indication) -> Handled {
        /*
        info!(
            self.ctx.log(),
            "Participant changed leader from {} to {}", self.current_leader, leader
        );
        */
        self.current_leader = leader;
        Handled::Ok
    }
}
