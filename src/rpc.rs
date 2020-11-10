use jsonrpc_core::Result;
use jsonrpc_derive::rpc;
use jsonrpc_pubsub::{typed::Subscriber, SubscriptionId};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Topic {
    NewTipHeader,
    NewTipBlock,
    NewTransaction,
}

#[allow(clippy::needless_return)]
#[rpc]
pub trait SubscriptionRpc {
    type Metadata;

    #[pubsub(subscription = "subscribe", subscribe, name = "subscribe")]
    fn subscribe(&self, meta: Self::Metadata, subscriber: Subscriber<String>, topic: Topic);

    #[pubsub(subscription = "subscribe", unsubscribe, name = "unsubscribe")]
    fn unsubscribe(&self, meta: Option<Self::Metadata>, id: SubscriptionId) -> Result<bool>;
}
