use super::messages::{
    AtomicBroadcastDeser, AtomicBroadcastMsg, Proposal, StopMsg as NetStopMsg, StopMsgDeser,
    RECONFIG_ID,
};
#[cfg(feature = "simulate_partition")]
use crate::bench::atomic_broadcast::messages::PartitioningExpMsg;
use crate::bench::atomic_broadcast::{
    benchmark_master::{NetworkScenario, ReconfigurationPolicy},
    messages::{ReconfigurationProposal, ReconfigurationResp},
    util::{exp_util::*, MetaResults},
};
use hashbrown::HashMap;
use kompact::prelude::*;
use quanta::{Clock, Instant};
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};
use synchronoise::CountdownEvent;

const PROPOSAL_ID_SIZE: usize = 8; // size of u64
const PAYLOAD: [u8; DATA_SIZE - PROPOSAL_ID_SIZE] = [0; DATA_SIZE - PROPOSAL_ID_SIZE];

#[derive(ComponentDefinition)]
pub struct Client {
    ctx: ComponentContext<Self>,
    exp_duration: Duration,
    num_concurrent_proposals: u64,
    nodes: HashMap<u64, ActorPath>,
    network_scenario: NetworkScenario,
    reconfig: Option<(ReconfigurationPolicy, Vec<u64>)>,
    warmup_latch: Arc<CountdownEvent>,
    finished_latch: Arc<CountdownEvent>,
    latest_proposal_id: u64,
    responses: HashMap<u64, Option<Duration>>,
    pending_proposals: HashMap<u64, ProposalMetaData>,
    timeout: Duration,
    current_leader: u64,
    leader_round: u64,
    state: ExperimentState,
    current_config: Vec<u64>,
    num_timed_out: u64,
    num_retried: usize,
    leader_changes: Vec<(SystemTime, (u64, u64))>,
    stop_ask: Option<Ask<(), MetaResults>>,
    window_timer: Option<ScheduledTimer>,
    warmup_decided: usize,
    /// timestamp, number of proposals completed
    windowed_result: WindowedResult,
    clock: Clock,
    reconfig_start_ts: Option<SystemTime>,
    reconfig_end_ts: Option<SystemTime>,
    #[cfg(feature = "track_timeouts")]
    timeouts: Vec<u64>,
    #[cfg(feature = "track_timeouts")]
    late_responses: Vec<u64>,
    #[cfg(feature = "track_timestamps")]
    timestamps: HashMap<u64, Instant>,
    #[cfg(feature = "preloaded_log")]
    num_preloaded_proposals: u64,
    #[cfg(feature = "preloaded_log")]
    rem_preloaded_proposals: u64,
    #[cfg(feature = "simulate_partition")]
    partition_timer: Option<ScheduledTimer>,
    #[cfg(feature = "simulate_partition")]
    recover_periodic_partition: bool,
    #[cfg(feature = "simulate_partition")]
    longest_down_time: Option<Duration>,
    #[cfg(feature = "simulate_partition")]
    latest_decided_ts: Option<Instant>,
}

