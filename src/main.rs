#[macro_use]
extern crate clap;

mod command;
mod exporter;
mod logger;
mod rpc;
mod subscription;

use command::{init_config, Config};
use exporter::Exporter;
use logger::init_logger;
use rpc::{gen_client, Topic};
use subscription::subscribe;
use jsonrpc_core::{
    futures::future,
    futures::prelude::*,
};
use ckb_metrics_config::{
    Config as MetricsConfig, Exporter as MetricsExporter, Format as MetricsFormat,
    Target as MetricsTarget,
};
use std::collections::HashMap;

fn main() {
    let Config {
        ckb_tcp_listened_address,
        listened_address,
    } = init_config();
    init_logger();
    let _guard = init_metrics_service(listened_address);

    let (block_sender, block_receiver) = crossbeam::channel::bounded(2000);
    ::std::thread::spawn(move || {
        Exporter::default().listen(block_receiver);
    });

    let subscription_future = subscribe(&ckb_tcp_listened_address, block_sender);
}

fn init_metrics_service(listen_address: String) -> ckb_metrics_service::Guard {
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
    ckb_metrics_service::init(config).expect("init metrics service")
}