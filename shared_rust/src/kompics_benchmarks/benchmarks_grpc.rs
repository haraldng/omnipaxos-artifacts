// This file is generated. Do not edit
// @generated

// https://github.com/Manishearth/rust-clippy/issues/702
#![allow(unknown_lints)]
#![allow(clippy::all)]

#![cfg_attr(rustfmt, rustfmt_skip)]

#![allow(box_pointers)]
#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(trivial_casts)]
#![allow(unsafe_code)]
#![allow(unused_imports)]
#![allow(unused_results)]


// interface

pub trait BenchmarkRunner {
    fn ready(&self, o: ::grpc::RequestOptions, p: super::messages::ReadyRequest) -> ::grpc::SingleResponse<super::messages::ReadyResponse>;

    fn shutdown(&self, o: ::grpc::RequestOptions, p: super::messages::ShutdownRequest) -> ::grpc::SingleResponse<super::messages::ShutdownAck>;

    fn atomic_broadcast(&self, o: ::grpc::RequestOptions, p: super::benchmarks::AtomicBroadcastRequest) -> ::grpc::SingleResponse<super::messages::TestResult>;
}

// client

pub struct BenchmarkRunnerClient {
    grpc_client: ::std::sync::Arc<::grpc::Client>,
    method_Ready: ::std::sync::Arc<::grpc::rt::MethodDescriptor<super::messages::ReadyRequest, super::messages::ReadyResponse>>,
    method_Shutdown: ::std::sync::Arc<::grpc::rt::MethodDescriptor<super::messages::ShutdownRequest, super::messages::ShutdownAck>>,
    method_AtomicBroadcast: ::std::sync::Arc<::grpc::rt::MethodDescriptor<super::benchmarks::AtomicBroadcastRequest, super::messages::TestResult>>,
}

impl ::grpc::ClientStub for BenchmarkRunnerClient {
    fn with_client(grpc_client: ::std::sync::Arc<::grpc::Client>) -> Self {
        BenchmarkRunnerClient {
            grpc_client: grpc_client,
            method_Ready: ::std::sync::Arc::new(::grpc::rt::MethodDescriptor {
                name: "/kompics.benchmarks.BenchmarkRunner/Ready".to_string(),
                streaming: ::grpc::rt::GrpcStreaming::Unary,
                req_marshaller: Box::new(::grpc::protobuf::MarshallerProtobuf),
                resp_marshaller: Box::new(::grpc::protobuf::MarshallerProtobuf),
            }),
            method_Shutdown: ::std::sync::Arc::new(::grpc::rt::MethodDescriptor {
                name: "/kompics.benchmarks.BenchmarkRunner/Shutdown".to_string(),
                streaming: ::grpc::rt::GrpcStreaming::Unary,
                req_marshaller: Box::new(::grpc::protobuf::MarshallerProtobuf),
                resp_marshaller: Box::new(::grpc::protobuf::MarshallerProtobuf),
            }),
            method_AtomicBroadcast: ::std::sync::Arc::new(::grpc::rt::MethodDescriptor {
                name: "/kompics.benchmarks.BenchmarkRunner/AtomicBroadcast".to_string(),
                streaming: ::grpc::rt::GrpcStreaming::Unary,
                req_marshaller: Box::new(::grpc::protobuf::MarshallerProtobuf),
                resp_marshaller: Box::new(::grpc::protobuf::MarshallerProtobuf),
            }),
        }
    }
}

impl BenchmarkRunner for BenchmarkRunnerClient {
    fn ready(&self, o: ::grpc::RequestOptions, p: super::messages::ReadyRequest) -> ::grpc::SingleResponse<super::messages::ReadyResponse> {
        self.grpc_client.call_unary(o, p, self.method_Ready.clone())
    }

    fn shutdown(&self, o: ::grpc::RequestOptions, p: super::messages::ShutdownRequest) -> ::grpc::SingleResponse<super::messages::ShutdownAck> {
        self.grpc_client.call_unary(o, p, self.method_Shutdown.clone())
    }

    fn atomic_broadcast(&self, o: ::grpc::RequestOptions, p: super::benchmarks::AtomicBroadcastRequest) -> ::grpc::SingleResponse<super::messages::TestResult> {
        self.grpc_client.call_unary(o, p, self.method_AtomicBroadcast.clone())
    }
}

// server

pub struct BenchmarkRunnerServer;


impl BenchmarkRunnerServer {
    pub fn new_service_def<H : BenchmarkRunner + 'static + Sync + Send + 'static>(handler: H) -> ::grpc::rt::ServerServiceDefinition {
        let handler_arc = ::std::sync::Arc::new(handler);
        ::grpc::rt::ServerServiceDefinition::new("/kompics.benchmarks.BenchmarkRunner",
            vec![
                ::grpc::rt::ServerMethod::new(
                    ::std::sync::Arc::new(::grpc::rt::MethodDescriptor {
                        name: "/kompics.benchmarks.BenchmarkRunner/Ready".to_string(),
                        streaming: ::grpc::rt::GrpcStreaming::Unary,
                        req_marshaller: Box::new(::grpc::protobuf::MarshallerProtobuf),
                        resp_marshaller: Box::new(::grpc::protobuf::MarshallerProtobuf),
                    }),
                    {
                        let handler_copy = handler_arc.clone();
                        ::grpc::rt::MethodHandlerUnary::new(move |o, p| handler_copy.ready(o, p))
                    },
                ),
                ::grpc::rt::ServerMethod::new(
                    ::std::sync::Arc::new(::grpc::rt::MethodDescriptor {
                        name: "/kompics.benchmarks.BenchmarkRunner/Shutdown".to_string(),
                        streaming: ::grpc::rt::GrpcStreaming::Unary,
                        req_marshaller: Box::new(::grpc::protobuf::MarshallerProtobuf),
                        resp_marshaller: Box::new(::grpc::protobuf::MarshallerProtobuf),
                    }),
                    {
                        let handler_copy = handler_arc.clone();
                        ::grpc::rt::MethodHandlerUnary::new(move |o, p| handler_copy.shutdown(o, p))
                    },
                ),
                ::grpc::rt::ServerMethod::new(
                    ::std::sync::Arc::new(::grpc::rt::MethodDescriptor {
                        name: "/kompics.benchmarks.BenchmarkRunner/AtomicBroadcast".to_string(),
                        streaming: ::grpc::rt::GrpcStreaming::Unary,
                        req_marshaller: Box::new(::grpc::protobuf::MarshallerProtobuf),
                        resp_marshaller: Box::new(::grpc::protobuf::MarshallerProtobuf),
                    }),
                    {
                        let handler_copy = handler_arc.clone();
                        ::grpc::rt::MethodHandlerUnary::new(move |o, p| handler_copy.atomic_broadcast(o, p))
                    },
                ),
            ],
        )
    }
}
