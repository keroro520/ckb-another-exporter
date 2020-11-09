#[macro_use]
extern crate clap;

use crossbeam::channel::bounded;
use jsonrpc_client_transports::RpcError;
use jsonrpc_core::{serde_from_str, Result, futures::prelude::*};
use jsonrpc_core_client::{
    transports::duplex::{duplex, Duplex},
    RpcChannel, TypedSubscriptionStream,
};
use jsonrpc_derive::rpc;
use jsonrpc_pubsub::{typed::Subscriber, SubscriptionId};
use jsonrpc_server_utils::{
    codecs::StreamCodec, tokio::codec::Decoder, tokio::net::TcpStream, tokio::run,
};
use log::{self, Metadata, Record};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::thread::spawn;

mod exporter;
mod rpc;
mod logger;
mod command;

use exporter::Exporter;
use rpc::{Topic, gen_client};
use logger::init_logger;
use command::init_config;

fn main() {
    init_logger();
    let config = init_config();

    // 18114 is tcp_listen_port; 8114 is http_listen_port
    let tcp_stream = {
        let addr = config.ckb_tcp_listened_address.parse::<SocketAddr>().unwrap();
        TcpStream::connect(&addr).wait().unwrap()
    };

    // framed_layer is both sink and stream. use `split` to break them out.
    let framed_layer = StreamCodec::stream_incoming().framed(tcp_stream);
    let (sink, stream) = framed_layer.split();

    // Plain `sink`/`stream` doesn't satisfy
    // `<_ as futures::sink::Sink>::SinkError = jsonrpc_client_transports::RpcError`,
    // then Duplex<SINK, STREAM> doesn't satisfy Future/Stream. Therefore we map_err.
    let sink = sink.sink_map_err(|err| RpcError::Other(err.into()));
    let stream = stream.map_err(|err| RpcError::Other(err.into()));

    let (duplex, sender_channel) = duplex(sink, stream);
    let duplex: Duplex<_, _> = duplex;
    let sender_channel: RpcChannel = sender_channel;
    let sender_client = gen_client::Client::from(sender_channel);

    // background handler
    ::std::thread::spawn(move || {
        run(duplex.map_err(|err| {
            log::error!("[ckb-another-exporter] duplex error {:?}", err);
        }));
    });

    // exporter
    let (block_sender, block_receiver) = bounded(2000);
    spawn(move || {
        Exporter::listen(config.listened_address.clone(), block_receiver);
    });

    // subscriber
    let subscriber: TypedSubscriptionStream<String> = sender_client
        .subscribe(Topic::NewTipBlock)
        .wait()
        .expect("subscribe failed");
    run(subscriber
        .map_err(|err| {
            log::error!("[ckb-another-exporter] subscriber error {:?}", err);
        })
        .for_each(move |message| {
            let block: ckb_jsonrpc_types::BlockView =
                serde_from_str(&message).expect("subscribe invalid message");
            let block: ckb_types::core::BlockView = block.into();
            block_sender.send(block).expect("send block");
            Ok(())
        }));
}
