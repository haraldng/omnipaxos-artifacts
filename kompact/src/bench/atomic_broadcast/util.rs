use crate::bench::atomic_broadcast::client::WindowedResult;
use quanta::Instant;
use std::time::{Duration, SystemTime};

pub(crate) mod exp_util {
    #[cfg(feature = "measure_io")]
    use crate::bench::atomic_broadcast::util::io_metadata::IOMetaData;
    use benchmark_suite_shared::{
        kompics_benchmarks::benchmarks::AtomicBroadcastRequest, BenchmarkError,
    };
    use hocon::HoconLoader;
    #[cfg(feature = "measure_io")]
    use std::{
        fs::{create_dir_all, File, OpenOptions},
        io::Write,
        time::{SystemTime, UNIX_EPOCH},
    };
    use std::{path::PathBuf, time::Duration};

    pub const TCP_NODELAY: bool = true;
    pub const CONFIG_PATH: &str = "./configs/atomic_broadcast.conf";
    pub const PAXOS_PATH: &str = "paxos_replica";
    pub const RAFT_PATH: &str = "raft_replica";
    pub const MULTIPAXOS_PATH: &str = "mp_replica";
    pub const REGISTER_TIMEOUT: Duration = Duration::from_secs(5);

    pub const LAGGING_DELAY_FACTOR: u64 = 2;
    pub const WINDOW_DURATION: Duration = Duration::from_millis(5000);
    pub const DATA_SIZE: usize = 8;
    pub const WARMUP_DURATION: Duration = Duration::from_secs(60);
    pub const COOLDOWN_DURATION: Duration = WARMUP_DURATION;

    pub struct ExperimentParams {
        pub election_timeout_ms: u64,
        pub outgoing_period: Duration,
        pub max_inflight: usize,
        pub initial_election_factor: u64,
        pub preloaded_log_size: u64,
        #[allow(dead_code)]
        io_meta_results_path: String,
        #[allow(dead_code)]
        experiment_str: String,
    }

    impl ExperimentParams {
        pub fn load_from_file(
            path: &str,
            run_id: &str,
            meta_sub_dir: String,
            experiment_str: String,
            election_timeout_ms: u64,
        ) -> ExperimentParams {
            let p: PathBuf = path.into();
            let config = HoconLoader::new()
                .load_file(p)
                .expect("Failed to load file")
                .hocon()
                .expect("Failed to load as HOCON");
            let outgoing_period = config["experiment"]["outgoing_period"]
                .as_duration()
                .expect("Failed to load outgoing_period");
            let max_inflight = config["experiment"]["max_inflight"]
                .as_i64()
                .expect("Failed to load max_inflight") as usize;
            let initial_election_factor = config["experiment"]["initial_election_factor"]
                .as_i64()
                .expect("Failed to load initial_election_factor")
                as u64;
            let preloaded_log_size = config["experiment"]["preloaded_log_size"]
                .as_i64()
                .expect("Failed to load preloaded_log_size")
                as u64;
            let io_meta_results_path = format!("../meta_results/{}/io/{}/", run_id, meta_sub_dir); // meta_results/3-10k/io/paxos,3,10000.data
            ExperimentParams {
                election_timeout_ms,
                outgoing_period,
                max_inflight,
                initial_election_factor,
                preloaded_log_size,
                io_meta_results_path,
                experiment_str,
            }
        }

        #[cfg(feature = "measure_io")]
        pub fn get_io_file(&self, direction: &str, component: &str) -> File {
            let dir = format!(
                "{}/{}/{}",
                self.io_meta_results_path.as_str(),
                direction,
                self.experiment_str.as_str(),
            );
            create_dir_all(dir.clone()).expect("Failed to create io meta directory: {}");
            let io_file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(format!("{}/{}.data", dir, component))
                .expect("Failed to open file: {}");
            io_file
        }
    }

    pub fn create_experiment_str(c: &AtomicBroadcastRequest) -> String {
        format!(
            "{},{},{},{},{},{},{},{}",
            c.algorithm,
            c.number_of_nodes,
            c.concurrent_proposals,
            format!("{}min", c.duration_secs / 60), // TODO
            c.reconfiguration.clone(),
            c.reconfig_policy,
            c.network_scenario,
            format!("{}ms", c.election_timeout_ms)
        )
    }

