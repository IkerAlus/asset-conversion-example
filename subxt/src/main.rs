use codec::Encode;
use subxt::{
    OnlineClient, 
    config::{
        Config, PolkadotConfig, SubstrateConfig,
        }, 
        utils::{
            AccountId32, MultiAddress
        }
    };
use subxt_signer::sr25519::dev::{self};
use subxt::config::extrinsic_params::{BaseExtrinsicParamsBuilder, BaseExtrinsicParams};

// Metadata that we'll use for our example
#[subxt::subxt(runtime_metadata_path = "./metadata/asset_hub_metadata.scale")]
pub mod local {}

// Types that we retrieve from the Metadata for our example
type MultiLocation = local::runtime_types::staging_xcm::v3::multilocation::MultiLocation;
use local::runtime_types::staging_xcm::v3::junction::Junction::{GeneralIndex, PalletInstance};
use local::runtime_types::staging_xcm::v3::junctions::Junctions::{Here, X2};
type Call = local::runtime_types::asset_hub_westend_runtime::RuntimeCall;
type AssetConversionCall = local::runtime_types::pallet_asset_conversion::pallet::Call;
type AssetsCall = local::runtime_types::pallet_assets::pallet::Call;

// Asset details
const ASSET_ID: u32 = 1;
const NAME: &str = "Asset1";
const SYMBOL: &str = "A1";
const URI: &str = "ws://127.0.0.1:9944";

// This is our custom configuration for the signed extensions.
// We don't need to construct this at runtime,
// so an empty enum is appropriate:
pub enum CustomConfig {}

impl Config for CustomConfig {
    type Hash = <SubstrateConfig as Config>::Hash;
    type AccountId = <SubstrateConfig as Config>::AccountId;
    type Address = <PolkadotConfig as Config>::Address;
    type Signature = <SubstrateConfig as Config>::Signature;
    type Hasher = <SubstrateConfig as Config>::Hasher;
    type Header = <SubstrateConfig as Config>::Header;
    type ExtrinsicParams = WestmintExtrinsicParams<Self>;
}

/// A struct representing the signed extra and additional parameters required
/// to construct a transaction for the default substrate node.
pub type WestmintExtrinsicParams<T> = BaseExtrinsicParams<T, AssetTip>;

/// A builder which leads to [`SubstrateExtrinsicParams`] being constructed.
/// This is what you provide to methods like `sign_and_submit()`.
pub type WestmintExtrinsicParamsBuilder<T> = BaseExtrinsicParamsBuilder<T, AssetTip>;

/// A tip payment made in the form of a specific asset.
#[derive(Debug, Default, Encode)]
pub struct AssetTip {
    #[codec(compact)]
    tip: u128,
    asset: Option<MultiLocation>,
}

impl AssetTip {
    /// Create a new tip of the amount provided.
    pub fn new(amount: u128) -> Self {
        AssetTip {
            tip: amount,
            asset: None,
        }
    }

    /// Designate the tip as being of a particular asset class.
    /// If this is not set, then the native currency is used.
    pub fn of_asset(mut self, asset: MultiLocation) -> Self {
        self.asset = Some(asset);
        self
    }
}

impl From<u128> for AssetTip {
    fn from(n: u128) -> Self {
        AssetTip::new(n)
    }
}

// `pallet-assets` create_asset call
fn create_asset_call(
    admin: MultiAddress<AccountId32, ()>,
    min_balance: u128,
) -> Result<Call, Box<dyn std::error::Error>> {
    let call = Call::Assets(AssetsCall::create {
        id: ASSET_ID,
        admin: admin,
        min_balance: min_balance,
    });

    Ok(call)
}

// `pallet-assets` set_metadata call
fn set_asset_metadata_call(
    asset_id: u32,
    name: Vec<u8>,
    symbol: Vec<u8>,
    decimals: u8,
) -> Result<Call, Box<dyn std::error::Error>> {
    let call = Call::Assets(AssetsCall::set_metadata {
        id: asset_id,
        name: name,
        symbol: symbol,
        decimals: decimals,
    });

    Ok(call)
}

