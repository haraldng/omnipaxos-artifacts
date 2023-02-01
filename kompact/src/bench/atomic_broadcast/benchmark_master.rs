use crate::{
    bench::atomic_broadcast::{
        benchmark_client::{ClientParams, ClientParamsDeser},
        client::{Client, LocalClientMessage},
        util::{benchmark_master::*, exp_util::*, MetaResults},
    },
    partitioning_actor::{IterationControlMsg, PartitioningActor},
};
use benchmark_suite_shared::{
    kompics_benchmarks::benchmarks::AtomicBroadcastRequest, BenchmarkError, DeploymentMetaData,
    DistributedBenchmarkMaster,
};
use hashbrown::HashMap;
use hdrhistogram::Histogram;
use kompact::prelude::*;
use std::{sync::Arc, time::Duration};
use synchronoise::CountdownEvent;
use crate::bench::atomic_broadcast::reconfig_client::ReconfigurationClient;

pub struct AtomicBroadcastMaster {
    num_nodes: Option<u64>,
    num_nodes_needed: Option<u64>,
    exp_duration: Option<Duration>,
    concurrent_proposals: Option<u64>,
    reconfiguration: Option<(ReconfigurationPolicy, Vec<u64>)>,
    network_scenario: Option<NetworkScenario>,
    election_timeout_ms: Option<u64>,
    system: Option<KompactSystem>,
    finished_latch: Option<Arc<CountdownEvent>>,
    iteration_id: u32,
    client_comp: Option<Arc<Component<Client>>>,
    reconfig_client_comp: Option<Arc<Component<ReconfigurationClient>>>,
    partitioning_actor: Option<Arc<Component<PartitioningActor>>>,
    latency_hist: Option<Histogram<u64>>,
    num_timed_out: Vec<u64>,
    num_retried: Vec<u64>,
    experiment_str: Option<String>,
    meta_results_path: Option<String>,
    meta_results_sub_dir: Option<String>,
    lagging_delay_ms: u64
}

impl AtomicBroadcastMaster {
    pub(crate) fn new() -> AtomicBroadcastMaster {
        AtomicBroadcastMaster {
            num_nodes: None,
            num_nodes_needed: None,
            exp_duration: None,
            concurrent_proposals: None,
            reconfiguration: None,
            network_scenario: None,
            election_timeout_ms: None,
            system: None,
            finished_latch: None,
            iteration_id: 0,
            client_comp: None,
            reconfig_client_comp: None,
            partitioning_actor: None,
            latency_hist: None,
            num_timed_out: vec![],
            num_retried: vec![],
            experiment_str: None,
            meta_results_path: None,
            meta_results_sub_dir: None,
            lagging_delay_ms: 0
        }
    }

    fn initialise_iteration(
        &self,
        nodes: Vec<ActorPath>,
        client: ActorPath,
        pid_map: Option<HashMap<ActorPath, u32>>,
    ) -> Arc<Component<PartitioningActor>> {
        let system = self.system.as_ref().unwrap();
        let prepare_latch = Arc::new(CountdownEvent::new(1));
        /*** Setup partitioning actor ***/
        let (partitioning_actor, unique_reg_f) = system.create_and_register(|| {
            PartitioningActor::with(
                prepare_latch.clone(),
                None,
                self.iteration_id,
                nodes,
                pid_map,
                None,
            )
        });
        unique_reg_f.wait_expect(
            Duration::from_millis(1000),
            "PartitioningComp failed to register!",
        );

        let partitioning_actor_f = system.start_notify(&partitioning_actor);
        partitioning_actor_f
            .wait_timeout(Duration::from_millis(1000))
            .expect("PartitioningComp never started!");
        let mut ser_client = Vec::<u8>::new();
        client
            .serialise(&mut ser_client)
            .expect("Failed to serialise ClientComp actorpath");
        partitioning_actor
            .actor_ref()
            .tell(IterationControlMsg::Prepare(Some(ser_client)));
        prepare_latch.wait();
        partitioning_actor
    }