    pub fn create_metaresults_sub_dir(
        number_of_nodes: u64,
        concurrent_proposals: u64,
        reconfiguration: &str,
    ) -> String {
        format!(
            "{}-{}-{}",
            number_of_nodes, concurrent_proposals, reconfiguration
        )
    }

    pub fn get_reconfig_nodes(s: &str, n: u64) -> Result<Option<Vec<u64>>, BenchmarkError> {
        match s.to_lowercase().as_ref() {
            "off" => Ok(None),
            "single" => Ok(Some(vec![n + 1])),
            "majority" => {
                let majority = n / 2 + 1;
                let new_nodes: Vec<u64> = (n + 1..n + 1 + majority).collect();
                Ok(Some(new_nodes))
            }
            _ => Err(BenchmarkError::InvalidMessage(String::from(
                "Got unknown reconfiguration parameter",
            ))),
        }
    }

    #[cfg(feature = "measure_io")]
    pub fn persist_io_metadata(
        io_windows: Vec<(SystemTime, IOMetaData)>,
        in_file: &mut File,
        out_file: &mut File,
    ) {
        let mut received_str = String::new();
        let mut sent_str = String::new();
        let total = io_windows
            .iter()
            .fold(IOMetaData::default(), |sum, (ts, io_meta)| {
                received_str.push_str(&format!(
                    "{:?} | {} {}\n",
                    ts.duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    io_meta.get_num_received(),
                    io_meta.get_received_kb(),
                ));
                sent_str.push_str(&format!(
                    "{:?} | {} {}\n",
                    ts.duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    io_meta.get_num_sent(),
                    io_meta.get_sent_kb(),
                ));
                sum + (*io_meta)
            });
        let total_received = format!(
            "total | {} {}",
            total.get_num_received(),
            total.get_received_kb()
        );
        let total_sent = format!(
            "total | {} {}",
            total.get_num_received(),
            total.get_received_kb()
        );
        writeln!(in_file, "{}\n{}", total_received, received_str).expect("Failed to write IO file");
        writeln!(out_file, "{}\n{}", total_sent, sent_str).expect("Failed to write IO file");

        in_file.flush().expect("Failed to flush IO in file");
        out_file.flush().expect("Failed to flush IO out file");
    }
}

pub mod benchmark_master {
    use crate::bench::atomic_broadcast::client::WindowedResult;
    use chrono::{DateTime, Utc};
    use hashbrown::HashMap;
    use hdrhistogram::Histogram;
    use hocon::HoconLoader;
    use kompact::prelude::{ActorPath, SystemField};
    use quanta::Instant;
    use std::{
        fs::{create_dir_all, OpenOptions},
        io::Write,
        path::PathBuf,
        time::{Duration, SystemTime},
    };

    /// reads hocon config file and returns (timeout, meta_results path, size of preloaded_log)
    pub fn load_benchmark_config<P>(path: P) -> (Duration, u64)
    where
        P: Into<PathBuf>,
    {
        let p: PathBuf = path.into();
        let config = HoconLoader::new()
            .load_file(p)
            .expect("Failed to load file")
            .hocon()
            .expect("Failed to load as HOCON");
        let client_timeout = config["experiment"]["client_normal_timeout"]
            .as_duration()
            .expect("Failed to load client normal timeout");
        let preloaded_log_size = if cfg!(feature = "preloaded_log") {
            config["experiment"]["preloaded_log_size"]
                .as_i64()
                .expect("Failed to load preloaded_log_size") as u64
        } else {
            0
        };
        (client_timeout, preloaded_log_size)
    }