// `pallet-assets` create_mint call
fn mint_token_call(
    beneficiary: MultiAddress<AccountId32, ()>,
    amount: u128,
) -> Result<Call, Box<dyn std::error::Error>> {
    let call = Call::Assets(AssetsCall::mint {
        id: ASSET_ID,
        beneficiary: beneficiary,
        amount: amount,
    });

    Ok(call)
}

// We will use this to create the liquidity pool with a Native asset and our Custom asset
fn create_pool_with_native_call() -> Result<Call, Box<dyn std::error::Error>> {
    let call = Call::AssetConversion(AssetConversionCall::create_pool {
        asset1: MultiLocation {
            parents: 1,
            interior: Here,
        },
        asset2: MultiLocation {
            parents: 0,
            interior: X2(PalletInstance(50), GeneralIndex(ASSET_ID.into())),
        },
    });

    Ok(call)
}

// We will use this to add liquidity to our liquidity pool
fn provide_liquidity_to_token_native_pool_call(
    amount1_desired: u128,
    amount2_desired: u128,
    amount1_min: u128,
    amount2_min: u128,
    mint_to: AccountId32,
) -> Result<Call, Box<dyn std::error::Error>> {
    let call = Call::AssetConversion(AssetConversionCall::add_liquidity {
        // Native Asset MultiLocation
        asset1: MultiLocation {
            parents: 1,
            interior: Here,
        },
        // Our Custom Asset MultiLocation
        // PalletInstance(50) refers to the pallet-assets in Asset Hub Westend 
        asset2: MultiLocation {
            parents: 0,
            interior: X2(PalletInstance(50), GeneralIndex(ASSET_ID.into())),
        },
        amount1_desired: amount1_desired,
        amount2_desired: amount2_desired,
        amount1_min: amount1_min,
        amount2_min: amount2_min,
        mint_to: mint_to.into(),
    });

    Ok(call)
}

// We use this to sign and send the calls that we defined earlier as a single 
// batch and wait until it's successful
async fn sign_and_send_batch_calls(
    api: OnlineClient<CustomConfig>,
    calls: Vec<Call>,
) -> Result<(), subxt::Error> {
    let alice_pair_signer = dev::alice();
    let tx = local::tx().utility().batch_all(calls);

    let tx_params = WestmintExtrinsicParamsBuilder::new();
    api.tx()
        .sign_and_submit_then_watch(&tx, &alice_pair_signer, tx_params)
        .await?
        .wait_for_in_block()
        .await?
        .wait_for_success()
        .await?;

    Ok(())
}

// Here we simulate the native asset transfer to estimate the fees using
// `TransactionPaymentApi_query_info`
async fn estimate_fees(
    api: OnlineClient<CustomConfig>,
    dest: MultiAddress<AccountId32, ()>,
    amount: u128,
    ) -> Result<u128, Box<dyn std::error::Error>> {
    let alice = dev::alice();

    let balance_transfer_tx = local::tx().balances().transfer_keep_alive(dest, amount);
    
    let signed = api.tx().create_signed(&balance_transfer_tx, &alice, Default::default()).await.unwrap();
    
    let partial_fee: u128 = signed.partial_fee_estimate().await.unwrap();
    
    println!("The estimated fee is: {partial_fee} Plancks");

    Ok(partial_fee)
}

// With this fn we use the AssetConversionApi.quote_price_exact_tokens_for_tokens
// to convert the estimated fees from the Native asset to our Custom asset.
async fn convert_fees(
    api: OnlineClient<CustomConfig>,
    amount: u128,
) -> Result<(), Box<dyn std::error::Error>> {
    let native = MultiLocation {
        parents: 1,
        interior: Here,
    };
    let asset = MultiLocation {
        parents: 0,
        interior: X2(PalletInstance(50), GeneralIndex(ASSET_ID.into())),   
    };
    let amount = amount;
    let include_fee = true;

    let runtime_apis = local::apis().asset_conversion_api().quote_price_exact_tokens_for_tokens(
        native,
        asset,
        amount,
        include_fee
    );

    let converted_fee = api.runtime_api().at_latest().await.unwrap().call(runtime_apis).await.unwrap();

    println!("The estimated fee in the custom asset is: {:#}", converted_fee.unwrap());

    Ok(())
}