impl Client {
    pub fn with(
        initial_config: Vec<u64>,
        exp_duration: Duration,
        num_concurrent_proposals: u64,
        nodes: HashMap<u64, ActorPath>,
        network_scenario: NetworkScenario,
        reconfig: Option<(ReconfigurationPolicy, Vec<u64>)>,
        timeout: Duration,
        preloaded_log_size: u64,
        warmup_latch: Arc<CountdownEvent>,
        finished_latch: Arc<CountdownEvent>,
    ) -> Client {
        let clock = Clock::new();
        Client {
            ctx: ComponentContext::uninitialised(),
            exp_duration,
            num_concurrent_proposals,
            nodes,
            network_scenario,
            reconfig,
            warmup_latch,
            finished_latch,
            latest_proposal_id: preloaded_log_size,
            responses: HashMap::with_capacity(1000000),
            pending_proposals: HashMap::with_capacity(num_concurrent_proposals as usize),
            timeout,
            current_leader: 0,
            leader_round: 0,
            state: ExperimentState::Setup,
            current_config: initial_config,
            num_timed_out: 0,
            num_retried: 0,
            leader_changes: vec![],
            stop_ask: None,
            window_timer: None,
            warmup_decided: 0,
            windowed_result: WindowedResult::default(),
            clock,
            reconfig_start_ts: None,
            reconfig_end_ts: None,
            #[cfg(feature = "track_timeouts")]
            timeouts: vec![],
            #[cfg(feature = "track_timestamps")]
            timestamps: HashMap::with_capacity(exp_duration as usize),
            #[cfg(feature = "track_timeouts")]
            late_responses: vec![],
            #[cfg(feature = "preloaded_log")]
            num_preloaded_proposals: preloaded_log_size,
            #[cfg(feature = "preloaded_log")]
            rem_preloaded_proposals: preloaded_log_size,
            #[cfg(feature = "simulate_partition")]
            partition_timer: None,
            #[cfg(feature = "simulate_partition")]
            recover_periodic_partition: false,
            #[cfg(feature = "simulate_partition")]
            longest_down_time: None,
            #[cfg(feature = "simulate_partition")]
            latest_decided_ts: None,
        }
    }

    fn propose_normal(&self, id: u64) {
        let leader = self.nodes.get(&self.current_leader).unwrap();
        let data = create_raw_proposal(id);
        let p = Proposal::with(data);
        leader
            .tell_serialised(AtomicBroadcastMsg::Proposal(p), self)
            .expect("Should serialise Proposal");
    }

    fn propose_reconfiguration(&self, node: &ActorPath) {
        let (policy, reconfig) = self.reconfig.as_ref().unwrap();
        info!(
            self.ctx.log(),
            "Proposing reconfiguration: policy: {:?}, new nodes: {:?}", policy, reconfig
        );
        let rp = ReconfigurationProposal::with(*policy, reconfig.clone());
        node.tell_serialised(AtomicBroadcastMsg::ReconfigurationProposal(rp), self)
            .expect("Should serialise reconfig Proposal");
        #[cfg(feature = "track_timeouts")]
        {
            info!(self.ctx.log(), "Proposed reconfiguration. latest_proposal_id: {}, timed_out: {}, pending proposals: {}, min: {:?}, max: {:?}",
                self.latest_proposal_id, self.num_timed_out, self.pending_proposals.len(), self.pending_proposals.keys().min(), self.pending_proposals.keys().max());
        }
    }

    fn send_concurrent_proposals(&mut self) {
        let num_inflight = self.pending_proposals.len() as u64;
        if num_inflight == self.num_concurrent_proposals || self.current_leader == 0 {
            return;
        }
        let available_n = self.num_concurrent_proposals - num_inflight;
        let from = self.latest_proposal_id + 1;
        let to = self.latest_proposal_id + available_n;
        let cache_start_time =
            self.num_concurrent_proposals == 1 || cfg!(feature = "track_latency");
        for id in from..=to {
            let current_time = match cache_start_time {
                true => Some(self.clock.now()),
                _ => None,
            };
            self.propose_normal(id);
            let timer = self.schedule_once(self.timeout, move |c, _| c.proposal_timeout(id));
            let proposal_meta = ProposalMetaData::with(current_time, timer);
            self.pending_proposals.insert(id, proposal_meta);
        }
        self.latest_proposal_id = to;
    }

    fn handle_normal_response(&mut self, id: u64, latency_res: Option<Duration>) {
        #[cfg(feature = "track_timestamps")]
        {
            let timestamp = self.clock.now();
            self.timestamps.insert(id, timestamp);
        }
        match self.state {
            ExperimentState::Running => {
                self.responses.insert(id, latency_res);
            }
            ExperimentState::WarmUp => {
                self.warmup_decided = self.warmup_decided + 1;
            }
            _ => {}
        }
    }

