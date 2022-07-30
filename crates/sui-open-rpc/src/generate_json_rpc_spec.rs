// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::fs::File;
use std::io::Write;

use clap::ArgEnum;
use clap::Parser;
use pretty_assertions::assert_str_eq;

use crate::examples::RpcExampleProvider;
use sui_config::genesis_config::GenesisConfig;
use sui_json_rpc::api::EventStreamingApiOpenRpc;
use sui_json_rpc::bcs_api::BcsApiImpl;
use sui_json_rpc::gateway_api::{GatewayWalletSyncApiImpl, RpcGatewayImpl, TransactionBuilderImpl};
use sui_json_rpc::read_api::{FullNodeApi, ReadApi};
use sui_json_rpc::sui_rpc_doc;
use sui_json_rpc::SuiRpcModule;
use sui_test_data::create_test_data;
use test_utils::network::start_rpc_test_network;

mod examples;

#[derive(Debug, Parser, Clone, Copy, ArgEnum)]
enum Action {
    Print,
    Test,
    Record,
}

#[derive(Debug, Parser)]
#[clap(
    name = "Sui format generator",
    about = "Trace serde (de)serialization to generate format descriptions for Sui types"
)]
struct Options {
    #[clap(arg_enum, default_value = "Record", ignore_case = true)]
    action: Action,
}

const FILE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/spec/openrpc.json",);

const OBJECT_SAMPLE_FILE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/samples/objects.json",);

const TRANSACTION_SAMPLE_FILE_PATH: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/samples/transactions.json",);

const OWNED_OBJECT_SAMPLE_FILE_PATH: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/samples/owned_objects.json",);

#[tokio::main]
async fn main() {
    let options = Options::parse();

    let mut open_rpc = sui_rpc_doc();
    open_rpc.add_module(TransactionBuilderImpl::rpc_doc_module());
    open_rpc.add_module(RpcGatewayImpl::rpc_doc_module());
    open_rpc.add_module(ReadApi::rpc_doc_module());
    open_rpc.add_module(FullNodeApi::rpc_doc_module());
    open_rpc.add_module(BcsApiImpl::rpc_doc_module());
    open_rpc.add_module(EventStreamingApiOpenRpc::module_doc());
    // TODO: Re-enable this when event read API is ready
    //open_rpc.add_module(EventReadApiOpenRpc::module_doc());
    open_rpc.add_module(GatewayWalletSyncApiImpl::rpc_doc_module());

    open_rpc.add_examples(RpcExampleProvider::new().examples());

    match options.action {
        Action::Print => {
            let content = serde_json::to_string_pretty(&open_rpc).unwrap();
            println!("{content}");
            let network = start_rpc_test_network(Some(GenesisConfig::custom_genesis(1, 4, 30)))
                .await
                .unwrap();
            let (objects, txs, addresses) = create_test_data(&network).await.unwrap();
            println!("{}", serde_json::to_string_pretty(&objects).unwrap());
            println!("{}", serde_json::to_string_pretty(&txs).unwrap());
            println!("{}", serde_json::to_string_pretty(&addresses).unwrap());
        }
        Action::Record => {
            let content = serde_json::to_string_pretty(&open_rpc).unwrap();
            let mut f = File::create(FILE_PATH).unwrap();
            writeln!(f, "{content}").unwrap();
            let network = start_rpc_test_network(Some(GenesisConfig::custom_genesis(1, 4, 30)))
                .await
                .unwrap();
            let (objects, txs, addresses) = create_test_data(&network).await.unwrap();
            let content = serde_json::to_string_pretty(&objects).unwrap();
            let mut f = File::create(OBJECT_SAMPLE_FILE_PATH).unwrap();
            writeln!(f, "{content}").unwrap();
            let content = serde_json::to_string_pretty(&txs).unwrap();
            let mut f = File::create(TRANSACTION_SAMPLE_FILE_PATH).unwrap();
            writeln!(f, "{content}").unwrap();
            let content = serde_json::to_string_pretty(&addresses).unwrap();
            let mut f = File::create(OWNED_OBJECT_SAMPLE_FILE_PATH).unwrap();
            writeln!(f, "{content}").unwrap();
        }
        Action::Test => {
            let reference = std::fs::read_to_string(FILE_PATH).unwrap();
            let content = serde_json::to_string_pretty(&open_rpc).unwrap() + "\n";
            assert_str_eq!(&reference, &content);
        }
    }
}