// Here we make a Native asset transfer while paying the tx fees with our custom
// asset, using the `AssetConversionTxPayment` signed extension that we configured
// as `ChargeAssetTxPayment`
async fn sign_and_send_transfer(
    api: OnlineClient<CustomConfig>,
    dest: MultiAddress<AccountId32, ()>,
    amount: u128,
    multi: MultiLocation,
) -> Result<(), subxt::Error> {
    let alice_pair_signer = dev::alice();
    let balance_transfer_tx = local::tx().balances().transfer_keep_alive(dest, amount);
    
    let tx_params = WestmintExtrinsicParamsBuilder::new().tip(AssetTip::new(0).of_asset(multi));
    
    // Here we send the Native asset transfer and wait for it to be finalized, while
    // listening for the `AssetTxFeePaid` event that confirms we succesfully paid
    // the fees with our custom asset
    api
    .tx()
    .sign_and_submit_then_watch(&balance_transfer_tx, &alice_pair_signer, tx_params)
    .await?
    .wait_for_finalized_success()
    .await?
    .has::<local::asset_conversion_tx_payment::events::AssetTxFeePaid>()?;
    
    println!("Balance transfer submitted and fee paid succesfully");
    Ok(())
}

// We use this to setup the stage for our transfer, using the calls defined earlier
// to create our custom asset, set it's metadata, mint it, create the liquidity pool
// and provide liquidity to it. We send the calls as a batch for simplicity.
async fn prepare_setup(api: OnlineClient<CustomConfig>) {
    let alice: MultiAddress<AccountId32, ()> = dev::alice().public_key().into();
    let address: AccountId32 = dev::alice().public_key().into();

    let mut call_buffer: Vec<Call> = Vec::<Call>::new();
    call_buffer.push(create_asset_call(alice.clone(), 1).unwrap());

    call_buffer.push(
        set_asset_metadata_call(
            ASSET_ID,
            NAME.as_bytes().to_vec(),
            SYMBOL.as_bytes().to_vec(),
            0,
        )
        .unwrap(),
    );

    const AMOUNT_TO_MINT: u128 = 100000000000000;

    call_buffer.push(mint_token_call( alice.clone(), AMOUNT_TO_MINT).unwrap());
 
    call_buffer.push(create_pool_with_native_call().unwrap());

    call_buffer.push(
        provide_liquidity_to_token_native_pool_call(
            10000000000,
            10000000,
            0,
            0,
            address,
        )
        .unwrap(),
    );

    if let Err(subxt::Error::Runtime(dispatch_err)) =
        sign_and_send_batch_calls(api, call_buffer).await
    {
        eprintln!("Could not dispatch the call: {}", dispatch_err);
    }
}

#[tokio::main]
async fn main() {
    // Establish the uri of the local asset hub westend node to which we are 
    // connecting to and instantiate the api
    let api = OnlineClient::<CustomConfig>::from_url(URI).await.unwrap();

    // Setup the stage
    let _setup = prepare_setup(api.clone()).await;

    // Give it a little time for the tx to be included in the blocks
    std::thread::sleep(std::time::Duration::from_secs(2));

    let dest: MultiAddress<AccountId32, ()> = dev::bob().public_key().into();

    // Here we estimate the tx fees
    let fee = estimate_fees(api.clone(), dest.clone(), 100000).await.unwrap().try_into();

    let _converted_fee = convert_fees(api.clone(), fee.unwrap()).await;

    // Here we create and submit the native asset transfer passing the custom 
    // asset's MultiLocation to pay the fees
    let _result = sign_and_send_transfer(api.clone(), dest, 100000, MultiLocation {
        parents: 0,
        interior: X2(PalletInstance(50), GeneralIndex(ASSET_ID.into())),
    }).await;
}