    #[cfg(feature = "simulate_partition")]
    /// reads hocon config file and returns (timeout, meta_results path, size of preloaded_log)
    pub fn load_client_timeout_factor<P>(path: P) -> u64
    where
        P: Into<PathBuf>,
    {
        let p: PathBuf = path.into();
        let config = HoconLoader::new()
            .load_file(p)
            .expect("Failed to load file")
            .hocon()
            .expect("Failed to load as HOCON");
        let client_timeout_factor = config["experiment"]["client_timeout_factor"]
            .as_i64()
            .expect("Failed to load client timeout factor")
            as u64;
        client_timeout_factor
    }

    pub fn load_pid_map<P>(path: P, nodes: &[ActorPath]) -> HashMap<ActorPath, u32>
    where
        P: Into<PathBuf>,
    {
        let p: PathBuf = path.into();
        let config = HoconLoader::new()
            .load_file(p)
            .expect("Failed to load file")
            .hocon()
            .expect("Failed to load as HOCON");
        let mut pid_map = HashMap::with_capacity(nodes.len());
        for ap in nodes {
            let addr_str = ap.address().to_string();
            let pid = config["deployment"][addr_str.as_str()]
                .as_i64()
                .unwrap_or_else(|| panic!("Failed to load pid map of {}", addr_str))
                as u32;
            pid_map.insert(ap.clone(), pid);
        }
        pid_map
    }

    pub fn persist_timestamp_results(
        timestamps_dir: &str,
        experiment_str: &str,
        timestamps: Vec<Instant>,
        leader_changes: Vec<(SystemTime, (u64, u64))>,
        reconfig_ts: Option<(SystemTime, SystemTime)>,
    ) {
        // let timestamps_dir = self.get_meta_results_dir(Some("timestamps"));
        create_dir_all(&timestamps_dir)
            .unwrap_or_else(|_| panic!("Failed to create given directory: {}", &timestamps_dir));
        let mut timestamps_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(format!("{}/raw_{}.data", &timestamps_dir, experiment_str))
            .expect("Failed to open timestamps file");

        if let Some((reconfig_start, reconfig_end)) = reconfig_ts {
            let start_ts = DateTime::<Utc>::from(reconfig_start);
            let end_ts = DateTime::<Utc>::from(reconfig_end);
            writeln!(timestamps_file, "r,{},{} ", start_ts, end_ts)
                .expect("Failed to write reconfig timestamps to timestamps file");
        }
        for (leader_change_ts, lc) in leader_changes {
            let ts = DateTime::<Utc>::from(leader_change_ts);
            writeln!(timestamps_file, "l,{},{:?} ", ts, lc)
                .expect("Failed to write leader changes to timestamps file");
        }
        for ts in timestamps {
            let timestamp = ts.as_u64();
            writeln!(timestamps_file, "{}", timestamp)
                .expect("Failed to write raw timestamps file");
        }
        writeln!(timestamps_file, "").unwrap();
        timestamps_file
            .flush()
            .expect("Failed to flush raw timestamps file");
    }

    pub fn persist_windowed_results(
        windowed_dir: &str,
        experiment_str: &str,
        windowed_res: WindowedResult,
    ) {
        // let windowed_dir = self.get_meta_results_dir(Some("windowed"));
        create_dir_all(windowed_dir)
            .unwrap_or_else(|_| panic!("Failed to create given directory: {}", &windowed_dir));
        let mut windowed_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(format!("{}/{}.data", windowed_dir, experiment_str))
            .expect("Failed to open windowed file");
        writeln!(windowed_file, "\n{} ", windowed_res.num_warmup_windows)
            .expect("Failed to write number of warmup windows");
        let mut prev_n = 0;
        for n in windowed_res.windows {
            write!(windowed_file, "{},", n - prev_n).expect("Failed to write windowed file");
            prev_n = n;
        }
        windowed_file
            .flush()
            .expect("Failed to flush windowed file");
    }

    pub fn persist_num_decided_results(
        num_decided_dir: &str,
        experiment_str: &str,
        num_decided: u64,
        num_warmup_decided: u64,
    ) {
        create_dir_all(num_decided_dir)
            .unwrap_or_else(|_| panic!("Failed to create given directory: {}", num_decided_dir));
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(format!("{}/{}.data", num_decided_dir, experiment_str))
            .expect("Failed to open num_decided file");
        writeln!(file, "{} | {}", num_decided, num_warmup_decided)
            .expect("Failed to write num_decided");
        file.flush().expect("Failed to flush num_decided file");
    }