    fn handle_reconfig_response(&mut self, rr: ReconfigurationResp) {
        if let Some(proposal_meta) = self.pending_proposals.remove(&RECONFIG_ID) {
            self.reconfig_end_ts = Some(SystemTime::now());
            let new_config = rr.current_configuration;
            self.cancel_timer(proposal_meta.timer);
            match self.state {
                ExperimentState::PendingReconfiguration => {
                    self.finished_latch
                        .decrement()
                        .expect("Failed to countdown finished latch");
                    info!(self.ctx.log(), "Got reconfig at last. Num responses: {}, {} proposals timed out. Leader changes: {}, {:?}, Last leader was: {}, ballot/term: {}", self.responses.len(), self.num_timed_out, self.leader_changes.len(), self.leader_changes, self.current_leader, self.leader_round);
                    self.state = ExperimentState::Finished;
                }
                ExperimentState::Running => {
                    self.reconfig = None;
                    self.current_config = new_config;
                    info!(
                        self.ctx.log(),
                        "Reconfig OK, leader: {}, old: {}, current_config: {:?}",
                        rr.latest_leader,
                        self.current_leader,
                        self.current_config
                    );
                    if rr.latest_leader > 0
                        && self.current_leader != rr.latest_leader
                        && rr.leader_round > self.leader_round
                    {
                        self.current_leader = rr.latest_leader;
                        self.leader_round = rr.leader_round;
                        self.leader_changes
                            .push((SystemTime::now(), (self.current_leader, self.leader_round)));
                    }
                    self.send_concurrent_proposals();
                }
                e => unreachable!(
                    "Unexpected experiment state when getting reconfig response: {:?}",
                    e
                ),
            }
        }
    }

    fn proposal_timeout(&mut self, id: u64) -> Handled {
        if self.responses.contains_key(&id) {
            return Handled::Ok;
        }
        // info!(self.ctx.log(), "Timed out proposal {}", id);
        self.num_timed_out += 1;
        if self.state == ExperimentState::Running {
            self.propose_normal(id);
            let _ = self.schedule_once(self.timeout, move |c, _| c.proposal_timeout(id));
        }
        #[cfg(feature = "track_timeouts")]
        {
            self.timeouts.push(id);
        }
        Handled::Ok
    }

    fn reconfig_timeout(&mut self) -> Handled {
        let leader = self
            .nodes
            .get(&self.current_leader)
            .expect("No leader to propose reconfiguration to!");
        self.propose_reconfiguration(leader);
        let timer = self.schedule_once(self.timeout, move |c, _| c.reconfig_timeout());
        let proposal_meta = self
            .pending_proposals
            .get_mut(&RECONFIG_ID)
            .expect("Could not find MetaData for Reconfiguration in pending_proposals");
        proposal_meta.set_timer(timer);
        Handled::Ok
    }

