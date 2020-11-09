# ckb-another-exporter

Some chain-related metrics are difficult to expose via ckb node itself. Luckily, we can extract these chain-related metrics via analyzing corresponding blocks, which can be retrived via rpc `get_block`. In practice, we retrive the blocks via rpc `subscribe("new_block")`, which is more efficent than `get_block`.

To use ckb-another-exporter, you should:
    - enable rpc module "Subscription"
    - enable `tcp_listen_address`
