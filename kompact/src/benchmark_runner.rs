use super::*;
use benchmark_suite_shared::{
    benchmark_runner::not_implemented,
    kompics_benchmarks::{benchmarks, benchmarks_grpc, messages},
};

#[derive(Clone)]
pub struct BenchmarkRunnerActorImpl;

impl BenchmarkRunnerActorImpl {
    pub fn new() -> BenchmarkRunnerActorImpl {
        BenchmarkRunnerActorImpl {
            //core: Core::new(),
        }
    }
}

impl benchmarks_grpc::BenchmarkRunner for BenchmarkRunnerActorImpl {
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

    fn atomic_broadcast(
        &self,
        _o: grpc::RequestOptions,
        _p: benchmarks::AtomicBroadcastRequest,
    ) -> grpc::SingleResponse<messages::TestResult> {
        grpc::SingleResponse::completed(not_implemented())
    }
}

#[derive(Clone)]
pub struct BenchmarkRunnerComponentImpl;

impl BenchmarkRunnerComponentImpl {
    pub fn new() -> BenchmarkRunnerComponentImpl {
        BenchmarkRunnerComponentImpl {
            //core: Core::new(),
        }
    }
}

impl benchmarks_grpc::BenchmarkRunner for BenchmarkRunnerComponentImpl {
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

    fn atomic_broadcast(
        &self,
        _o: grpc::RequestOptions,
        _p: benchmarks::AtomicBroadcastRequest,
    ) -> grpc::SingleResponse<messages::TestResult> {
        grpc::SingleResponse::completed(not_implemented())
    }
}

#[derive(Clone)]
pub struct BenchmarkRunnerMixedImpl;

impl BenchmarkRunnerMixedImpl {
    pub fn new() -> BenchmarkRunnerMixedImpl {
        BenchmarkRunnerMixedImpl {}
    }
}

impl benchmarks_grpc::BenchmarkRunner for BenchmarkRunnerMixedImpl {
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

    fn atomic_broadcast(
        &self,
        _o: grpc::RequestOptions,
        _p: benchmarks::AtomicBroadcastRequest,
    ) -> grpc::SingleResponse<messages::TestResult> {
        grpc::SingleResponse::completed(not_implemented())
    }
}
