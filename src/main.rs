use clap::Parser;
use std::time::Duration;
use tokio::time::sleep;
use tonic_openssl_lnd::lnrpc::channel_point::FundingTxid;
use tonic_openssl_lnd::lnrpc::policy_update_request::Scope;
use tonic_openssl_lnd::lnrpc::{
    ChannelPoint, GetInfoRequest, GetInfoResponse, ListChannelsRequest, PolicyUpdateRequest,
};

mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::try_init()?;

    let config = config::Config::parse();

    let mut lnd_client = tonic_openssl_lnd::connect(
        config.lnd_host.clone(),
        config.lnd_port,
        config.cert_file(),
        config.macaroon_file(),
    )
    .await
    .expect("failed to connect");

    let mut ln_client = lnd_client.lightning().clone();
    let lnd_info: GetInfoResponse = ln_client
        .get_info(GetInfoRequest {})
        .await
        .expect("Failed to get lnd info")
        .into_inner();

    log::info!("Connected to lnd: {}", lnd_info.identity_pubkey);

    loop {
        let channels = ln_client
            .list_channels(ListChannelsRequest::default())
            .await
            .map(|res| res.into_inner().channels)
            .unwrap_or_default();

        for channel in channels {
            let strings = channel.channel_point.split(':').collect::<Vec<_>>();
            let txid = strings[0].to_string();
            let output_index = strings[1].parse::<u32>().unwrap();
            let funding_txid = FundingTxid::FundingTxidStr(txid);
            let scope = Scope::ChanPoint(ChannelPoint {
                output_index,
                funding_txid: Some(funding_txid),
            });

            let percentage = channel.local_balance as f64 / channel.capacity as f64 * 100.0;
            let policy = if percentage > 60_f64 {
                // high balance
                PolicyUpdateRequest {
                    base_fee_msat: config.high_fee_base,
                    fee_rate_ppm: config.high_fee_ppm,
                    scope: Some(scope),
                    ..Default::default()
                }
            } else if percentage > 40_f64 {
                // medium balance
                PolicyUpdateRequest {
                    base_fee_msat: config.medium_fee_base,
                    fee_rate_ppm: config.medium_fee_ppm,
                    scope: Some(scope),
                    ..Default::default()
                }
            } else {
                // low balance
                PolicyUpdateRequest {
                    base_fee_msat: config.low_fee_base,
                    fee_rate_ppm: config.low_fee_ppm,
                    scope: Some(scope),
                    ..Default::default()
                }
            };

            ln_client.update_channel_policy(policy).await?;
        }

        sleep(Duration::from_secs(config.interval)).await;
    }
}