    fn experiment_timeout(&mut self) -> Handled {
        if self.reconfig.is_none() {
            #[cfg(feature = "simulate_partition")]
            {
                let now = self.clock.now();
                if let Some(timer) = self.partition_timer.take() {
                    self.cancel_timer(timer);
                }
                if self.network_scenario != NetworkScenario::FullyConnected
                    && !self.pending_proposals.is_empty()
                {
                    // there was some unfinished proposals... assume they got decided now.
                    let previous_ts = self.latest_decided_ts.take().unwrap();
                    let down_time = now.duration_since(previous_ts);
                    if down_time > self.longest_down_time.unwrap_or_default() {
                        warn!(self.ctx.log(), "Longest down time recorded when experiment finished: Protocol probably did not recover from the partition");
                        self.longest_down_time = Some(down_time);
                    }
                }
            }
            self.finished_latch
                .decrement()
                .expect("Failed to countdown finished latch");

            if let Some(timer) = self.window_timer.take() {
                self.cancel_timer(timer);
            }
            self.state = ExperimentState::Finished;
            let leader_changes: Vec<_> = self.leader_changes.iter().map(|(_ts, lc)| lc).collect();
            if self.num_timed_out > 0 || self.num_retried > 0 {
                info!(self.ctx.log(), "Num decided: {}. with {} timeouts and {} retries. Number of leader changes: {}, Last leader was: {}, ballot/term: {}.", self.responses.len(), self.num_timed_out, self.num_retried, self.leader_changes.len(), self.current_leader, self.leader_round);
                if leader_changes.len() < 20 {
                    info!(self.ctx.log(), "{:?}", leader_changes);
                }
                #[cfg(feature = "simulate_partition")]
                {
                    if self.network_scenario != NetworkScenario::FullyConnected {
                        info!(
                            self.ctx.log(),
                            "Longest down time: {:?}", self.longest_down_time
                        );
                    }
                }
                #[cfg(feature = "track_timeouts")]
                {
                    let min = self.timeouts.iter().min();
                    let max = self.timeouts.iter().max();
                    let late_min = self.late_responses.iter().min();
                    let late_max = self.late_responses.iter().max();
                    info!(
                        self.ctx.log(),
                        "Num decided: {}, Timed out: Min: {:?}, Max: {:?}. Late responses: {}, min: {:?}, max: {:?}",
                        self.responses.len(),
                        min,
                        max,
                        self.late_responses.len(),
                        late_min,
                        late_max
                    );
                }
            } else {
                info!(
                        self.ctx.log(),
                        "Num decided: {}. Number of leader changes: {}, {:?}, Last leader was: {}, ballot/term: {}.",
                        self.responses.len(),
                        self.leader_changes.len(),
                        leader_changes,
                        self.current_leader,
                        self.leader_round
                    );
            }
        } else {
            self.state = ExperimentState::PendingReconfiguration;
            warn!(
                self.ctx.log(),
                "Got all normal responses but still pending reconfiguration"
            );
        }
        Handled::Ok
    }

    fn send_stop(&mut self) {
        for ap in self.nodes.values() {
            ap.tell_serialised(NetStopMsg::Client, self)
                .expect("Failed to send Client stop");
        }
        let _ = self.schedule_once(COOLDOWN_DURATION, move |c, _| {
            c.reply_stop_ask();
            Handled::Ok
        });
    }

    fn reply_stop_ask(&mut self) {
        if !self.nodes.is_empty() {
            warn!(self.ctx.log(), "Client did not get stop response from {:?}. Returning to BenchmarkMaster anyway who will force kill.", self.nodes);
        }
        let stop_ask = self.stop_ask.take().unwrap();
        let num_decided = self.responses.len() as u64;
        let l = std::mem::take(&mut self.responses);
        let mut v: Vec<_> = l
            .into_iter()
            .filter(|(_, latency)| latency.is_some())
            .collect();
        v.sort();
        let latencies: Vec<Duration> = v.into_iter().map(|(_, latency)| latency.unwrap()).collect();

        let reconfig_ts = self
            .reconfig_start_ts
            .map(|start_ts| (start_ts, self.reconfig_end_ts.unwrap()));
        #[allow(unused_mut)] // TODO remove
        let mut meta_results = MetaResults {
            num_decided,
            num_warmup_decided: self.warmup_decided as u64,
            num_timed_out: self.num_timed_out,
            num_retried: self.num_retried as u64,
            latencies,
            leader_changes: std::mem::take(&mut self.leader_changes),
            windowed_results: std::mem::take(&mut self.windowed_result),
            reconfig_ts,
            timestamps: vec![],
            #[cfg(feature = "simulate_partition")]
            longest_down_time: self.longest_down_time.unwrap_or_default(),
        };
        #[cfg(feature = "track_timestamps")]
        {
            let mut timestamps: Vec<_> = std::mem::take(&mut self.timestamps).into_iter().collect();
            timestamps.sort();
            meta_results.timestamps = timestamps.into_iter().map(|(_pid, ts)| ts).collect();
        }
        stop_ask
            .reply(meta_results)
            .expect("Failed to reply StopAsk!");
    }

