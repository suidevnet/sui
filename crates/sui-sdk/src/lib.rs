// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;
use std::fmt::Write;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

use anyhow::anyhow;
use futures::StreamExt;
use futures_core::Stream;
use jsonrpsee::core::client::Subscription;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use jsonrpsee::ws_client::{WsClient, WsClientBuilder};
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::Mutex;

// re-export essential sui crates
pub use sui_config::gateway;
use sui_config::gateway::GatewayConfig;
use sui_core::gateway_state::{GatewayClient, GatewayState};
pub use sui_json as json;
use sui_json::SuiJsonValue;
use sui_json_rpc::api::EventStreamingApiClient;
use sui_json_rpc::api::RpcBcsApiClient;
use sui_json_rpc::api::RpcFullNodeReadApiClient;
use sui_json_rpc::api::RpcGatewayApiClient;
use sui_json_rpc::api::RpcReadApiClient;
use sui_json_rpc::api::RpcTransactionBuilderClient;
use sui_json_rpc::api::WalletSyncApiClient;
pub use sui_json_rpc_types as rpc_types;
use sui_json_rpc_types::{
    GatewayTxSeqNumber, GetObjectDataResponse, GetRawObjectDataResponse,
    RPCTransactionRequestParams, SuiEventEnvelope, SuiEventFilter, SuiObjectInfo, SuiObjectRef,
    SuiRawObject, SuiTypeTag, TransactionEffectsResponse, TransactionResponse,
};
pub use sui_types as types;
use sui_types::base_types::{ObjectID, SuiAddress, TransactionDigest};
use sui_types::crypto::{SignableBytes, SuiSignature};
use sui_types::messages::{Transaction, TransactionData};
use sui_types::sui_serde::Base64;

pub mod crypto;

pub struct SuiClient {
    api: Arc<SuiClientApi>,
    state: Arc<Mutex<ClientState>>,
    transaction_builder: TransactionBuilder,
    read_api: ReadApi,
}

#[derive(Default)]
struct ClientState {
    objects: BTreeMap<SuiObjectRef, SuiRawObject>,
    account_objects: BTreeMap<SuiAddress, SuiObjectRef>,
}

impl ClientState {
    fn update_object(&mut self) {}
    fn get_object(&mut self) {}
}

#[allow(clippy::large_enum_variant)]
enum SuiClientApi {
    Http(HttpClient),
    Ws(WsClient),
    Embedded(GatewayClient),
}

impl SuiClient {
    pub fn new_http_client(server_url: &str) -> Result<SuiClient, anyhow::Error> {
        let client = HttpClientBuilder::default().build(server_url)?;
        Ok(SuiClient::new(SuiClientApi::Http(client)))
    }

    pub async fn new_ws_client(server_url: &str) -> Result<SuiClient, anyhow::Error> {
        let client = WsClientBuilder::default().build(server_url).await?;
        Ok(SuiClient::new(SuiClientApi::Ws(client)))
    }

    pub fn new_embedded_client(config: &GatewayConfig) -> Result<SuiClient, anyhow::Error> {
        let state = GatewayState::create_client(config, None)?;
        Ok(SuiClient::new(SuiClientApi::Embedded(state)))
    }
    fn new(api: SuiClientApi) -> Self {
        let api = Arc::new(api);

        let transaction_builder = TransactionBuilder(api.clone());
        let read_api = ReadApi(api.clone());

        SuiClient {
            api: api.clone(),
            state: Arc::new(Mutex::new(ClientState::default())),
            transaction_builder,
            read_api,
        }
    }
}

pub struct TransactionBuilder(Arc<SuiClientApi>);

impl TransactionBuilder {
    pub async fn transfer_object(
        &self,
        signer: SuiAddress,
        object_id: ObjectID,
        gas: Option<ObjectID>,
        gas_budget: u64,
        recipient: SuiAddress,
    ) -> anyhow::Result<TransactionData> {
        Ok(match &*self.0 {
            SuiClientApi::Http(c) => {
                let transaction_bytes = c
                    .transfer_object(signer, object_id, gas, gas_budget, recipient)
                    .await?;
                TransactionData::from_signable_bytes(&transaction_bytes.tx_bytes.to_vec()?)?
            }
            SuiClientApi::Ws(c) => {
                let transaction_bytes = c
                    .transfer_object(signer, object_id, gas, gas_budget, recipient)
                    .await?;
                TransactionData::from_signable_bytes(&transaction_bytes.tx_bytes.to_vec()?)?
            }
            SuiClientApi::Embedded(c) => {
                c.public_transfer_object(signer, object_id, gas, gas_budget, recipient)
                    .await?
            }
        })
    }

