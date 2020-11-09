use ckb_metrics::metrics;
use ckb_metrics_config::{
    Config as MetricsConfig, Exporter as MetricsExporter, Format as MetricsFormat,
    Target as MetricsTarget,
};
use ckb_metrics_service::init as init_metrics_service;
use ckb_types::core::{BlockNumber, BlockView};
use ckb_types::packed::ProposalShortId;
use crossbeam::channel::Receiver;
use std::collections::HashMap;

pub const SHRINK_CAPACITY: u64 = 1000;

#[derive(Default, Debug, Clone)]
pub struct Exporter {
    table: HashMap<BlockNumber, BlockView>,
    min_number: BlockNumber,
    max_number: BlockNumber,

    total_block_transactions: i64,
    proposals: HashMap<ProposalShortId, BlockNumber>,
}

impl Exporter {
    pub fn listen(listen_address: String, receiver: Receiver<BlockView>) {
        let mut instance = Self::default();
        instance.listen_(listen_address, receiver);
    }

    fn listen_(&mut self, listen_address: String, receiver: Receiver<BlockView>) {
        let config = {
            let prometheus = MetricsExporter {
                target: MetricsTarget::Http { listen_address },
                format: MetricsFormat::Prometheus,
            };
            let mut exporter = HashMap::new();
            exporter.insert("prometheus".to_string(), prometheus);
            MetricsConfig {
                threads: 2,
                histogram_window: 20,
                histogram_granularity: 1,
                upkeep_interval: 500,
                exporter,
            }
        };
        let _guard = init_metrics_service(config).expect("init metrics service");

        while let Ok(block) = receiver.recv() {
            let number = block.number();
            if self.max_number > number {
                self.max_number = number;
            }
            if self.min_number > number {
                self.min_number = number;
            }
            self.total_block_transactions += block.transactions().len() as i64;
            self.table.insert(number, block.clone());

            metrics!(
                gauge,
                "ckb.exporter.block_transactions_total",
                self.total_block_transactions as i64
            );
            metrics!(gauge, "ckb.exporter.tip_number", number as i64);
            for transaction in block.transactions() {
                let proposal_id = transaction.proposal_short_id();
                if let Some(proposed_number) = self.proposals.remove(&proposal_id) {
                    if proposed_number < number {
                        metrics!(
                            counter,
                            "ckb.exporter.2pc_delay_blocks",
                            1,
                            "delay" => (number - proposed_number).to_string(),
                        );
                    }
                }
            }

            for proposal_id in block.union_proposal_ids_iter() {
                // I just want to know the **maximum** delay
                self.proposals.entry(proposal_id).or_insert(number);
            }
            if self.max_number % 100 == 0 {
                self.shrink();
            }
        }
    }

    fn shrink(&mut self) {
        while self.max_number > self.min_number + SHRINK_CAPACITY {
            self.table.remove(&self.min_number);
            self.min_number += 1;
        }
        let min_number = self.min_number;
        self.proposals.retain(|_, number| min_number <= *number);
    }
}