    #[cfg(feature = "simulate_partition")]
    fn create_partition(&mut self) {
        assert_ne!(self.current_leader, 0);
        let leader_ap = self.nodes.get(&self.current_leader).expect("No leader");
        let followers: Vec<_> = self
            .nodes
            .iter()
            .filter_map(|(pid, _)| {
                if pid != &self.current_leader {
                    Some(pid)
                } else {
                    None
                }
            })
            .copied()
            .collect();
        match self.network_scenario {
            NetworkScenario::QuorumLoss(duration) => {
                let fully_connected_node = *followers.first().unwrap(); // first follower to be partitioned from the leader
                let rest: Vec<u64> = self
                    .nodes
                    .keys()
                    .filter(|pid| *pid != &fully_connected_node)
                    .copied()
                    .collect();
                info!(
                    self.ctx.log(),
                    "Creating {:?} partition. fully-connected: {}, leader: {}, num_responses: {}",
                    self.network_scenario,
                    fully_connected_node,
                    self.current_leader,
                    self.responses.len()
                );
                for pid in &rest {
                    let ap = self.nodes.get(&pid).expect("No node ap");
                    ap.tell_serialised(
                        PartitioningExpMsg::DisconnectPeers((rest.clone(), false), None),
                        self,
                    )
                    .expect("Should serialise");
                }
                self.schedule_partition_recovery(duration);
            }
            NetworkScenario::ConstrainedElection(duration) => {
                let lagging_follower = *followers.first().unwrap(); // first follower to be partitioned from the leader
                info!(self.ctx.log(), "Creating partition {:?}. leader: {}, term: {}, lagging follower connected to majority: {}, num_responses: {}", self.network_scenario, self.current_leader, self.leader_round, lagging_follower, self.responses.len() + self.warmup_decided);
                for pid in &followers {
                    let (disconnect_peers, delay) = if pid == &lagging_follower {
                        let delay = false; // lagging node disconnects directly
                        (vec![self.current_leader], delay) // lagging node only disconnects from leader
                    } else {
                        // the rest disconnects to everybody but the lagging node
                        let disconnect_peers: Vec<_> = self
                            .nodes
                            .keys()
                            .filter(|p| *p != &lagging_follower && *p != pid)
                            .copied()
                            .collect();
                        let delay = true; // others disconnects with a delay
                        (disconnect_peers, delay)
                    };
                    info!(
                        self.ctx.log(),
                        "Node {} getting disconnected from {:?}", pid, disconnect_peers
                    );
                    let ap = self.nodes.get(&pid).expect("No follower ap");
                    ap.tell_serialised(
                        PartitioningExpMsg::DisconnectPeers((disconnect_peers, delay), None),
                        self,
                    )
                    .expect("Should serialise");
                }
                let non_lagging: Vec<u64> = followers
                    .iter()
                    .filter(|pid| *pid != &lagging_follower)
                    .copied()
                    .collect();
                leader_ap
                    .tell_serialised(
                        PartitioningExpMsg::DisconnectPeers(
                            (non_lagging, false),
                            Some(lagging_follower),
                        ),
                        self,
                    )
                    .expect("Should serialise");
                self.schedule_partition_recovery(duration);
            }
            NetworkScenario::Chained(duration) => {
                // Chained scenario
                let disconnected_follower = followers.first().unwrap();
                let ap = self.nodes.get(disconnected_follower).unwrap();
                info!(self.ctx.log(), "Creating {:?} partition. disconnected follower: {}, leader: {}, num_responses: {}", self.network_scenario, disconnected_follower, self.current_leader, self.responses.len() + self.warmup_decided);
                ap.tell_serialised(
                    PartitioningExpMsg::DisconnectPeers((vec![self.current_leader], false), None),
                    self,
                )
                .expect("Should serialise!");
                leader_ap
                    .tell_serialised(
                        PartitioningExpMsg::DisconnectPeers(
                            (vec![*disconnected_follower], false),
                            None,
                        ),
                        self,
                    )
                    .expect("Should serialise!");
                info!(
                    self.ctx.log(),
                    "Creating partition. Disconnecting leader: {} from follower: {}, num_responses: {}",
                    self.current_leader,
                    disconnected_follower,
                    self.responses.len()
                );
                self.schedule_partition_recovery(duration);
            }
            NetworkScenario::PeriodicFull(duration) => {
                // Periodic full scenario
                let disconnected_follower = *followers.first().unwrap();
                let timer =
                    self.schedule_periodic(Duration::from_millis(0), duration, move |c, _| {
                        if c.partition_timer.is_some() {
                            c.periodic_partition(disconnected_follower);
                        }
                        Handled::Ok
                    });
                self.partition_timer = Some(timer);
                return; // end of periodic partition
            }
            NetworkScenario::FullyConnected => {
                return;
            }
        }
        #[cfg(feature = "simulate_partition")]
        {
            let now = self.clock.now();
            self.latest_decided_ts = Some(now);
        }
    }

