use crate::config::Config;
use clap::Parser;
use std::time::Duration;
use tokio::time::sleep;
use tonic_openssl_lnd::lnrpc::channel_point::FundingTxid;
use tonic_openssl_lnd::lnrpc::policy_update_request::Scope;
use tonic_openssl_lnd::lnrpc::{
    ChanInfoRequest, ChannelPoint, GetInfoRequest, GetInfoResponse, ListChannelsRequest,
    PolicyUpdateRequest,
};
use tonic_openssl_lnd::LndLightningClient;

mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::try_init()?;

    let config = Config::parse();

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
            if let Err(e) =
                handle_channel(&mut ln_client, &config, &lnd_info.identity_pubkey, channel).await
            {
                log::error!("Failed to handle channel: {e}");
            }
        }

        sleep(Duration::from_secs(config.interval)).await;
    }
}

async fn handle_channel(
    ln_client: &mut LndLightningClient,
    config: &Config,
    node_id: &str,
    channel: tonic_openssl_lnd::lnrpc::Channel,
) -> anyhow::Result<()> {
    let strings = channel.channel_point.split(':').collect::<Vec<_>>();
    let txid = strings[0].to_string();
    let output_index = strings[1].parse::<u32>().unwrap();
    let funding_txid = FundingTxid::FundingTxidStr(txid);
    let scope = Scope::ChanPoint(ChannelPoint {
        output_index,
        funding_txid: Some(funding_txid),
    });

    let chan = ln_client
        .get_chan_info(ChanInfoRequest {
            chan_id: channel.chan_id,
        })
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get channel info ({}): {}", channel.chan_id, e))?
        .into_inner();

    let current = if chan.node1_pub == node_id {
        chan.node1_policy
            .ok_or(anyhow::anyhow!("No node1 policy"))?
    } else {
        chan.node2_policy
            .ok_or(anyhow::anyhow!("No node2 policy"))?
    };

    let percentage = channel.local_balance as f64 / channel.capacity as f64 * 100.0;
    let policy = if percentage > 60_f64 {
        // high balance
        PolicyUpdateRequest {
            base_fee_msat: config.high_fee_base,
            fee_rate_ppm: config.high_fee_ppm,
            time_lock_delta: current.time_lock_delta,
            max_htlc_msat: current.max_htlc_msat,
            scope: Some(scope),
            ..Default::default()
        }
    } else if percentage > 40_f64 {
        // medium balance
        PolicyUpdateRequest {
            base_fee_msat: config.medium_fee_base,
            fee_rate_ppm: config.medium_fee_ppm,
            time_lock_delta: current.time_lock_delta,
            max_htlc_msat: current.max_htlc_msat,
            scope: Some(scope),
            ..Default::default()
        }
    } else {
        // low balance
        PolicyUpdateRequest {
            base_fee_msat: config.low_fee_base,
            fee_rate_ppm: config.low_fee_ppm,
            time_lock_delta: current.time_lock_delta,
            max_htlc_msat: current.max_htlc_msat,
            scope: Some(scope),
            ..Default::default()
        }
    };

    if current.fee_base_msat != policy.base_fee_msat
        || current.fee_rate_milli_msat != policy.fee_rate_ppm.into()
    {
        log::info!(
            "Updating channel policy for channel {} to base_fee_msat: {}, fee_rate_ppm: {}",
            channel.chan_id,
            policy.base_fee_msat,
            policy.fee_rate_ppm
        );
        ln_client.update_channel_policy(policy).await?;
    }

    Ok(())
}