    pub async fn transfer_sui(
        &self,
        signer: SuiAddress,
        sui_object_id: ObjectID,
        gas_budget: u64,
        recipient: SuiAddress,
        amount: Option<u64>,
    ) -> anyhow::Result<TransactionData> {
        Ok(match &*self.0 {
            SuiClientApi::Http(c) => {
                let transaction_bytes = c
                    .transfer_sui(signer, sui_object_id, gas_budget, recipient, amount)
                    .await?;
                TransactionData::from_signable_bytes(&transaction_bytes.tx_bytes.to_vec()?)?
            }
            SuiClientApi::Ws(c) => {
                let transaction_bytes = c
                    .transfer_sui(signer, sui_object_id, gas_budget, recipient, amount)
                    .await?;
                TransactionData::from_signable_bytes(&transaction_bytes.tx_bytes.to_vec()?)?
            }
            SuiClientApi::Embedded(c) => {
                c.transfer_sui(signer, sui_object_id, gas_budget, recipient, amount)
                    .await?
            }
        })
    }

    pub async fn move_call(
        &self,
        signer: SuiAddress,
        package_object_id: ObjectID,
        module: String,
        function: String,
        type_arguments: Vec<SuiTypeTag>,
        arguments: Vec<SuiJsonValue>,
        gas: Option<ObjectID>,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        Ok(match &*self.0 {
            SuiClientApi::Http(c) => {
                let transaction_bytes = c
                    .move_call(
                        signer,
                        package_object_id,
                        module,
                        function,
                        type_arguments,
                        arguments,
                        gas,
                        gas_budget,
                    )
                    .await?;
                TransactionData::from_signable_bytes(&transaction_bytes.tx_bytes.to_vec()?)?
            }
            SuiClientApi::Ws(c) => {
                let transaction_bytes = c
                    .move_call(
                        signer,
                        package_object_id,
                        module,
                        function,
                        type_arguments,
                        arguments,
                        gas,
                        gas_budget,
                    )
                    .await?;
                TransactionData::from_signable_bytes(&transaction_bytes.tx_bytes.to_vec()?)?
            }
            SuiClientApi::Embedded(c) => {
                c.move_call(
                    signer,
                    package_object_id,
                    module,
                    function,
                    type_arguments,
                    arguments,
                    gas,
                    gas_budget,
                )
                .await?
            }
        })
    }

    pub async fn publish(
        &self,
        sender: SuiAddress,
        compiled_modules: Vec<Vec<u8>>,
        gas: Option<ObjectID>,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        Ok(match &*self.0 {
            SuiClientApi::Http(c) => {
                let compiled_modules = compiled_modules
                    .iter()
                    .map(|b| Base64::from_bytes(b))
                    .collect();
                let transaction_bytes =
                    c.publish(sender, compiled_modules, gas, gas_budget).await?;
                TransactionData::from_signable_bytes(&transaction_bytes.tx_bytes.to_vec()?)?
            }
            SuiClientApi::Ws(c) => {
                let compiled_modules = compiled_modules
                    .iter()
                    .map(|b| Base64::from_bytes(b))
                    .collect();
                let transaction_bytes =
                    c.publish(sender, compiled_modules, gas, gas_budget).await?;
                TransactionData::from_signable_bytes(&transaction_bytes.tx_bytes.to_vec()?)?
            }
            SuiClientApi::Embedded(c) => {
                c.publish(sender, compiled_modules, gas, gas_budget).await?
            }
        })
    }