    fn create_client(
        &self,
        nodes_id: HashMap<u64, ActorPath>,
        network_scenario: NetworkScenario,
        client_timeout: Duration,
        short_timeout: Duration,
        warmup_latch: Arc<CountdownEvent>,
    ) -> (Arc<Component<Client>>, ActorPath) {
        let system = self.system.as_ref().unwrap();
        let finished_latch = self.finished_latch.clone().unwrap();
        /*** Setup client ***/
        let initial_config: Vec<_> = (1..=self.num_nodes.unwrap()).map(|x| x as u64).collect();
        let reconfig = self.reconfiguration.clone();
        let warmup_duration = if self.is_test_run() { SHORT_WARMUP_DURATION } else { WARMUP_DURATION };
        let (client_comp, unique_reg_f) = system.create_and_register(|| {
            Client::with(
                initial_config,
                self.exp_duration.unwrap(),
                self.concurrent_proposals.unwrap(),
                nodes_id,
                network_scenario,
                reconfig,
                client_timeout,
                short_timeout,
                self.lagging_delay_ms,
                warmup_latch,
                finished_latch,
                warmup_duration,
            )
        });
        unique_reg_f.wait_expect(REGISTER_TIMEOUT, "Client failed to register!");
        let client_comp_f = system.start_notify(&client_comp);
        client_comp_f
            .wait_timeout(REGISTER_TIMEOUT)
            .expect("ClientComp never started!");
        let client_path = system
            .register_by_alias(&client_comp, format!("client{}", &self.iteration_id))
            .wait_expect(REGISTER_TIMEOUT, "Failed to register alias for ClientComp");
        (client_comp, client_path)
    }

    fn create_reconfig_client(
        &self,
        nodes_id: HashMap<u64, ActorPath>,
        client_timeout: Duration,
        preloaded_log_size: u64,
        leader_election_latch: Arc<CountdownEvent>,
    ) -> (Arc<Component<ReconfigurationClient>>, ActorPath) {
        let system = self.system.as_ref().unwrap();
        let finished_latch = self.finished_latch.clone().unwrap();
        /*** Setup client ***/
        let initial_config: Vec<_> = (1..=self.num_nodes.unwrap()).map(|x| x as u64).collect();
        let reconfig = self.reconfiguration.clone();
        let (recofig_client_comp, unique_reg_f) = system.create_and_register(|| {
            ReconfigurationClient::with(
                initial_config,
                RECONFIGURATION_PROPOSALS,
                self.concurrent_proposals.unwrap(),
                nodes_id,
                reconfig,
                client_timeout,
                preloaded_log_size,
                leader_election_latch,
                finished_latch,
            )
        });
        unique_reg_f.wait_expect(REGISTER_TIMEOUT, "Client failed to register!");
        let client_comp_f = system.start_notify(&recofig_client_comp);
        client_comp_f
            .wait_timeout(REGISTER_TIMEOUT)
            .expect("ClientComp never started!");
        let client_path = system
            .register_by_alias(&recofig_client_comp, format!("reconfig_client{}", &self.iteration_id))
            .wait_expect(REGISTER_TIMEOUT, "Failed to register alias for ClientComp");
        (recofig_client_comp, client_path)
    }