    #[cfg(feature = "simulate_partition")]
    fn schedule_partition_recovery(&mut self, d: Duration) {
        self.partition_timer = Some(self.schedule_once(d, move |c, _| {
            if let Some(timer) = c.partition_timer.take() {
                c.cancel_timer(timer);
            }
            c.recover_partition();
            Handled::Ok
        }));
    }

    #[cfg(feature = "simulate_partition")]
    fn recover_partition(&mut self) {
        info!(self.ctx.log(), "Recovering from network partition");
        for (_pid, ap) in &self.nodes {
            ap.tell_serialised(PartitioningExpMsg::RecoverPeers, self)
                .expect("Should serialise");
        }
    }

    #[cfg(feature = "simulate_partition")]
    fn periodic_partition(&mut self, disconnected_follower: u64) {
        if self.recover_periodic_partition {
            self.recover_partition()
        } else {
            info!(self.ctx.log(), "Partitioning {}", disconnected_follower);
            let rest: Vec<u64> = self
                .nodes
                .keys()
                .filter(|pid| **pid != disconnected_follower)
                .copied()
                .collect();
            for (pid, ap) in self.nodes.iter() {
                if pid == &disconnected_follower {
                    ap.tell_serialised(
                        PartitioningExpMsg::DisconnectPeers((rest.clone(), false), None),
                        self,
                    )
                    .expect("Should serialise!");
                } else {
                    ap.tell_serialised(
                        PartitioningExpMsg::DisconnectPeers(
                            (vec![disconnected_follower], false),
                            None,
                        ),
                        self,
                    )
                    .expect("Should serialise!");
                }
            }
        }
        self.recover_periodic_partition = !self.recover_periodic_partition;
    }

    fn start_warmup(&mut self) {
        self.state = ExperimentState::WarmUp;
        self.send_concurrent_proposals();
        self.schedule_once(WARMUP_DURATION, move |c, _| {
            c.warmup_latch
                .decrement()
                .expect("Failed to decrement warmup latch");
            Handled::Ok
        });
        let timer = self.schedule_periodic(WINDOW_DURATION, WINDOW_DURATION, move |c, _| {
            c.windowed_result
                .windows
                .push(c.warmup_decided + c.responses.len());
            Handled::Ok
        });
        self.window_timer = Some(timer);
    }
}

ignore_lifecycle!(Client);

impl Actor for Client {
    type Message = LocalClientMessage;