    pub async fn split_coin(
        &self,
        signer: SuiAddress,
        coin_object_id: ObjectID,
        split_amounts: Vec<u64>,
        gas: Option<ObjectID>,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        Ok(match &*self.0 {
            SuiClientApi::Http(c) => {
                let transaction_bytes = c
                    .split_coin(signer, coin_object_id, split_amounts, gas, gas_budget)
                    .await?;
                TransactionData::from_signable_bytes(&transaction_bytes.tx_bytes.to_vec()?)?
            }
            SuiClientApi::Ws(c) => {
                let transaction_bytes = c
                    .split_coin(signer, coin_object_id, split_amounts, gas, gas_budget)
                    .await?;
                TransactionData::from_signable_bytes(&transaction_bytes.tx_bytes.to_vec()?)?
            }
            SuiClientApi::Embedded(c) => {
                c.split_coin(signer, coin_object_id, split_amounts, gas, gas_budget)
                    .await?
            }
        })
    }

    pub async fn merge_coins(
        &self,
        signer: SuiAddress,
        primary_coin: ObjectID,
        coin_to_merge: ObjectID,
        gas: Option<ObjectID>,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        Ok(match &*self.0 {
            SuiClientApi::Http(c) => {
                let transaction_bytes = c
                    .merge_coin(signer, primary_coin, coin_to_merge, gas, gas_budget)
                    .await?;
                TransactionData::from_signable_bytes(&transaction_bytes.tx_bytes.to_vec()?)?
            }
            SuiClientApi::Ws(c) => {
                let transaction_bytes = c
                    .merge_coin(signer, primary_coin, coin_to_merge, gas, gas_budget)
                    .await?;
                TransactionData::from_signable_bytes(&transaction_bytes.tx_bytes.to_vec()?)?
            }
            SuiClientApi::Embedded(c) => {
                c.merge_coins(signer, primary_coin, coin_to_merge, gas, gas_budget)
                    .await?
            }
        })
    }

    pub async fn batch_transaction(
        &self,
        signer: SuiAddress,
        single_transaction_params: Vec<RPCTransactionRequestParams>,
        gas: Option<ObjectID>,
        gas_budget: u64,
    ) -> anyhow::Result<TransactionData> {
        Ok(match &*self.0 {
            SuiClientApi::Http(c) => {
                let transaction_bytes = c
                    .batch_transaction(signer, single_transaction_params, gas, gas_budget)
                    .await?;
                TransactionData::from_signable_bytes(&transaction_bytes.tx_bytes.to_vec()?)?
            }

            SuiClientApi::Ws(c) => {
                let transaction_bytes = c
                    .batch_transaction(signer, single_transaction_params, gas, gas_budget)
                    .await?;
                TransactionData::from_signable_bytes(&transaction_bytes.tx_bytes.to_vec()?)?
            }
            SuiClientApi::Embedded(c) => {
                c.batch_transaction(signer, single_transaction_params, gas, gas_budget)
                    .await?
            }
        })
    }
}

pub struct ReadApi(Arc<SuiClientApi>);

impl ReadApi {
    pub async fn get_objects_owned_by_address(
        &self,
        address: SuiAddress,
    ) -> anyhow::Result<Vec<SuiObjectInfo>> {
        Ok(match &*self.0 {
            SuiClientApi::Http(c) => c.get_objects_owned_by_address(address).await?,
            SuiClientApi::Ws(c) => c.get_objects_owned_by_address(address).await?,
            SuiClientApi::Embedded(c) => c.get_objects_owned_by_address(address).await?,
        })
    }

    pub async fn get_objects_owned_by_object(
        &self,
        object_id: ObjectID,
    ) -> anyhow::Result<Vec<SuiObjectInfo>> {
        Ok(match &*self.0 {
            SuiClientApi::Http(c) => c.get_objects_owned_by_object(object_id).await?,
            SuiClientApi::Ws(c) => c.get_objects_owned_by_object(object_id).await?,
            SuiClientApi::Embedded(c) => c.get_objects_owned_by_object(object_id).await?,
        })
    }

    pub async fn get_object(&self, object_id: ObjectID) -> anyhow::Result<GetObjectDataResponse> {
        Ok(match &*self.0 {
            SuiClientApi::Http(c) => c.get_object(object_id).await?,
            SuiClientApi::Ws(c) => c.get_object(object_id).await?,
            SuiClientApi::Embedded(c) => c.get_object(object_id).await?,
        })
    }

