#[macro_use]
extern crate clap;

use crossbeam::channel::bounded;
use std::thread::spawn;

mod command;
mod exporter;
mod logger;
mod rpc;
mod subscription;

use command::{init_config, Config};
use exporter::Exporter;
use logger::init_logger;
use rpc::{gen_client, Topic};
use subscription::run_subscriber;

fn main() {
    let Config {
        ckb_tcp_listened_address,
        listened_address,
    } = init_config();
    init_logger();

    let (block_sender, block_receiver) = bounded(2000);
    spawn(move || {
        Exporter::listen(listened_address, block_receiver);
    });

    run_subscriber(&ckb_tcp_listened_address, block_sender);
}
