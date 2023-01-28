use super::messages::{
    AtomicBroadcastDeser, AtomicBroadcastMsg, Proposal, StopMsg as NetStopMsg, StopMsgDeser,
    RECONFIG_ID,
};
use crate::bench::atomic_broadcast::{
    messages::{ReconfigurationProposal, ReconfigurationResp},
};
use hashbrown::HashMap;
use kompact::prelude::*;
use quanta::{Clock};
#[cfg(feature = "track_timeouts")]
use quanta::Instant;
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};
use synchronoise::{event::CountdownError, CountdownEvent};
use crate::bench::atomic_broadcast::benchmark_master::ReconfigurationPolicy;
use crate::bench::atomic_broadcast::util::exp_util::{DATA_SIZE, KILL_TIMEOUT, WINDOW_DURATION};
use crate::bench::atomic_broadcast::client::*;
use crate::bench::atomic_broadcast::util::MetaResults;

#[derive(ComponentDefinition)]
pub struct ReconfigurationClient {
    ctx: ComponentContext<Self>,
    num_proposals: u64,
    num_concurrent_proposals: u64,
    nodes: HashMap<u64, ActorPath>,
    reconfig: Option<(ReconfigurationPolicy, Vec<u64>)>,
    leader_election_latch: Arc<CountdownEvent>,
    finished_latch: Arc<CountdownEvent>,
    latest_proposal_id: u64,
    max_proposal_id: u64,
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
    /// timestamp, number of proposals completed
    windows: Vec<usize>,
    clock: Clock,
    reconfig_start_ts: Option<SystemTime>,
    reconfig_end_ts: Option<SystemTime>,
    #[cfg(feature = "track_timeouts")]
    timeouts: Vec<u64>,
    #[cfg(feature = "track_timeouts")]
    late_responses: Vec<u64>,
    #[cfg(feature = "track_timestamps")]
    timestamps: HashMap<u64, Instant>,
    num_preloaded_proposals: u64,
    rem_preloaded_proposals: u64,
}