    fn validate_experiment_params(
        &mut self,
        c: &AtomicBroadcastRequest,
        num_clients: u32,
    ) -> Result<(), BenchmarkError> {
        if (num_clients as u64) < c.number_of_nodes {
            return Err(BenchmarkError::InvalidTest(format!(
                "Not enough clients: {}, Required: {}",
                num_clients, c.number_of_nodes
            )));
        }
        match &c.algorithm.to_lowercase() {
            a if a != "paxos"
                && a != "raft"
                && a != "vr"
                && a != "multi-paxos"
                && a != "raft_pv_qc" =>
            {
                return Err(BenchmarkError::InvalidTest(format!(
                    "Unimplemented atomic broadcast algorithm: {}",
                    &c.algorithm
                )));
            }
            _ => {}
        }
        match &c.reconfiguration.to_lowercase() {
            off if off == "off" => {
                self.num_nodes_needed = Some(c.number_of_nodes);
                if c.reconfig_policy.to_lowercase() != "none" {
                    return Err(BenchmarkError::InvalidTest(format!(
                        "Reconfiguration is off, transfer policy should be none, but found: {}",
                        &c.reconfig_policy
                    )));
                }
            }
            s if s == "single" || s == "majority" => {
                if &c.algorithm.to_lowercase() == "multi-paxos" {
                    return Err(BenchmarkError::InvalidTest(format!(
                        "Reconfiguration is not implemented for: {}",
                        &c.algorithm
                    )));
                }
                let policy = match c.reconfig_policy.to_lowercase().as_str() {
                    "replace-follower" => ReconfigurationPolicy::ReplaceFollower,
                    "replace-leader" => ReconfigurationPolicy::ReplaceLeader,
                    _ => {
                        return Err(BenchmarkError::InvalidTest(format!(
                            "Unimplemented reconfiguration policy: {}",
                            &c.reconfig_policy
                        )));
                    }
                };
                match get_reconfig_nodes(&c.reconfiguration, c.number_of_nodes) {
                    Ok(reconfig) => {
                        let additional_n = match &reconfig {
                            Some(r) => r.len() as u64,
                            None => 0,
                        };
                        let n = c.number_of_nodes + additional_n;
                        if (num_clients as u64) < n {
                            return Err(BenchmarkError::InvalidTest(format!(
                                "Not enough clients: {}, Required: {}",
                                num_clients, n
                            )));
                        }
                        self.reconfiguration = reconfig.map(|r| (policy, r));
                        self.num_nodes_needed = Some(n);
                    }
                    Err(e) => return Err(e),
                }
            }
            _ => {
                return Err(BenchmarkError::InvalidTest(format!(
                    "Unimplemented reconfiguration: {}",
                    &c.reconfiguration
                )));
            }
        }
        let str = c.network_scenario.to_lowercase();
        let network_scenario = {
            if str.as_str() == "fully_connected" {
                NetworkScenario::FullyConnected
            } else {
                let split: Vec<_> = str.split('-').collect();
                if split.len() != 2 {
                    return Err(BenchmarkError::InvalidTest(format!(
                        "Unimplemented network scenario for {} nodes: {}",
                        c.number_of_nodes, &c.network_scenario
                    )));
                }
                let duration = match split[1].parse::<u64>() {
                    Ok(d) => Duration::from_secs(d),
                    _ => {
                        return Err(BenchmarkError::InvalidTest(format!(
                            "Invalid duration for network scenario {}",
                            &c.network_scenario
                        )));
                    }
                };
                let network_scenario = match split[0] {
                    "quorum_loss" if c.number_of_nodes == 5 => {
                        NetworkScenario::QuorumLoss(duration)
                    }
                    "constrained_election" if c.number_of_nodes == 5 => {
                        NetworkScenario::ConstrainedElection(duration)
                    }
                    "chained" if c.number_of_nodes == 3 => NetworkScenario::Chained(duration),
                    "periodic_full" => NetworkScenario::PeriodicFull(duration),
                    _ => {
                        return Err(BenchmarkError::InvalidTest(format!(
                            "Unimplemented network scenario for {} nodes: {}",
                            c.number_of_nodes, &c.network_scenario
                        )));
                    }
                };
                network_scenario
            }
        };
        self.network_scenario = Some(network_scenario);
        Ok(())
    }

    fn get_meta_results_dir(&self, results_type: Option<&str>) -> String {
        let meta = self
            .meta_results_path
            .as_ref()
            .expect("No meta results path!");
        let sub_dir = self
            .meta_results_sub_dir
            .as_ref()
            .expect("No meta results sub dir path!");
        format!("{}/{}/{}", meta, sub_dir, results_type.unwrap_or("/"))
    }
}

impl DistributedBenchmarkMaster for AtomicBroadcastMaster {
    type MasterConf = AtomicBroadcastRequest;
    type ClientConf = ClientParams;
    type ClientData = ActorPath;

