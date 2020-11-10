use std::net::SocketAddr;

pub(crate) struct Config {
    pub(crate) ckb_tcp_listened_address: String,
    pub(crate) listened_address: String,
}

pub(crate) fn init_config() -> Config {
    let matches = clap::app_from_crate!()
        .arg(
            clap::Arg::from_usage(
                "--ckb_tcp_listen_address <ENDPOINT> 'the ckb tcp_listen_address'",
            )
            .default_value("0.0.0.0:18114")
            .validator(|str| {
                str.parse::<SocketAddr>()
                    .map(|_| ())
                    .map_err(|err| err.to_string())
            }),
        )
        .arg(
            clap::Arg::from_usage(
                "--listened_address <ENDPOINT> 'the listened address to expose metrics'",
            )
            .default_value("0.0.0.0:8200")
            .validator(|str| {
                str.parse::<SocketAddr>()
                    .map(|_| ())
                    .map_err(|err| err.to_string())
            }),
        )
        .get_matches();
    let ckb_tcp_listened_address = matches
        .value_of("ckb_tcp_listen_address")
        .unwrap()
        .to_string();
    let listened_address = matches.value_of("listened_address").unwrap().to_string();
    Config {
        ckb_tcp_listened_address,
        listened_address,
    }
}
