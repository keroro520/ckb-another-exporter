use crate::{gen_client, Topic};
use crossbeam::channel::Sender;
use jsonrpc_client_transports::RpcError;
use jsonrpc_core::{futures::prelude::*, serde_from_str};
use jsonrpc_core_client::{
    transports::duplex::{duplex, Duplex},
    RpcChannel, TypedSubscriptionStream,
};
use jsonrpc_server_utils::{
    codecs::StreamCodec, tokio, tokio::codec::Decoder, tokio::net::TcpStream,
};
use std::net::SocketAddr;

pub(crate) fn run_subscriber(
    ckb_tcp_listened_address: &str,
    block_sender: Sender<ckb_types::core::BlockView>,
) {
    // Connect and construct the framed interface
    let addr = ckb_tcp_listened_address.parse::<SocketAddr>().unwrap();
    let raw_io = TcpStream::connect(&addr).wait().unwrap();
    let json_codec = StreamCodec::stream_incoming();
    let framed = Decoder::framed(json_codec, raw_io);

    // Framed is a unified of sink and stream. In order to construct the duplex interface, we need
    // to split framed-sink and framed-stream from framed.
    let (sink, stream) = {
        let (sink, stream) = framed.split();

        // Cast the error to pass the compile
        let sink = sink.sink_map_err(|err| RpcError::Other(err.into()));
        let stream = stream.map_err(|err| RpcError::Other(err.into()));

        (sink, stream)
    };

    // Construct rpc client which sends messages(requests) to server
    let requester = {
        let (duplex, sender_channel): (Duplex<_, _>, RpcChannel) = duplex(sink, stream);

        // Spawn the duplex handler to background
        tokio::spawn(duplex.map_err(|_| ()));

        // Construct rpc client which sends messages(requests) to server
        gen_client::Client::from(sender_channel)
    };

    // Subscribe `NewTipBlock` from server. We get a typed stream of subscription.
    let subscriber: TypedSubscriptionStream<String> =
        requester.subscribe(Topic::NewTipBlock).wait().unwrap();

    tokio::run(subscriber.map_err(|_| ()).for_each(move |message| {
        let block: ckb_jsonrpc_types::BlockView = serde_from_str(&message).unwrap();
        let block: ckb_types::core::BlockView = block.into();
        block_sender.send(block).unwrap();
        Ok(())
    }));
}