    #[cfg(feature = "simulate_partition")]
    pub fn persist_longest_down_time_results(
        longest_down_time_dir: &str,
        experiment_str: &str,
        down_time: Duration,
    ) {
        create_dir_all(longest_down_time_dir).unwrap_or_else(|_| {
            panic!(
                "Failed to create given directory: {}",
                longest_down_time_dir
            )
        });
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(format!("{}/{}.data", longest_down_time_dir, experiment_str))
            .expect("Failed to open longest down time file");
        let down_time_millis = down_time.as_millis();
        writeln!(file, "{}", down_time_millis).expect("Failed to write longest down time");
        file.flush()
            .expect("Failed to flush longest down time file");
    }

    pub fn persist_latency_results(
        _latency_dir: &str,
        _experiment_str: &str,
        latencies: &[Duration],
        histo: &mut Histogram<u64>,
    ) {
        /*
        create_dir_all(&latency_dir)
            .unwrap_or_else(|_| panic!("Failed to create given directory: {}", &latency_dir));
        let mut latency_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(format!("{}/raw_{}.data", &latency_dir, experiment_str))
            .expect("Failed to open latency file");
        */
        for l in latencies {
            let latency = l.as_millis() as u64;
            histo.record(latency).expect("Failed to record histogram");
            // writeln!(latency_file, "{}", latency).expect("Failed to write raw latency");
        }
        /*
        latency_file
            .flush()
            .expect("Failed to flush raw latency file");
        */
    }

    pub fn persist_latency_summary(latency_dir: &str, experiment_str: &str, hist: Histogram<u64>) {
        // let dir = self.get_meta_results_dir(Some("latency"));
        create_dir_all(latency_dir)
            .unwrap_or_else(|_| panic!("Failed to create given directory: {}", latency_dir));
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(format!("{}/summary_{}.out", latency_dir, experiment_str))
            .expect("Failed to open latency file");
        // let hist = std::mem::take(&mut self.latency_hist).unwrap();
        let quantiles = [
            0.001, 0.01, 0.005, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 0.99, 0.999,
        ];
        for q in &quantiles {
            writeln!(
                file,
                "Value at quantile {}: {} micro s",
                q,
                hist.value_at_quantile(*q)
            )
            .expect("Failed to write summary latency file");
        }
        let max = hist.max();
        writeln!(
            file,
            "Min: {} micro s, Max: {} micro s, Average: {} micro s",
            hist.min(),
            max,
            hist.mean()
        )
        .expect("Failed to write histogram summary");
        writeln!(file, "Total elements: {}", hist.len())
            .expect("Failed to write histogram summary");
        file.flush().expect("Failed to flush histogram file");
    }

