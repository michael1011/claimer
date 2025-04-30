use std::process;
use std::str::FromStr;
use std::time::Duration;

use boltz_client::{
    LBtcSwapScript, LBtcSwapTx, PublicKey, Serialize, ToHex, ZKKeyPair,
    bitcoin::secp256k1::{self, SecretKey},
    boltz::{BoltzApiClientV2, Cooperative, CreateReverseResponse, SwapTree},
    fees::Fee,
    network::{LiquidChain, esplora::EsploraLiquidClient},
    util::secrets::Preimage,
};
use clap::{CommandFactory, Parser, Subcommand};

const CLIENT_TIMEOUT_SECS: u64 = 20;
const FEE: Fee = Fee::Relative(0.1);
const LIQUID_NETWORK: LiquidChain = LiquidChain::Liquid;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "https://api.boltz.exchange/v2")]
    boltz_api: String,
    #[arg(short, long, default_value = "https://blockstream.info/liquid/api")]
    esplora_api: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    ClaimReverseSwap {
        #[arg(long)]
        swap_id: String,
        #[arg(long)]
        private_key: String,
        #[arg(long)]
        preimage: String,
        #[arg(long)]
        swap_tree: String,
        #[arg(long)]
        lockup_address: String,
        #[arg(long)]
        refund_public_key: String,
        #[arg(long)]
        address: String,
        #[arg(long)]
        blinding_key: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Args::parse();

    match &cli.command {
        Some(Commands::ClaimReverseSwap {
            swap_id,
            private_key,
            preimage,
            swap_tree,
            lockup_address,
            refund_public_key,
            address,
            blinding_key,
        }) => {
            match claim_reverse_swap(
                cli.boltz_api,
                &cli.esplora_api,
                swap_id.clone(),
                private_key,
                preimage,
                swap_tree,
                refund_public_key.clone(),
                lockup_address.clone(),
                address.clone(),
                blinding_key.clone(),
            )
            .await
            {
                Ok(tx) => println!("{}", tx),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    process::exit(1);
                }
            }
        }
        None => {
            Args::command().print_help().unwrap();
        }
    }

    process::exit(0);
}

async fn claim_reverse_swap(
    boltz_api: String,
    esplora_api: &str,
    swap_id: String,
    private_key: &str,
    preimage: &str,
    swap_tree: &str,
    refund_public_key: String,
    lockup_address: String,
    address: String,
    blinding_key: String,
) -> anyhow::Result<String> {
    let preimage = Preimage::from_vec(hex::decode(preimage)?).map_err(map_error)?;
    let secp = secp256k1::Secp256k1::new();
    let claim_key =
        ZKKeyPair::from_secret_key(&secp, &SecretKey::from_slice(&hex::decode(private_key)?)?);

    let reverse_response = CreateReverseResponse {
        id: swap_id.clone(),
        invoice: "".to_string(),
        swap_tree: serde_json::from_str::<SwapTree>(swap_tree)?,
        lockup_address,
        refund_public_key: PublicKey::from_str(&refund_public_key)?,
        timeout_block_height: 0,
        onchain_amount: 0,
        blinding_key: Some(blinding_key),
    };
    let script =
        LBtcSwapScript::reverse_from_swap_resp(&reverse_response, claim_key.public_key().into())
            .map_err(map_error)?;

    let boltz_client =
        BoltzApiClientV2::new(boltz_api, Some(Duration::from_secs(CLIENT_TIMEOUT_SECS)));
    let liquid_client = EsploraLiquidClient::new(LIQUID_NETWORK, esplora_api, CLIENT_TIMEOUT_SECS);

    let swap_tx = LBtcSwapTx::new_claim(
        script,
        address,
        &liquid_client,
        &boltz_client,
        swap_id.clone(),
    )
    .await
    .map_err(map_error)?;

    let tx = swap_tx
        .sign_claim(
            &claim_key,
            &preimage,
            FEE,
            Some(Cooperative {
                swap_id,
                boltz_api: &boltz_client,
                pub_nonce: None,
                partial_sig: None,
            }),
            true,
        )
        .await
        .map_err(map_error)?;

    Ok(tx.serialize().to_hex())
}

fn map_error(e: boltz_client::error::Error) -> anyhow::Error {
    anyhow::anyhow!("{:?}", e)
}