impl ReconfigurationClient {
    pub fn with(
        initial_config: Vec<u64>,
        num_proposals: u64,
        num_concurrent_proposals: u64,
        nodes: HashMap<u64, ActorPath>,
        reconfig: Option<(ReconfigurationPolicy, Vec<u64>)>,
        timeout: Duration,
        preloaded_log_size: u64,
        leader_election_latch: Arc<CountdownEvent>,
        finished_latch: Arc<CountdownEvent>,
    ) -> ReconfigurationClient {
        let clock = Clock::new();
        ReconfigurationClient {
            ctx: ComponentContext::uninitialised(),
            num_proposals,
            num_concurrent_proposals,
            nodes,
            reconfig,
            leader_election_latch,
            finished_latch,
            latest_proposal_id: preloaded_log_size,
            max_proposal_id: num_proposals + preloaded_log_size,
            responses: HashMap::with_capacity(num_proposals as usize),
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
            windows: vec![],
            clock,
            reconfig_start_ts: None,
            reconfig_end_ts: None,
            #[cfg(feature = "track_timeouts")]
            timeouts: vec![],
            #[cfg(feature = "track_timestamps")]
            timestamps: HashMap::with_capacity(num_proposals as usize),
            #[cfg(feature = "track_timeouts")]
            late_responses: vec![],
            num_preloaded_proposals: preloaded_log_size,
            rem_preloaded_proposals: preloaded_log_size,
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
            "{}",
            format!(
                "Proposing reconfiguration: policy: {:?}, new nodes: {:?}",
                policy, reconfig
            )
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
        let i = self.latest_proposal_id + available_n;
        let to = if i > self.max_proposal_id {
            self.max_proposal_id
        } else {
            i
        };
        if from > to {
            return;
        }
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
        self.responses.insert(id, latency_res);
        let received_count = self.responses.len() as u64;
        if received_count == self.num_proposals {
            if self.reconfig.is_none() {
                if let Some(timer) = self.window_timer.take() {
                    self.cancel_timer(timer);
                }
                self.state = ExperimentState::Finished;
                self.finished_latch
                    .decrement()
                    .expect("Failed to countdown finished latch");
                let leader_changes: Vec<_> =
                    self.leader_changes.iter().map(|(_ts, lc)| lc).collect();
                if self.num_timed_out > 0 || self.num_retried > 0 {
                    info!(self.ctx.log(), "Got all responses with {} timeouts and {} retries. Number of leader changes: {}, {:?}, Last leader was: {}, ballot/term: {}.", self.num_timed_out, self.num_retried, self.leader_changes.len(), leader_changes, self.current_leader, self.leader_round);
                    #[cfg(feature = "track_timeouts")]
                    {
                        let min = self.timeouts.iter().min();
                        let max = self.timeouts.iter().max();
                        let late_min = self.late_responses.iter().min();
                        let late_max = self.late_responses.iter().max();
                        info!(
                                self.ctx.log(),
                                "Timed out: Min: {:?}, Max: {:?}. Late responses: {}, min: {:?}, max: {:?}",
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
                        "Got all responses. Number of leader changes: {}, {:?}, Last leader was: {}, ballot/term: {}.",
                        self.leader_changes.len(),
                        leader_changes,
                        self.current_leader,
                        self.leader_round
                    );
                }
            } else {
                warn!(
                    self.ctx.log(),
                    "Got all normal responses but still pending reconfiguration"
                );
            }
        } else if received_count == self.num_proposals / 2 && self.reconfig.is_some() {
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

    fn handle_reconfig_response(&mut self, rr: ReconfigurationResp) {
        if let Some(proposal_meta) = self.pending_proposals.remove(&RECONFIG_ID) {
            self.reconfig_end_ts = Some(SystemTime::now());
            let new_config = rr.current_configuration;
            self.cancel_timer(proposal_meta.timer);
            if self.responses.len() as u64 == self.num_proposals {
                self.state = ExperimentState::Finished;
                self.finished_latch
                    .decrement()
                    .expect("Failed to countdown finished latch");
                info!(self.ctx.log(), "Got reconfig at last. {} proposals timed out. Leader changes: {}, {:?}, Last leader was: {}, ballot/term: {}", self.num_timed_out, self.leader_changes.len(), self.leader_changes, self.current_leader, self.leader_round);
            } else {
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
        }
    }

    fn proposal_timeout(&mut self, id: u64) -> Handled {
        if self.responses.contains_key(&id) {
            return Handled::Ok;
        }
        // info!(self.ctx.log(), "Timed out proposal {}", id);
        self.num_timed_out += 1;
        self.propose_normal(id);
        let _ = self.schedule_once(self.timeout, move |c, _| c.proposal_timeout(id));
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

    fn send_stop(&mut self) {
        for ap in self.nodes.values() {
            ap.tell_serialised(NetStopMsg::Client, self)
                .expect("Failed to send Client stop");
        }
        let _ = self.schedule_once(KILL_TIMEOUT, move |c, _| {
            if !c.nodes.is_empty() {
                warn!(c.ctx.log(), "Client did not get stop response from {:?}. Returning to BenchmarkMaster anyway who will force kill.", c.nodes.keys());
                c.reply_stop_ask();
            }
            Handled::Ok
        });
    }

    fn reply_stop_ask(&mut self) {
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
        let windowed_result = WindowedResult {
            num_warmup_windows: 0,
            windows: std::mem::take(&mut self.windows)
        };
        #[allow(unused_mut)]
            let mut meta_results = MetaResults {
            num_decided,
            num_warmup_decided: 0,
            num_timed_out: self.num_timed_out,
            num_retried: self.num_retried as u64,
            latencies,
            leader_changes: std::mem::take(&mut self.leader_changes),
            windowed_results: windowed_result,
            reconfig_ts,
            timestamps: vec![],
            #[cfg(feature = "simulate_partition")]
            longest_down_time: Duration::from_secs(0),
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
}

ignore_lifecycle!(ReconfigurationClient);

impl Actor for ReconfigurationClient {
    type Message = LocalClientMessage;

    fn receive_local(&mut self, msg: Self::Message) -> Handled {
        match msg {
            LocalClientMessage::Run => {
                self.state = ExperimentState::Running;
                assert_ne!(self.current_leader, 0);
                #[cfg(feature = "track_timestamps")]
                {
                    self.leader_changes.push((SystemTime::now(), (self.current_leader, self.leader_round)));

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
                let timer =
                    self.schedule_periodic(WINDOW_DURATION, WINDOW_DURATION, move |c, _| {
                        c.windows.push(c.responses.len());
                        Handled::Ok
                    });
                self.window_timer = Some(timer);
                self.send_concurrent_proposals();
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
                        info!(self.ctx.log(), "Got first leader: {}, but will wait for warmup to complete with all preloaded responses", self.current_leader);
                        return Handled::Ok; // wait until all preloaded responses before decrementing leader latch

                    }
                    AtomicBroadcastMsg::Leader(pid, round) if self.state == ExperimentState::Running && round > self.leader_round => {
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
                            self.send_concurrent_proposals();
                        } else {
                                if id <= self.num_preloaded_proposals && self.state == ExperimentState::Setup {
                                    self.current_leader = pr.latest_leader;
                                    self.leader_round = pr.leader_round;
                                    self.rem_preloaded_proposals -= 1;
                                    if self.rem_preloaded_proposals == 0 {
                                        match self.leader_election_latch.decrement() {
                                            Ok(_) => info!(self.ctx.log(), "Got all preloaded responses. Decrementing leader latch. leader: {}. Current config: {:?}. Payload size: {:?}", self.current_leader, self.current_config, DATA_SIZE),
                                            Err(e) => if e != CountdownError::AlreadySet {
                                                panic!("Failed to decrement election latch: {:?}", e);
                                            }
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
                        if !self.responses.contains_key(&id) {
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
                    if self.nodes.is_empty() {
                        self.reply_stop_ask();
                    }
                }
            },
            err(e) => error!(self.ctx.log(), "{}", &format!("Client failed to deserialise msg: {:?}", e)),
            default(_) => unimplemented!("Should be either AtomicBroadcastMsg or NetStopMsg"),
        }
        }
        Handled::Ok
    }
}