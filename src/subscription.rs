use crate::{gen_client, Topic};
use ckb_metrics::metrics;
use ckb_types::core::{BlockNumber, BlockView};
use ckb_types::packed::ProposalShortId;
use jsonrpc_client_transports::RpcError;
use jsonrpc_core::{futures::prelude::*, serde_from_str};
use jsonrpc_core_client::{
    transports::duplex::{duplex, Duplex},
    RpcChannel, TypedSubscriptionStream,
};
use jsonrpc_server_utils::{codecs::StreamCodec, tokio::codec::Decoder, tokio::net::TcpStream};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::{thread, time};

#[must_use]
pub(crate) fn subscribe(
    ckb_tcp_listened_address: &str,
) -> Box<dyn Future<Item = (), Error = ()> + Send + 'static> {
    // Connect and construct the framed interface
    let addr = ckb_tcp_listened_address.parse::<SocketAddr>().unwrap();

    let raw_io = loop {
        match TcpStream::connect(&addr).wait() {
            Ok(raw_io) => {
                log::info!("[ckb-another-exporter] Tcp stream connected");
                break raw_io;
            }
            _ => {
                log::info!("[ckb-another-exporter] Waiting for tcp stream connection");
                thread::sleep(time::Duration::from_secs(5));
            }
        }
    };

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
    let requester = gen_client::Client::from(sender_channel);
    let mut metrics = Metrics::default();

    let subscription_future = requester.subscribe(Topic::NewTipBlock).and_then(
        |subscriber: TypedSubscriptionStream<String>| {
            subscriber.for_each(move |message| {
                let block: ckb_jsonrpc_types::BlockView = serde_from_str(&message)
                    .map_err(|err| RpcError::ParseError(message, err.into()))?;
                let block: ckb_types::core::BlockView = block.into();
                metrics.new_block(block);
                Ok(())
            })
        },
    );
    Box::new(
        duplex
            .join(subscription_future)
            .map(|_| ())
            .map_err(|err| panic!("{:?}", err)),
    )
}

#[derive(Default, Debug, Clone)]
struct Metrics {
    table: HashMap<BlockNumber, BlockView>,
    min_number: BlockNumber,
    max_number: BlockNumber,

    total_block_transactions: i64,
    proposals: HashMap<ProposalShortId, BlockNumber>,
}

impl Metrics {
    pub fn new_block(&mut self, block: BlockView) {
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

    fn shrink(&mut self) {
        const SHRINK_CAPACITY: u64 = 1000;

        while self.max_number > self.min_number + SHRINK_CAPACITY {
            self.table.remove(&self.min_number);
            self.min_number += 1;
        }
        let min_number = self.min_number;
        self.proposals.retain(|_, number| min_number <= *number);
    }
}