    fn setup(
        &mut self,
        c: Self::MasterConf,
        m: &DeploymentMetaData,
    ) -> Result<Self::ClientConf, BenchmarkError> {
        println!("Setting up Atomic Broadcast (Master)");
        self.validate_experiment_params(&c, m.number_of_clients())?;
        let experiment_str = create_experiment_str(&c);
        self.num_nodes = Some(c.number_of_nodes);
        self.experiment_str = Some(experiment_str.clone());
        self.concurrent_proposals = Some(c.concurrent_proposals);
        self.election_timeout_ms = Some(c.election_timeout_ms);
        let duration = match self.network_scenario.as_ref().unwrap() {
            NetworkScenario::Chained(d) => *d,
            NetworkScenario::QuorumLoss(d) => *d,
            NetworkScenario::ConstrainedElection(d) => {
                let duration_ms = d.as_millis() as u64;
                let election_timeout_ms = self.election_timeout_ms.unwrap();
                let lagging_delay_ms = election_timeout_ms / LAGGING_DELAY_FACTOR;
                self.lagging_delay_ms = lagging_delay_ms;
                Duration::from_millis(duration_ms + lagging_delay_ms)
            }
            _ => Duration::from_secs(c.duration_secs),
        };
        self.exp_duration = Some(duration);
        self.meta_results_path = Some(format!("../meta_results/{}/", m.run_id()));
        self.meta_results_sub_dir = Some(create_metaresults_sub_dir(
            c.number_of_nodes,
            c.concurrent_proposals,
            c.get_reconfiguration(),
        ));
        if c.concurrent_proposals == 1 || cfg!(feature = "track_latency") {
            self.latency_hist =
                Some(Histogram::<u64>::new(4).expect("Failed to create latency histogram"));
        }
        let path = if self.reconfiguration.is_some() {
            RECONFIG_CONFIG_PATH
        } else {
            CONFIG_PATH
        };
        let mut conf = KompactConfig::default();
        conf.load_config_file(path);
        let bc = BufferConfig::from_config_file(path);
        bc.validate();
        let system = crate::kompact_system_provider::global()
            .new_remote_system_with_threads_config("atomicbroadcast", 4, conf, bc, TCP_NODELAY);
        self.system = Some(system);
        let cpd = ClientParamsDeser {
            algorithm: c.algorithm,
            num_nodes: c.number_of_nodes,
            concurrent_proposals: c.concurrent_proposals,
            is_reconfig_exp: self.reconfiguration.is_some(),
            election_timeout_ms: c.election_timeout_ms,
            run_id: String::from(m.run_id()),
            scenario: c.network_scenario,
        };
        let params = ClientParams::with(cpd, c.reconfiguration.as_str());
        Ok(params)
    }

    fn prepare_iteration(&mut self, d: Vec<Self::ClientData>) -> () {
        println!("Preparing iteration");
        if self.system.is_none() {
            panic!("No KompactSystem found!")
        }
        let finished_latch = Arc::new(CountdownEvent::new(1));
        self.finished_latch = Some(finished_latch);
        self.iteration_id += 1;
        let num_nodes_needed = self.num_nodes_needed.expect("No cached num_nodes") as usize;
        let mut nodes = d;
        let path = if self.reconfiguration.is_some() {
            RECONFIG_CONFIG_PATH
        } else {
            CONFIG_PATH
        };
        let (client_timeout, preloaded_log_size) = load_benchmark_config(path, self.reconfiguration.is_some());
        let pid_map: Option<HashMap<ActorPath, u32>> = if cfg!(feature = "use_pid_map") {
            Some(load_pid_map(NODES_CONF, nodes.as_slice()))
        } else {
            None
        };
        let mut nodes_id: HashMap<u64, ActorPath> = HashMap::new();
        match &pid_map {
            Some(pm) => {
                for (ap, pid) in pm
                    .iter()
                    .filter(|(_, pid)| **pid <= num_nodes_needed as u32)
                {
                    nodes_id.insert(*pid as u64, ap.clone());
                }
            }
            None => {
                for (id, ap) in nodes.iter().take(num_nodes_needed).enumerate() {
                    nodes_id.insert(id as u64 + 1, ap.clone());
                }
            }
        }
        nodes.clear();
        for i in 1..=(num_nodes_needed as u64) {
            nodes.push(nodes_id.get(&i).unwrap().clone());
        }
        let warmup_latch = Arc::new(CountdownEvent::new(1));
        let client_path = match self.reconfiguration {
            None => {
                let short_timeout = if self.network_scenario != Some(NetworkScenario::FullyConnected) {
                    let client_timeout_factor = load_client_timeout_factor(CONFIG_PATH);
                    let mut short_timeout_ms = self.election_timeout_ms.unwrap() / client_timeout_factor;
                    if short_timeout_ms < 25 {
                        short_timeout_ms = 25;
                    }
                    Duration::from_millis(short_timeout_ms)
                } else {
                    client_timeout
                };
                let (client_comp, client_path) = self.create_client(
                    nodes_id,
                    self.network_scenario.expect("No network scenario"),
                    client_timeout,
                    short_timeout,
                    warmup_latch.clone(),
                );
                self.client_comp = Some(client_comp);
                client_path
            }
            Some(_) => {
                let (client_comp, client_path) = self.create_reconfig_client(
                    nodes_id,
                    client_timeout,
                    preloaded_log_size,
                    warmup_latch.clone(),
                );
                self.reconfig_client_comp = Some(client_comp);
                client_path
            }
        };
        let partitioning_actor = self.initialise_iteration(nodes, client_path, pid_map);
        partitioning_actor
            .actor_ref()
            .tell(IterationControlMsg::Run);
        warmup_latch.wait(); // wait until leader is elected and warmup is completed
        self.partitioning_actor = Some(partitioning_actor);
    }

