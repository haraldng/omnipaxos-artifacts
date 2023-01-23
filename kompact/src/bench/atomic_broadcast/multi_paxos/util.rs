use crate::bench::atomic_broadcast::multi_paxos::messages::CommandBatchOrNoop;
use kompact::prelude::Port;
use std::cmp::max;

pub type Round = u64;
pub type Slot = u64;

/// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/roundsystem/RoundSystem.scala#L60
pub struct ClassicRoundRobin {
    pub(crate) n: u64,
}

impl ClassicRoundRobin {
    pub(crate) fn leader(&self, round: Round) -> u64 {
        let rest = round % self.n;
        if rest < 1 {
            self.n
        } else {
            rest
        }
    }

    pub(crate) fn next_classic_round(&self, leader_index: u64, round: Round) -> Round {
        let smallest_multiple_of_n = self.n * (round / self.n);
        let offset = leader_index % self.n;
        if (smallest_multiple_of_n + offset) > round {
            smallest_multiple_of_n + offset
        } else {
            smallest_multiple_of_n + self.n + offset
        }
    }
}

/// https://github.com/mwhittaker/frankenpaxos/blob/master/shared/src/main/scala/frankenpaxos/util/BufferMap.scala
pub struct BufferMap {
    grow_size: usize,
    buffer: Vec<Option<CommandBatchOrNoop>>,
    watermark: u64,
    largest_key: Option<u64>,
}

impl BufferMap {
    pub fn with(grow_size: usize) -> Self {
        Self {
            grow_size,
            buffer: Vec::with_capacity(grow_size),
            watermark: 0,
            largest_key: None,
        }
    }

    pub fn normalize(&self, key: u64) -> Option<u64> {
        if key >= self.watermark {
            Some(key - self.watermark)
        } else {
            None
        }
    }

    pub fn pad(&mut self, len: usize) {
        while self.buffer.len() < len {
            self.buffer.push(None);
        }
    }

    pub fn get(&self, key: u64) -> Option<CommandBatchOrNoop> {
        let normalized = self.normalize(key);
        match normalized {
            Some(normalized) if normalized >= self.buffer.len() as u64 => None,
            None => None,
            Some(normalized) => self
                .buffer
                .get(normalized as usize)
                .expect("Failed to get")
                .clone(),
        }
    }

    pub fn put(&mut self, key: u64, value: CommandBatchOrNoop) {
        self.largest_key = Some(max(self.largest_key.unwrap_or_default(), key));
        match self.normalize(key) {
            Some(normalized) => {
                if normalized < self.buffer.len() as u64 {
                    self.buffer[normalized as usize] = Some(value);
                } else {
                    self.pad(normalized as usize + 1 + self.grow_size);
                    self.buffer[normalized as usize] = Some(value);
                }
            }
            None => {}
        }
    }
    /*
    pub fn contains(&self, key: u64) -> bool {
        self.get(key).is_some()
    }
    */
}

#[test]
fn round_robin_test() {
    let mut r = ClassicRoundRobin { n: 3 };
    for round in 0..10 {
        for leader_index in 1..=3 {
            println!(
                "leader_index: {}, round: {}, result: {}",
                leader_index,
                round,
                r.next_classic_round(leader_index, round)
            );
        }
        println!("leader of round {}: {}", round, r.leader(round));
    }
}

pub struct LeaderElectionPort;

impl Port for LeaderElectionPort {
    type Indication = u64; // elected leader pid
    type Request = ();
}