    pub async fn get_raw_object(
        &self,
        object_id: ObjectID,
    ) -> anyhow::Result<GetRawObjectDataResponse> {
        Ok(match &*self.0 {
            SuiClientApi::Http(c) => c.get_raw_object(object_id).await?,
            SuiClientApi::Ws(c) => c.get_raw_object(object_id).await?,
            SuiClientApi::Embedded(c) => c.get_raw_object(object_id).await?,
        })
    }

    pub async fn get_total_transaction_number(&self) -> anyhow::Result<u64> {
        Ok(match &*self.0 {
            SuiClientApi::Http(c) => c.get_total_transaction_number().await?,
            SuiClientApi::Ws(c) => c.get_total_transaction_number().await?,
            SuiClientApi::Embedded(c) => c.get_total_transaction_number()?,
        })
    }

    pub async fn get_transactions_in_range(
        &self,
        start: GatewayTxSeqNumber,
        end: GatewayTxSeqNumber,
    ) -> anyhow::Result<Vec<(GatewayTxSeqNumber, TransactionDigest)>> {
        Ok(match &*self.0 {
            SuiClientApi::Http(c) => c.get_transactions_in_range(start, end).await?,
            SuiClientApi::Ws(c) => c.get_transactions_in_range(start, end).await?,
            SuiClientApi::Embedded(c) => c.get_transactions_in_range(start, end)?,
        })
    }

    pub async fn get_recent_transactions(
        &self,
        count: u64,
    ) -> anyhow::Result<Vec<(GatewayTxSeqNumber, TransactionDigest)>> {
        Ok(match &*self.0 {
            SuiClientApi::Http(c) => c.get_recent_transactions(count).await?,
            SuiClientApi::Ws(c) => c.get_recent_transactions(count).await?,
            SuiClientApi::Embedded(c) => c.get_recent_transactions(count)?,
        })
    }

    pub async fn get_transaction(
        &self,
        digest: TransactionDigest,
    ) -> anyhow::Result<TransactionEffectsResponse> {
        Ok(match &*self.0 {
            SuiClientApi::Http(c) => c.get_transaction(digest).await?,
            SuiClientApi::Ws(c) => c.get_transaction(digest).await?,
            SuiClientApi::Embedded(c) => c.get_transaction(digest).await?,
        })
    }
}

impl SuiClient {
    pub fn transaction_builder(&self) -> &TransactionBuilder {
        &self.transaction_builder
    }

    pub fn read_api(&self) -> &ReadApi {
        &self.read_api
    }

    pub async fn get_transactions_by_input_object(
        &self,
        object: ObjectID,
    ) -> anyhow::Result<Vec<(GatewayTxSeqNumber, TransactionDigest)>> {
        Ok(match &*self.api {
            SuiClientApi::Http(c) => c.get_transactions_by_input_object(object).await?,
            SuiClientApi::Ws(c) => c.get_transactions_by_input_object(object).await?,
            SuiClientApi::Embedded(_) => {
                return Err(anyhow!("Method not supported by embedded gateway client."))
            }
        })
    }

    pub async fn get_transactions_by_mutated_object(
        &self,
        object: ObjectID,
    ) -> anyhow::Result<Vec<(GatewayTxSeqNumber, TransactionDigest)>> {
        Ok(match &*self.api {
            SuiClientApi::Http(c) => c.get_transactions_by_mutated_object(object),
            SuiClientApi::Ws(c) => c.get_transactions_by_mutated_object(object),
            SuiClientApi::Embedded(_) => {
                return Err(anyhow!("Method not supported by embedded gateway client."))
            }
        }
        .await?)
    }

    pub async fn get_transactions_by_move_function(
        &self,
        package: ObjectID,
        module: Option<String>,
        function: Option<String>,
    ) -> anyhow::Result<Vec<(GatewayTxSeqNumber, TransactionDigest)>> {
        Ok(match &*self.api {
            SuiClientApi::Http(c) => c.get_transactions_by_move_function(package, module, function),
            SuiClientApi::Ws(c) => c.get_transactions_by_move_function(package, module, function),
            SuiClientApi::Embedded(_) => {
                return Err(anyhow!("Method not supported by embedded gateway client."))
            }
        }
        .await?)
    }