    fn run_iteration(&mut self) -> () {
        println!("Running Atomic Broadcast experiment!");
        match self.reconfiguration {
            None => {
                self.client_comp.as_ref().unwrap().actor_ref().tell(LocalClientMessage::Run);
            }
            Some(_) => {
                self.reconfig_client_comp.as_ref().unwrap().actor_ref().tell(LocalClientMessage::Run);
            },
        }
        let finished_latch = self.finished_latch.take().unwrap();
        finished_latch.wait();
    }

    fn cleanup_iteration(&mut self, last_iteration: bool, exec_time_millis: f64) -> () {
        println!(
            "Cleaning up Atomic Broadcast (master) iteration {}. Exec_time: {}",
            self.iteration_id, exec_time_millis
        );
        if !self.is_test_run() {
            std::thread::sleep(COOLDOWN_DURATION);
        }
        let meta_results = match self.reconfiguration {
            None => {
                let client = self.client_comp.as_ref().unwrap().actor_ref();
                self.get_meta_results_and_stop_client(client)
            }
            Some(_) => {
                let client = self.reconfig_client_comp.as_ref().unwrap().actor_ref();
                self.get_meta_results_and_stop_client(client)
            }
        };
        self.num_timed_out.push(meta_results.num_timed_out);
        self.num_retried.push(meta_results.num_retried);
        self.persist_meta_results(meta_results);
        self.kill_client();
        self.kill_partitioning_component();
        if last_iteration {
            println!("Cleaning up last iteration");
            self.persist_meta_summary_results();
            self.clear_state();
            let system = self.system.take().unwrap();
            system
                .shutdown()
                .expect("Kompact didn't shut down properly");
        }
    }
}

impl AtomicBroadcastMaster {
    fn persist_meta_results(&mut self, meta_results: MetaResults) {
        let experiment_str = self.experiment_str.as_ref().expect("No experiment string");
        let windowed_dir = self.get_meta_results_dir(Some("windowed"));
        persist_windowed_results(
            windowed_dir.as_str(),
            experiment_str,
            meta_results.windowed_results,
        );
        let num_decided_dir = self.get_meta_results_dir(Some("num_decided"));
        persist_num_decided_results(
            num_decided_dir.as_str(),
            experiment_str,
            meta_results.num_decided,
            meta_results.num_warmup_decided,
            meta_results.leader_changes.first().map(|(_t, (pid, _b))| *pid),
        );
        #[cfg(feature = "simulate_partition")]
        {
            if self.network_scenario.as_ref().unwrap() != &NetworkScenario::FullyConnected {
                let longest_down_time_dir = self.get_meta_results_dir(Some("longest_down_time"));
                persist_longest_down_time_results(
                    longest_down_time_dir.as_str(),
                    experiment_str,
                    meta_results.longest_down_time,
                );
            }
        }
        if self.concurrent_proposals == Some(1) || cfg!(feature = "track_latency") {
            let latency_dir = self.get_meta_results_dir(Some("latency"));
            persist_latency_results(
                latency_dir.as_str(),
                experiment_str,
                &meta_results.latencies,
                self.latency_hist.as_mut().unwrap(),
            );
        }
        if meta_results.reconfig_ts.is_some() || !meta_results.leader_changes.is_empty() {
            let timstamp_dir = self.get_meta_results_dir(Some("timestamps"));
            persist_timestamp_results(
                timstamp_dir.as_str(),
                experiment_str,
                meta_results.timestamps,
                meta_results.leader_changes,
                meta_results.reconfig_ts,
            );
        }
    }

