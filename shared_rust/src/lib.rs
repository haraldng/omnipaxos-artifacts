#![feature(unsized_locals)]
pub mod benchmark;
pub mod benchmark_client;
pub mod benchmark_master;
pub mod benchmark_runner;
pub mod helpers;
pub mod kompics_benchmarks;

pub use self::benchmark::*;
use self::kompics_benchmarks::*;
#[allow(unused_imports)]
use slog::{crit, debug, error, info, o, warn, Drain, Logger};
use slog_scope;
use slog_stdlog;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    thread,
    time::Duration,
};
//pub(crate) type BenchLogger = Logger;
pub struct BenchmarkMain;
impl BenchmarkMain {
    pub fn run_with<H, F>(
        args: Vec<String>,
        runner: H,
        benchmarks: Box<dyn BenchmarkFactory>,
        set_public_if: F,
    ) -> ()
    where
        H: benchmarks_grpc::BenchmarkRunner + Clone + Sync + Send + 'static,
        F: FnOnce(IpAddr),
    {
        let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
        let logger = Logger::root(slog_term::FullFormat::new(plain).build().fuse(), o!());

        info!(logger, "The root logger works!");
        // Args: ["/Users/haraldng/code/kompicsbenches/kompact/target/release/kompact_benchmarks", "127.0.0.1:45678", "127.0.0.1:45679", "3"]

        let _scope_guard = slog_scope::set_global_logger(logger.clone());
        let _log_guard = slog_stdlog::init().unwrap();

        if args.len() <= 3 {
            // local mode
            let bench_runner_addr: String =
                args.get(1).map(|s| s.clone()).unwrap_or("127.0.0.1:45678".to_string());

            benchmark_runner::run_server(runner, bench_runner_addr, None)
        } else if args.len() == 4 {
            // client mode
            let master_addr: SocketAddr =
                args[1].parse().expect("Could not convert arg to socket address!");
            let client_addr: SocketAddr =
                args[2].parse().expect("Could not convert arg to socket address!");
            println!("Running in client mode with master={}, client={}", master_addr, client_addr);
            set_public_if(client_addr.ip());
            benchmark_client::run(
                client_addr.ip(),
                client_addr.port(),
                master_addr.ip(),
                master_addr.port(),
                benchmarks,
                logger.new(o!("ty" => "benchmark_client::run")),
            );
            unreachable!("This should not return!");
        } else if args.len() == 5 {
            // master mode
            let bench_runner_addr: SocketAddr =
                args[1].parse().expect("Could not convert arg to socket address!");
            let master_addr: SocketAddr =
                args[2].parse().expect("Could not convert arg to socket address!");
            let num_clients: usize = args[3]
                .parse()
                .expect("Could not convert arg to unsigned integer number of clients.");
            let run_id: String = args[4].clone();
            println!(
                "Running in master mode with runner={}, master={}, #clients={}",
                bench_runner_addr, master_addr, num_clients
            );
            set_public_if(master_addr.ip());
            benchmark_master::run(
                bench_runner_addr.port(),
                master_addr.port(),
                num_clients,
                benchmarks,
                logger.new(o!("ty" => "benchmark_master::run")),
                run_id
            );
            unreachable!("This should not return!");
        } else {
            panic!("Too many args={} provided!", args.len());
        };
    }
}

fn force_shutdown() {
    std::thread::spawn(|| {
        thread::sleep(Duration::from_millis(500));
        std::process::exit(0);
    });
}

pub mod test_utils {
    use super::*;
    use benchmarks_grpc::BenchmarkRunner;
    use futures::future::Future;
    use grpc::ClientStubExt;
    use itertools::Itertools;
    use std::{
        clone::Clone,
        net::SocketAddr,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };

    pub fn test_implementation<F>(benchmarks: Box<F>)
    where F: BenchmarkFactory + Clone + 'static {
        let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
        let logger = Logger::root(slog_term::FullFormat::new(plain).build().fuse(), o!());

        info!(logger, "The root logger works!");

        let scope_guard = slog_scope::set_global_logger(logger.clone());
        scope_guard.cancel_reset(); // prevent one test removing the other's logger when running in parallel
        let _ = slog_stdlog::init(); // ignore the error if the other implementation already set the logger

