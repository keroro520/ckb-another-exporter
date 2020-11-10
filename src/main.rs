#[macro_use]
extern crate clap;

mod command;
mod logger;
mod rpc;
mod subscription;

use ckb_metrics_config::{
    Config as MetricsConfig, Exporter as MetricsExporter, Format as MetricsFormat,
    Target as MetricsTarget,
};
use command::{init_config, Config};
use jsonrpc_server_utils::tokio;
use logger::init_logger;
use rpc::{gen_client, Topic};
use std::collections::HashMap;
use subscription::subscribe;

fn main() {
    let Config {
        ckb_tcp_listened_address,
        listened_address,
    } = init_config();
    init_logger();
    let _guard = init_metrics_service(listened_address);

    tokio::run(subscribe(&ckb_tcp_listened_address))
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
