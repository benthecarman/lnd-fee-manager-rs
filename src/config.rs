use bitcoin::Network;
use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(version, author, about)]
/// A tool for setting fees for a LND node
pub struct Config {
    /// Interval in seconds to check for new channels and update fees
    #[clap(default_value_t = 60, long)]
    pub interval: u64,
    /// Fee rate in ppm for when our inbound liquidity is low
    #[clap(default_value_t = 10, long)]
    pub low_fee_ppm: u32,
    /// Base fee in msats for when our inbound liquidity is low
    #[clap(default_value_t = 0, long)]
    pub low_fee_base: i64,
    /// Fee rate in ppm for when our inbound liquidity is in the middle
    #[clap(default_value_t = 50, long)]
    pub medium_fee_ppm: u32,
    /// Base fee in msats for when our inbound liquidity is in the middle
    #[clap(default_value_t = 1000, long)]
    pub medium_fee_base: i64,
    /// Fee rate in ppm for when our inbound liquidity is high
    #[clap(default_value_t = 200, long)]
    pub high_fee_ppm: u32,
    /// Base fee in msats for when our inbound liquidity high
    #[clap(default_value_t = 1000, long)]
    pub high_fee_base: i64,
    /// Host of the GRPC server for lnd
    #[clap(default_value_t = String::from("127.0.0.1"), long)]
    pub lnd_host: String,
    /// Port of the GRPC server for lnd
    #[clap(default_value_t = 10009, long)]
    pub lnd_port: u32,
    /// Network lnd is running on ["bitcoin", "testnet", "signet, "regtest"]
    #[clap(default_value_t = Network::Bitcoin, short, long)]
    pub network: Network,
    /// Path to tls.cert file for lnd
    #[clap(long)]
    cert_file: Option<String>,
    /// Path to admin.macaroon file for lnd
    #[clap(long)]
    macaroon_file: Option<String>,
}

impl Config {
    pub fn macaroon_file(&self) -> String {
        self.macaroon_file
            .clone()
            .unwrap_or_else(|| default_macaroon_file(&self.network))
    }

    pub fn cert_file(&self) -> String {
        self.cert_file.clone().unwrap_or_else(default_cert_file)
    }
}

fn home_directory() -> String {
    let buf = home::home_dir().expect("Failed to get home dir");
    let str = format!("{}", buf.display());

    // to be safe remove possible trailing '/' and
    // we can manually add it to paths
    match str.strip_suffix('/') {
        Some(stripped) => stripped.to_string(),
        None => str,
    }
}

pub fn default_cert_file() -> String {
    format!("{}/.lnd/tls.cert", home_directory())
}

pub fn default_macaroon_file(network: &Network) -> String {
    let network_str = match network {
        Network::Bitcoin => "mainnet",
        Network::Testnet => "testnet",
        Network::Signet => "signet",
        Network::Regtest => "regtest",
    };

    format!(
        "{}/.lnd/data/chain/bitcoin/{}/admin.macaroon",
        home_directory(),
        network_str
    )
}