    pub async fn get_transactions_from_addr(
        &self,
        addr: SuiAddress,
    ) -> anyhow::Result<Vec<(GatewayTxSeqNumber, TransactionDigest)>> {
        Ok(match &*self.api {
            SuiClientApi::Http(c) => c.get_transactions_from_addr(addr),
            SuiClientApi::Ws(c) => c.get_transactions_from_addr(addr),
            SuiClientApi::Embedded(_) => {
                return Err(anyhow!("Method not supported by embedded gateway client."))
            }
        }
        .await?)
    }

    pub async fn get_transactions_to_addr(
        &self,
        addr: SuiAddress,
    ) -> anyhow::Result<Vec<(GatewayTxSeqNumber, TransactionDigest)>> {
        Ok(match &*self.api {
            SuiClientApi::Http(c) => c.get_transactions_to_addr(addr),
            SuiClientApi::Ws(c) => c.get_transactions_to_addr(addr),
            SuiClientApi::Embedded(_) => {
                return Err(anyhow!("Method not supported by embedded gateway client."))
            }
        }
        .await?)
    }

    pub async fn execute_transaction(
        &self,
        tx: Transaction,
    ) -> anyhow::Result<TransactionResponse> {
        Ok(match &*self.api {
            SuiClientApi::Http(c) => {
                let tx_bytes = Base64::from_bytes(&tx.data.to_bytes());
                let flag = Base64::from_bytes(&[tx.tx_signature.flag_byte()]);
                let signature = Base64::from_bytes(tx.tx_signature.signature_bytes());
                let pub_key = Base64::from_bytes(tx.tx_signature.public_key_bytes());
                c.execute_transaction(tx_bytes, flag, signature, pub_key)
                    .await?
            }
            SuiClientApi::Ws(c) => {
                let tx_bytes = Base64::from_bytes(&tx.data.to_bytes());
                let flag = Base64::from_bytes(&[tx.tx_signature.flag_byte()]);
                let signature = Base64::from_bytes(tx.tx_signature.signature_bytes());
                let pub_key = Base64::from_bytes(tx.tx_signature.public_key_bytes());
                c.execute_transaction(tx_bytes, flag, signature, pub_key)
                    .await?
            }
            SuiClientApi::Embedded(c) => c.execute_transaction(tx).await?,
        })
    }

    pub async fn sync_account_state(&self, address: SuiAddress) -> anyhow::Result<()> {
        match &*self.api {
            SuiClientApi::Http(c) => c.sync_account_state(address).await?,
            SuiClientApi::Ws(c) => c.sync_account_state(address).await?,
            SuiClientApi::Embedded(c) => c.sync_account_state(address).await?,
        }
        Ok(())
    }

    pub async fn subscribe_event(
        &self,
        filter: SuiEventFilter,
    ) -> anyhow::Result<impl Stream<Item = Result<SuiEventEnvelope, anyhow::Error>>> {
        match &*self.api {
            SuiClientApi::Ws(c) => {
                let subscription: Subscription<SuiEventEnvelope> =
                    c.subscribe_event(filter).await?;
                Ok(subscription.map(|item| Ok(item?)))
            }
            _ => Err(anyhow!("Subscription only supported by WebSocket client.")),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientType {
    Embedded(GatewayConfig),
    RPC(String),
}

impl Display for ClientType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut writer = String::new();

        match self {
            ClientType::Embedded(config) => {
                writeln!(writer, "Client Type : Embedded Gateway")?;
                writeln!(
                    writer,
                    "Gateway state DB folder path : {:?}",
                    config.db_folder_path
                )?;
                let authorities = config
                    .validator_set
                    .iter()
                    .map(|info| info.network_address());
                writeln!(
                    writer,
                    "Authorities : {:?}",
                    authorities.collect::<Vec<_>>()
                )?;
            }
            ClientType::RPC(url) => {
                writeln!(writer, "Client Type : JSON-RPC")?;
                writeln!(writer, "RPC URL : {}", url)?;
            }
        }
        write!(f, "{}", writer)
    }
}

impl ClientType {
    pub async fn init(&self) -> Result<SuiClient, anyhow::Error> {
        Ok(match self {
            ClientType::Embedded(config) => SuiClient::new_embedded_client(config)?,
            ClientType::RPC(url) => {
                if url.starts_with("ws") {
                    SuiClient::new_ws_client(url).await?
                } else {
                    SuiClient::new_http_client(url)?
                }
            }
        })
    }
}