    pub fn persist_timeouts_retried_summary(
        meta_path: &str,
        experiment_str: &str,
        num_timed_out: Vec<u64>,
        num_retried: Vec<u64>,
        num_iterations: u64,
    ) {
        let mut num_timed_out = num_timed_out;
        let mut num_retried = num_retried;
        let timed_out_sum: u64 = num_timed_out.iter().sum();
        let retried_sum: u64 = num_retried.iter().sum();
        if timed_out_sum > 0 || retried_sum > 0 {
            let summary_file_path = format!("{}/summary.out", meta_path);
            create_dir_all(meta_path).unwrap_or_else(|_| {
                panic!("Failed to create given directory: {}", summary_file_path)
            });
            let mut summary_file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(summary_file_path)
                .expect("Failed to open meta summary file");
            writeln!(summary_file, "{}", experiment_str)
                .expect("Failed to write meta summary file");

            if timed_out_sum > 0 {
                let len = num_timed_out.len();
                let timed_out_len = num_timed_out.iter().filter(|x| **x > 0).count();
                num_timed_out.sort();
                let min = num_timed_out.first().unwrap();
                let max = num_timed_out.last().unwrap();
                let avg = timed_out_sum / num_iterations;
                let median = num_timed_out[len / 2];
                let timed_out_summary_str = format!(
                    "{}/{} runs had timeouts. sum: {}, avg: {}, med: {}, min: {}, max: {}",
                    timed_out_len, len, timed_out_sum, avg, median, min, max
                );
                writeln!(summary_file, "{}", timed_out_summary_str).unwrap_or_else(|_| {
                    panic!(
                        "Failed to write meta summary file: {}",
                        timed_out_summary_str
                    )
                });
            }
            if retried_sum > 0 {
                let len = num_retried.len();
                let retried_len = num_retried.iter().filter(|x| **x > 0).count();
                num_retried.sort_unstable();
                let min = num_retried.first().unwrap();
                let max = num_retried.last().unwrap();
                let avg = retried_sum / num_iterations;
                let median = num_timed_out[len / 2];
                let retried_summary_str = format!(
                    "{}/{} runs had retries. sum: {}, avg: {}, med: {}, min: {}, max: {}",
                    retried_len, len, retried_sum, avg, median, min, max
                );
                writeln!(summary_file, "{}", retried_summary_str).unwrap_or_else(|_| {
                    panic!("Failed to write meta summary file: {}", retried_summary_str)
                });
            }
            summary_file.flush().expect("Failed to flush meta file");
        }
    }
}

#[cfg(feature = "measure_io")]
pub mod io_metadata {
    use pretty_bytes::converter::convert;
    use std::{fmt, ops::Add};

    #[derive(Copy, Clone, Default, Eq, PartialEq, Debug)]
    pub struct IOMetaData {
        msgs_sent: usize,
        bytes_sent: usize,
        msgs_received: usize,
        bytes_received: usize,
    }

    impl IOMetaData {
        pub fn update_received<T>(&mut self, msg: &T) {
            let size = std::mem::size_of_val(msg);
            self.bytes_received += size;
            self.msgs_received += 1;
        }

        pub fn update_sent<T>(&mut self, msg: &T) {
            let size = std::mem::size_of_val(msg);
            self.bytes_sent += size;
            self.msgs_sent += 1;
        }

        pub fn update_sent_with_size(&mut self, size: usize) {
            self.bytes_sent += size;
            self.msgs_sent += 1;
        }

        pub fn update_received_with_size(&mut self, size: usize) {
            self.bytes_received += size;
            self.msgs_received += 1;
        }

        pub fn reset(&mut self) {
            self.msgs_received = 0;
            self.bytes_received = 0;
            self.msgs_sent = 0;
            self.bytes_sent = 0;
        }

        pub fn get_num_received(&self) -> usize {
            self.msgs_received
        }

        pub fn get_num_sent(&self) -> usize {
            self.msgs_sent
        }

        pub fn get_received_kb(&self) -> usize {
            self.bytes_received / 1000
        }

        pub fn get_sent_kb(&self) -> usize {
            self.bytes_sent / 1000
        }

        pub fn get_received_bytes(&self) -> usize {
            self.bytes_received
        }

        pub fn get_sent_bytes(&self) -> usize {
            self.bytes_sent
        }
    }

    impl Add for IOMetaData {
        type Output = Self;

        fn add(self, other: Self) -> Self {
            Self {
                msgs_received: self.msgs_received + other.msgs_received,
                bytes_received: self.bytes_received + other.bytes_received,
                msgs_sent: self.msgs_sent + other.msgs_sent,
                bytes_sent: self.bytes_sent + other.bytes_sent,
            }
        }
    }
}

#[derive(Debug)]
pub struct MetaResults {
    pub num_decided: u64,
    pub num_warmup_decided: u64,
    pub num_timed_out: u64,
    pub num_retried: u64,
    pub latencies: Vec<Duration>,
    pub leader_changes: Vec<(SystemTime, (u64, u64))>,
    pub windowed_results: WindowedResult,
    pub reconfig_ts: Option<(SystemTime, SystemTime)>,
    pub timestamps: Vec<Instant>,
    #[cfg(feature = "simulate_partition")]
    pub longest_down_time: Duration,
}