    fn persist_meta_summary_results(&mut self) {
        let meta_path = self.meta_results_path.as_ref().unwrap();
        let experiment_str = self.experiment_str.as_ref().unwrap();
        let num_timed_out = std::mem::take(&mut self.num_timed_out);
        let num_retried = std::mem::take(&mut self.num_retried);
        persist_timeouts_retried_summary(
            meta_path,
            experiment_str,
            num_timed_out,
            num_retried,
            self.iteration_id as u64,
        );
        if self.concurrent_proposals == Some(1) || cfg!(feature = "track_latency") {
            let latency_dir = self.get_meta_results_dir(Some("latency"));
            let hist = self.latency_hist.take().unwrap();
            persist_latency_summary(latency_dir.as_str(), experiment_str, hist);
        }
    }

    fn get_meta_results_and_stop_client(
        &mut self,
        client: ActorRef<LocalClientMessage>,
    ) -> MetaResults {
        let meta_results: MetaResults = client
            .ask_with(|promise| LocalClientMessage::Stop(Ask::new(promise, ())))
            .wait();
        meta_results
    }

    fn kill_client(&mut self) {
        let system = self.system.as_mut().unwrap();
        let kill_client_f = match self.reconfiguration {
            None => {
                let client = self.client_comp.take().unwrap();
                system.kill_notify(client)
            },
            Some(_) => {
                let reconfig_client = self.reconfig_client_comp.take().unwrap();
                system.kill_notify(reconfig_client)
            }
        };
        kill_client_f
            .wait_timeout(REGISTER_TIMEOUT)
            .expect("Client never died");
    }

    fn kill_partitioning_component(&mut self) {
        let system = self.system.as_mut().unwrap();
        if let Some(partitioning_actor) = self.partitioning_actor.take() {
            let kill_pactor_f = system.kill_notify(partitioning_actor);
            kill_pactor_f
                .wait_timeout(REGISTER_TIMEOUT)
                .expect("Partitioning Actor never died!");
        }
    }

    fn clear_state(&mut self) {
        self.num_nodes = None;
        self.num_nodes_needed = None;
        self.exp_duration = None;
        self.concurrent_proposals = None;
        self.reconfiguration = None;
        self.network_scenario = None;
        self.election_timeout_ms = None;
        self.iteration_id = 0;
        self.latency_hist = None;
        self.num_timed_out.clear();
        self.num_retried.clear();
        self.experiment_str = None;
        self.meta_results_path = None;
        self.meta_results_sub_dir = None;
        self.lagging_delay_ms = 0;
    }

    /// returns true if the experiment is a test run according to the test parameters defined in Benchmarks.scala
    fn is_test_run(&self) -> bool {
        self.num_nodes.unwrap_or_default() == 3
            && self.concurrent_proposals.unwrap_or_default() == 200
            && self.network_scenario == Some(NetworkScenario::FullyConnected)
            && self.exp_duration.unwrap_or_default() == Duration::from_secs(10)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ReconfigurationPolicy {
    ReplaceLeader,
    ReplaceFollower,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum NetworkScenario {
    FullyConnected,
    QuorumLoss(Duration),
    ConstrainedElection(Duration),
    Chained(Duration),
    PeriodicFull(Duration),
}