        let runner_addr: SocketAddr = "127.0.0.1:45678".parse().expect("runner address");
        let master_addr: SocketAddr = "127.0.0.1:45679".parse().expect("master address");
        let client_ports: [u16; 4] = [45680, 45681, 45682, 45683];
        let client_addrs: [SocketAddr; 4] =
            client_ports.map(|port| SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port));

        let mut implemented: Vec<String> = Vec::new();
        let mut not_implemented: Vec<String> = Vec::new();

        let _check_result = |label: &str, tr: messages::TestResult| {
            if tr.has_success() {
                let s = tr.get_success();
                assert_eq!(s.run_results.len(), s.number_of_runs as usize);
                implemented.push(label.into());
            } else if tr.has_failure() {
                let f = tr.get_failure();
                assert!(
                    f.reason.contains("RSE"),
                    "Tests are only allowed to fail because of RSE requirements!"
                ); // since tests are short they may not meet RSE requirements
                implemented.push(label.into());
            } else if tr.has_not_implemented() {
                warn!(logger, "Test {} was not implemented", label);
                not_implemented.push(label.into());
            } else {
                panic!("Unexpected test result: {:?}", tr);
            }
        };

        let num_clients = 4;

        let benchmarks1 = benchmarks.clone();

        let master_logger = logger.clone();

        let run_id = "test".to_string();

        let master_handle = std::thread::Builder::new()
            .name("benchmark_master".to_string())
            .spawn(move || {
                info!(master_logger, "Starting master");
                benchmark_master::run(
                    runner_addr.port(),
                    master_addr.port(),
                    num_clients,
                    benchmarks1,
                    master_logger.new(o!("ty" => "benchmark_master::run")),
                    run_id
                );
                info!(master_logger, "Finished master");
            })
            .expect("master thread");

        let client_handles = client_addrs.map(|client_addr_ref| {
            let client_addr = client_addr_ref.clone();
            let bench = benchmarks.clone();
            let client_logger = logger.clone();
            let thread_name = format!("benchmark_client-{}", client_addr.port());
            let client_handle = std::thread::Builder::new()
                .name(thread_name)
                .spawn(move || {
                    info!(client_logger, "Starting client {}", client_addr);
                    benchmark_client::run(
                        client_addr.ip(),
                        client_addr.port(),
                        master_addr.ip(),
                        master_addr.port(),
                        bench,
                        client_logger
                            .new(o!("ty" => "benchmark_client::run", "addr" => client_addr)),
                    );
                    info!(client_logger, "Finished client {}", client_addr);
                })
                .expect("client thread");
            client_handle
        });

        let bench_stub = benchmarks_grpc::BenchmarkRunnerClient::new_plain(
            &runner_addr.ip().to_string(),
            runner_addr.port(),
            Default::default(),
        )
        .expect("bench stub");

        let mut attempts = 0;
        while attempts < 20 {
            attempts += 1;
            info!(logger, "Checking if ready, attempt #{}", attempts);
            let ready_f =
                bench_stub.ready(grpc::RequestOptions::default(), messages::ReadyRequest::new());
            match ready_f.drop_metadata().wait() {
                Ok(res) => {
                    if res.status {
                        info!(logger, "Was ready.");
                        break;
                    } else {
                        info!(logger, "Wasn't ready, yet.");
                        std::thread::sleep(Duration::from_millis(500));
                    }
                },
                Err(e) => {
                    info!(logger, "Couldn't connect, yet: {}", e);
                    std::thread::sleep(Duration::from_millis(500));
                },
            }
        }

        info!(logger, "Sending shutdown request to master");
        let mut sreq = messages::ShutdownRequest::new();
        sreq.set_force(false);
        let _shutdownres_f =
            bench_stub.shutdown(grpc::RequestOptions::default(), sreq).drop_metadata();

        info!(logger, "Waiting for master to finish...");
        master_handle.join().expect("Master panicked!");
        info!(logger, "Master is done.");
        info!(logger, "Waiting for all clients to finish...");
        match client_handles {
            [ch0, ch1, ch2, ch3] => {
                ch0.join().expect("Client panicked!");
                ch1.join().expect("Client panicked!");
                ch2.join().expect("Client panicked!");
                ch3.join().expect("Client panicked!");
            },
        }
        info!(logger, "All clients are done.");

        info!(
            logger,
            "
%%%%%%%%%%%%%%%%%%%%%%%%%%%
%% MASTER-CLIENT SUMMARY %%
%%%%%%%%%%%%%%%%%%%%%%%%%%%
{} tests implemented: {:?}
{} tests not implemented: {:?}
",
            implemented.len(),
            implemented,
            not_implemented.len(),
            not_implemented
        );
    }

    pub fn test_local_implementation<H>(runner: H)
    where H: benchmarks_grpc::BenchmarkRunner + Clone + Sync + Send + 'static {
        let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
        let logger = Logger::root(slog_term::FullFormat::new(plain).build().fuse(), o!());

        info!(logger, "The root logger works!");

        let scope_guard = slog_scope::set_global_logger(logger.clone());
        scope_guard.cancel_reset(); // prevent one test removing the other's logger when running in parallel
        let _ = slog_stdlog::init(); // ignore the error if the other implementation already set the logger

        let runner_addr: SocketAddr = "127.0.0.1:45677".parse().expect("runner address");

        let mut implemented: Vec<String> = Vec::new();
        let mut not_implemented: Vec<String> = Vec::new();

        let _check_result = |label: &str, tr: messages::TestResult| {
            if tr.has_success() {
                let s = tr.get_success();
                assert_eq!(s.run_results.len(), s.number_of_runs as usize);
                implemented.push(label.into());
            } else if tr.has_failure() {
                let f = tr.get_failure();
                assert!(
                    f.reason.contains("RSE"),
                    "Tests are only allowed to fail because of RSE requirements!"
                ); // since tests are short they may not meet RSE requirements
                implemented.push(label.into());
            } else if tr.has_not_implemented() {
                warn!(logger, "Test {} was not implemented", label);
                not_implemented.push(label.into());
            } else {
                panic!("Unexpected test result: {:?}", tr);
            }
        };

        let runner_logger = logger.clone();
        let runner_shutdown = Arc::new(AtomicBool::new(false));
        let runner_shutdown2 = runner_shutdown.clone();

        let runner_handle = std::thread::Builder::new()
            .name("benchmark_runner".to_string())
            .spawn(move || {
                info!(runner_logger, "Starting runner");
                benchmark_runner::run_server(
                    runner,
                    runner_addr.to_string(),
                    Some(runner_shutdown2),
                );
                info!(runner_logger, "Finished runner");
            })
            .expect("runner thread");

        let bench_stub = benchmarks_grpc::BenchmarkRunnerClient::new_plain(
            &runner_addr.ip().to_string(),
            runner_addr.port(),
            Default::default(),
        )
        .expect("bench stub");

        let mut attempts = 0;
        while attempts < 20 {
            attempts += 1;
            info!(logger, "Checking if ready, attempt #{}", attempts);
            let ready_f =
                bench_stub.ready(grpc::RequestOptions::default(), messages::ReadyRequest::new());
            match ready_f.drop_metadata().wait() {
                Ok(res) => {
                    if res.status {
                        info!(logger, "Was ready.");
                        break;
                    } else {
                        info!(logger, "Wasn't ready, yet.");
                        std::thread::sleep(Duration::from_millis(500));
                    }
                },
                Err(e) => {
                    info!(logger, "Couldn't connect, yet: {}", e);
                    std::thread::sleep(Duration::from_millis(500));
                },
            }
        }

       info!(logger, "Sending shutdown request to runner");
        runner_shutdown.store(true, Ordering::Relaxed);
        runner_handle.thread().unpark();

        info!(logger, "Waiting for runner to finish...");
        runner_handle.join().expect("Runner panicked!");
        info!(logger, "Runner is done.");

        info!(
            logger,
            "
%%%%%%%%%%%%%%%%%%%%%%%%
%% LOCAL TEST SUMMARY %%
%%%%%%%%%%%%%%%%%%%%%%%%
{} tests implemented: {:?}
{} tests not implemented: {:?}
",
            implemented.len(),
            implemented,
            not_implemented.len(),
            not_implemented
        );
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum KVOperation {
        ReadInvokation,
        ReadResponse,
        WriteInvokation,
        WriteResponse,
    }

    #[derive(Clone, Copy, Debug)]
    pub struct KVTimestamp {
        pub key:       u64,
        pub operation: KVOperation,
        pub value:     Option<u32>,
        pub time:      i64,
        pub sender:    u32,
    }

    pub fn all_linearizable(timestamps: &Vec<KVTimestamp>) -> bool {
        for (_key, mut trace) in timestamps.into_iter().map(|x| (x.key, *x)).into_group_map() {
            trace.sort_by_key(|x| x.time);
            let mut s: Vec<u32> = vec![0];
            if !is_linearizable(&trace, s.as_mut()) {
                return false;
            }
        }
        true
    }

    fn is_linearizable(h: &Vec<KVTimestamp>, s: &mut Vec<u32>) -> bool {
        match h.is_empty() {
            true => true,
            false => {
                let minimal_ops = get_minimal_ops(h);
                for op in minimal_ops {
                    let response_val = get_response_value(op, h).unwrap();
                    match op.operation {
                        KVOperation::WriteInvokation | KVOperation::WriteResponse => {
                            //                            println!("current value={}, write {}", s.last().unwrap(), response_val);
                            let removed_op_trace = remove_op(op, h);
                            s.push(response_val);
                            if is_linearizable(&removed_op_trace, s) {
                                return true;
                            } else {
                                s.pop();
                            }
                        },
                        KVOperation::ReadInvokation | KVOperation::ReadResponse => {
                            //                            println!("current value={}, read {}", s.last().unwrap(), response_val);
                            let removed_op_trace = remove_op(op, h);
                            if s.last().unwrap() == &response_val
                                && is_linearizable(&removed_op_trace, s)
                            {
                                return true;
                            }
                        },
                    }
                }
                false
            },
        }
    }

    fn get_minimal_ops(trace: &Vec<KVTimestamp>) -> Vec<&KVTimestamp> {
        let mut minimal_ops = Vec::new();
        for entry in trace {
            match entry.operation {
                KVOperation::ReadInvokation | KVOperation::WriteInvokation => {
                    minimal_ops.push(entry)
                },
                _ => break,
            }
        }
        minimal_ops.clone()
    }

    fn get_response_value(invokation: &KVTimestamp, trace: &Vec<KVTimestamp>) -> Option<u32> {
        match invokation.operation {
            KVOperation::ReadInvokation => {
                let f = trace.iter().find(|&&x| {
                    x.operation == KVOperation::ReadResponse && x.sender == invokation.sender
                });
                match f {
                    Some(ts) => ts.value,
                    _ => None,
                }
            },
            _ => invokation.value,
        }
    }

    fn remove_op(entry: &KVTimestamp, trace: &Vec<KVTimestamp>) -> Vec<KVTimestamp> {
        let mut cloned: Vec<KVTimestamp> = trace.clone();
        cloned.retain(|&x| x.sender != entry.sender);
        assert!(cloned.len() < trace.len());
        cloned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::Future;
    use test_utils::{test_implementation, test_local_implementation};

    #[derive(Default)]
    struct TestLocalBench;
    impl Benchmark for TestLocalBench {
        type Conf = ();
        type Instance = TestLocalBenchI;

        const LABEL: &'static str = "TestBench";

        fn msg_to_conf(_msg: Box<dyn (::protobuf::Message)>) -> Result<Self::Conf, BenchmarkError> {
            Ok(())
        }

        fn new_instance() -> Self::Instance { TestLocalBenchI {} }
    }
    struct TestLocalBenchI;
    impl BenchmarkInstance for TestLocalBenchI {
        type Conf = ();

        fn setup(&mut self, _c: &Self::Conf) -> () {
            println!("Test got setup.");
        }

        fn prepare_iteration(&mut self) -> () {
            println!("Preparing iteration");
        }

        fn run_iteration(&mut self) -> () {
            println!("Running iteration");
        }

        fn cleanup_iteration(&mut self, last_iteration: bool, _exec_time_millis: f64) -> () {
            let last = if last_iteration { "last" } else { "" };
            println!("Cleaning up after {} iteration", last);
        }
    }

    struct TestDistributedBench {
        //_data: std::marker::PhantomData<M>,
    }
    //impl<M> Send for TestDistributedBench<M> {}
    //impl<M> Sync for TestDistributedBench<M> {}
    struct TestDistributedBenchMaster;
    struct TestDistributedBenchClient;

    impl TestDistributedBench {
        fn new() -> TestDistributedBench {
            TestDistributedBench {
                //_data: std::marker::PhantomData
            }
        }
    }

    impl DistributedBenchmark for TestDistributedBench {
        type Client = TestDistributedBenchClient;
        type ClientConf = ();
        type ClientData = ();
        type Master = TestDistributedBenchMaster;
        type MasterConf = ();

        const LABEL: &'static str = "DistTestBench";

        fn new_master() -> Self::Master { TestDistributedBenchMaster {} }

        fn msg_to_master_conf(
            _msg: Box<dyn (::protobuf::Message)>,
        ) -> Result<Self::MasterConf, BenchmarkError> {
            //downcast_msg!(msg; benchmarks::PingPongRequest; |_ppr| ())
            //downcast_msg!(msg; M; |_ppr| ())
            Ok(())
        }

        fn new_client() -> Self::Client { TestDistributedBenchClient {} }

        fn str_to_client_conf(_str: String) -> Result<Self::ClientConf, BenchmarkError> { Ok(()) }

        fn str_to_client_data(_str: String) -> Result<Self::ClientData, BenchmarkError> { Ok(()) }

        fn client_conf_to_str(_c: Self::ClientConf) -> String { "".to_string() }

        fn client_data_to_str(_d: Self::ClientData) -> String { "".to_string() }
    }

    impl DistributedBenchmarkMaster for TestDistributedBenchMaster {
        type ClientConf = ();
        type ClientData = ();
        type MasterConf = ();

        fn setup(
            &mut self,
            _c: Self::MasterConf,
            _m: &DeploymentMetaData,
        ) -> Result<Self::ClientConf, BenchmarkError> {
            println!("Master setting up");
            Ok(())
        }

        fn prepare_iteration(&mut self, _d: Vec<Self::ClientData>) -> () {
            println!("Master preparing iteration");
        }

        fn run_iteration(&mut self) -> () {
            println!("Master running iteration");
        }

        fn cleanup_iteration(&mut self, last_iteration: bool, _exec_time_millis: f64) -> () {
            let last = if last_iteration { "last" } else { "" };
            println!("Master cleaning up {} iteration", last);
        }
    }

    impl DistributedBenchmarkClient for TestDistributedBenchClient {
        type ClientConf = ();
        type ClientData = ();

        fn setup(&mut self, _c: Self::ClientConf) -> Self::ClientData {
            println!("Client setting up");
        }

        fn prepare_iteration(&mut self) -> () {
            println!("Client preparing iteration");
        }

        fn cleanup_iteration(&mut self, last_iteration: bool) -> () {
            let last = if last_iteration { "last" } else { "" };
            println!("Client cleaning up {} iteration", last);
        }
    }

    #[derive(Clone, Debug)]
    struct TestFactory;

    impl BenchmarkFactory for TestFactory {
        fn by_label(&self, label: &str) -> Result<AbstractBench, NotImplementedError> {
            match label {
                TestLocalBench::LABEL => self.ping_pong().map_into(),
                TestDistributedBench::LABEL => self.net_ping_pong().map_into(),
                _ => Err(NotImplementedError::NotFound),
            }
        }

        fn box_clone(&self) -> Box<dyn BenchmarkFactory> { Box::new(TestFactory {}) }

    }

    impl benchmarks_grpc::BenchmarkRunner for TestFactory {
        fn ready(
            &self,
            _o: grpc::RequestOptions,
            _p: messages::ReadyRequest,
        ) -> grpc::SingleResponse<messages::ReadyResponse> {
            println!("Got ready? req.");
            let mut msg = messages::ReadyResponse::new();
            msg.set_status(true);
            grpc::SingleResponse::completed(msg)
        }

        fn shutdown(
            &self,
            _o: grpc::RequestOptions,
            _p: messages::ShutdownRequest,
        ) -> ::grpc::SingleResponse<messages::ShutdownAck> {
            unimplemented!();
        }

    }

    #[test]
    fn test_client_master() {
        let benchmarks = Box::new(TestFactory {});
        test_implementation(benchmarks);
    }

    #[test]
    fn test_local() {
        let runner = TestFactory {};
        test_local_implementation(runner);
    }
}
