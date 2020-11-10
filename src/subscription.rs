use crate::{gen_client, Topic};
use crossbeam::channel::Sender;
use jsonrpc_client_transports::RpcError;
use jsonrpc_core::{futures::future, futures::prelude::*, serde_from_str};
use jsonrpc_core_client::{
    transports::duplex::{duplex, Duplex},
    RpcChannel, TypedSubscriptionStream,
};
use jsonrpc_server_utils::{
    codecs::StreamCodec, tokio, tokio::codec::Decoder, tokio::net::TcpStream,
};
use std::net::SocketAddr;
use jsonrpc_server_utils::tokio::executor::{
  Executor , DefaultExecutor
};
use std::collections::HashMap;
use ckb_types::core::{BlockNumber, BlockView};
use ckb_types::packed::ProposalShortId;

#[derive(Default, Debug, Clone)]
struct Metrics {
    table: HashMap<BlockNumber, BlockView>,
    min_number: BlockNumber,
    max_number: BlockNumber,

    total_block_transactions: i64,
    proposals: HashMap<ProposalShortId, BlockNumber>,
}

#[must_use]
pub(crate) fn subscribe(
    ckb_tcp_listened_address: &str,
    block_sender: Sender<ckb_types::core::BlockView>,
) -> Box<dyn Future<Item=(), Error=()>> {
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

    let (duplex, sender_channel): (Duplex<_, _>, RpcChannel) = duplex(sink, stream);
    // Construct rpc client which sends messages(requests) to server, and subscribe `NewTipBlock`
    // from server. We get a typed stream of subscription.
    let requester =  gen_client::Client::from(sender_channel);
    let mut metrics = Metrics::default();
    let subscription_future = requester
            .subscribe(Topic::NewTipBlock)
            .and_then(|subscriber: TypedSubscriptionStream<String>| {
                subscriber
                    .map_err(|err| panic!("{:?}", err))
                    .for_each(move |message| {
                        let block: ckb_jsonrpc_types::BlockView = serde_from_str(&message).expect("invalid json block");
                        let block: ckb_types::core::BlockView = block.into();
                        block_sender.send(block).expect("send block into channel");
                        Ok(())
                    })
            });
    Box::new(duplex.join(subscription_future).map(|_| ()).map_err(|err| panic!("{:?}", err)))
}