    fn receive_local(&mut self, msg: Self::Message) -> Handled {
        match msg {
            LocalClientMessage::Run => {
                self.state = ExperimentState::Running;
                self.windowed_result.set_num_warmup_windows();
                assert_ne!(self.current_leader, 0);
                #[cfg(feature = "simulate_partition")]
                self.create_partition();
                #[cfg(feature = "track_timestamps")]
                {
                    self.leader_changes
                        .push((SystemTime::now(), (self.current_leader, self.leader_round)));
                }
                #[cfg(feature = "periodic_client_logging")]
                {
                    self.schedule_periodic(WINDOW_DURATION, WINDOW_DURATION, move |c, _| {
                        info!(
                            c.ctx.log(),
                            "Num responses: {}, leader: {}, ballot/term: {}",
                            c.responses.len(),
                            c.current_leader,
                            c.leader_round
                        );
                        Handled::Ok
                    });
                }
                let _ = self.schedule_once(self.exp_duration, move |c, _| c.experiment_timeout());
                if self.reconfig.is_some() {
                    let leader = self
                        .nodes
                        .get(&self.current_leader)
                        .expect("No leader to propose reconfiguration to!");
                    self.propose_reconfiguration(&leader);
                    self.reconfig_start_ts = Some(SystemTime::now());
                    let timer = self.schedule_once(self.timeout, move |c, _| c.reconfig_timeout());
                    let proposal_meta = ProposalMetaData::with(None, timer);
                    self.pending_proposals.insert(RECONFIG_ID, proposal_meta);
                }
            }
            LocalClientMessage::Stop(a) => {
                let pending_proposals = std::mem::take(&mut self.pending_proposals);
                for proposal_meta in pending_proposals {
                    self.cancel_timer(proposal_meta.1.timer);
                }
                self.send_stop();
                self.stop_ask = Some(a);
            }
        }
        Handled::Ok
    }

