use super::*;
use benchmark_suite_shared::benchmark::*;
use std::time::Duration;

pub mod atomic_broadcast;

// pub trait ReceiveRun {
//     fn recipient(&self) -> kompact::prelude::Recipient<&'static messages::Run>;
// }

pub fn component() -> Box<dyn BenchmarkFactory> {
    Box::new(ComponentFactory {})
}
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentFactory;
impl BenchmarkFactory for ComponentFactory {
    fn by_label(&self, _label: &str) -> Result<AbstractBench, NotImplementedError> {
        Err(NotImplementedError::NotFound)
    }

    fn box_clone(&self) -> Box<dyn BenchmarkFactory> {
        Box::new(ComponentFactory {})
    }

    fn atomic_broadcast(
        &self,
    ) -> Result<Box<dyn AbstractDistributedBenchmark>, NotImplementedError> {
        Err(NotImplementedError::NotImplementable)
    }
}

pub fn actor() -> Box<dyn BenchmarkFactory> {
    Box::new(ActorFactory {})
}
#[derive(Clone, Debug, PartialEq)]
pub struct ActorFactory;
impl BenchmarkFactory for ActorFactory {
    fn by_label(&self, _label: &str) -> Result<AbstractBench, NotImplementedError> {
        Err(NotImplementedError::NotFound)
    }

    fn box_clone(&self) -> Box<dyn BenchmarkFactory> {
        Box::new(ActorFactory {})
    }

    fn atomic_broadcast(
        &self,
    ) -> Result<Box<dyn AbstractDistributedBenchmark>, NotImplementedError> {
        Err(NotImplementedError::NotImplementable)
    }
}
pub fn mixed() -> Box<dyn BenchmarkFactory> {
    Box::new(MixedFactory {})
}
#[derive(Clone, Debug, PartialEq)]
pub struct MixedFactory;
impl BenchmarkFactory for MixedFactory {
    fn by_label(&self, label: &str) -> Result<AbstractBench, NotImplementedError> {
        match label {
            atomic_broadcast::benchmark::AtomicBroadcast::LABEL => {
                self.atomic_broadcast().map_into()
            }
            _ => Err(NotImplementedError::NotFound),
        }
    }

    fn box_clone(&self) -> Box<dyn BenchmarkFactory> {
        Box::new(MixedFactory {})
    }

    fn atomic_broadcast(
        &self,
    ) -> Result<Box<dyn AbstractDistributedBenchmark>, NotImplementedError> {
        Ok(atomic_broadcast::benchmark::AtomicBroadcast {}.into())
    }
}