    fn receive_network(&mut self, m: NetMessage) -> Handled {
        let NetMessage {
            sender: _,
            receiver: _,
            data,
        } = m;
        match_deser! {data {
            msg(am): AtomicBroadcastMsg [using AtomicBroadcastDeser] => {
                // info!(self.ctx.log(), "Handling {:?}", am);
                match am {
                    AtomicBroadcastMsg::Leader(pid, round) if self.state == ExperimentState::Setup => {
                        assert!(pid > 0);
                        self.current_leader = pid;
                        self.leader_round = round;
                        if cfg!(feature = "preloaded_log") {
                            return Handled::Ok; // wait until all preloaded responses before decrementing leader latch
                        } else {
                            info!(self.ctx.log(), "Got first leader: {}. Starting warmup", self.current_leader);
                            self.start_warmup();
                        }
                    }
                    AtomicBroadcastMsg::Leader(pid, round) if (self.state == ExperimentState::Running || self.state == ExperimentState::WarmUp) && round > self.leader_round => {
                        // info!(self.ctx.log(), "CHANGED LEADER: {}, ROUND: {}", pid, round);
                        self.leader_round = round;
                        if self.current_leader != pid {
                            self.current_leader = pid;
                            self.leader_changes.push((SystemTime::now(), (self.current_leader, self.leader_round)));
                        }
                    },
                    AtomicBroadcastMsg::ReconfigurationResp(rr) => {
                        self.handle_reconfig_response(rr);
                    }
                    AtomicBroadcastMsg::ProposalResp(pr) => {
                        if self.state == ExperimentState::Finished { return Handled::Ok; }
                        let data = pr.data;
                        let id = data.as_slice().get_u64();
                        if let Some(proposal_meta) = self.pending_proposals.remove(&id) {
                            let now = self.clock.now();
                            #[cfg(feature = "simulate_partition")] {
                                if self.network_scenario != NetworkScenario::FullyConnected && self.state != ExperimentState::Finished {
                                    if let Some(previous_ts) = std::mem::replace(&mut self.latest_decided_ts, Some(now)) {
                                        let down_time = now.duration_since(previous_ts);
                                        if down_time > self.longest_down_time.unwrap_or_default() {
                                            // info!(self.ctx.log(), "new longest down time: {:?}, id: {}, leader: {}", down_time, id, self.current_leader);
                                            self.longest_down_time = Some(down_time);
                                        }
                                    }
                                }
                            }
                            let latency = proposal_meta.start_time.map(|start_time| now.duration_since(start_time));
                            self.cancel_timer(proposal_meta.timer);
                            if self.current_config.contains(&pr.latest_leader) && self.leader_round < pr.leader_round{
                                // info!(self.ctx.log(), "Got leader in normal response: {}. old: {}", pr.latest_leader, self.current_leader);
                                self.leader_round = pr.leader_round;
                                if self.current_leader != pr.latest_leader {
                                    self.current_leader = pr.latest_leader;
                                    self.leader_changes.push((SystemTime::now(), (self.current_leader, self.leader_round)));
                                }
                            }
                            self.handle_normal_response(id, latency);
                            match self.state {
                                ExperimentState::Running | ExperimentState::WarmUp => self.send_concurrent_proposals(),
                                _ => {},
                            }
                        } else {
                            #[cfg(feature = "preloaded_log")] {
                                if id <= self.num_preloaded_proposals && self.state == ExperimentState::Setup {
                                    self.current_leader = pr.latest_leader;
                                    self.leader_round = pr.leader_round;
                                    self.rem_preloaded_proposals -= 1;
                                    if self.rem_preloaded_proposals == 0 {
                                        info!(self.ctx.log(), "Got all preloaded responses. Starting warmup. Leader: {}", self.current_leader);
                                        self.start_warmup();
                                    }
                                }
                            }
                            #[cfg(feature = "track_timeouts")] {
                                if self.timeouts.contains(&id) {
                                    /*if self.late_responses.is_empty() {
                                        info!(self.ctx.log(), "Got first late response: {}", id);
                                    }*/
                                    self.late_responses.push(id);
                                }
                            }
                        }
                    },
                    AtomicBroadcastMsg::Proposal(p) => {    // node piggybacked proposal i.e. proposal failed
                        let id = p.data.as_slice().get_u64();
                        if !self.responses.contains_key(&id) && self.state == ExperimentState::Running {
                            self.num_retried += 1;
                            self.propose_normal(id);
                        }
                    }
                    _ => {},
                }
            },
            msg(stop): NetStopMsg [using StopMsgDeser] => {
                if let NetStopMsg::Peer(pid) = stop {
                    self.nodes.remove(&pid).unwrap_or_else(|| panic!("Got stop from unknown pid {}", pid));
                }
            },
            err(e) => error!(self.ctx.log(), "{}", &format!("Client failed to deserialise msg: {:?}", e)),
            default(_) => unimplemented!("Should be either AtomicBroadcastMsg or NetStopMsg"),
        }
        }
        Handled::Ok
    }
}

pub fn create_raw_proposal(id: u64) -> Vec<u8> {
    let mut data: Vec<u8> = Vec::with_capacity(DATA_SIZE - PROPOSAL_ID_SIZE);
    data.put_u64(id);
    data.put_slice(&PAYLOAD);
    data
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum ExperimentState {
    Setup,
    WarmUp,
    Running,
    PendingReconfiguration, // only reconfiguration left in the experiment
    Finished,
}

#[derive(Debug)]
pub enum LocalClientMessage {
    Run,
    Stop(Ask<(), MetaResults>), // (num_timed_out, latency)
}

#[derive(Debug)]
struct ProposalMetaData {
    start_time: Option<Instant>,
    timer: ScheduledTimer,
}

impl ProposalMetaData {
    fn with(start_time: Option<Instant>, timer: ScheduledTimer) -> ProposalMetaData {
        ProposalMetaData { start_time, timer }
    }

    fn set_timer(&mut self, timer: ScheduledTimer) {
        self.timer = timer;
    }
}

#[derive(Debug, Default)]
pub struct WindowedResult {
    pub num_warmup_windows: usize,
    pub windows: Vec<usize>,
}

impl WindowedResult {
    fn set_num_warmup_windows(&mut self) {
        self.num_warmup_windows = self.windows.len();
    }
}
