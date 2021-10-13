use std::convert::TryInto;

use runner::borsh::{self, BorshDeserialize, BorshSerialize};
use runner::{AccountId, InMemorySigner};

const STATUS_MSG_WASM_FILEPATH: &str = "./examples/res/status_message.wasm";

/// This is the cached contract_id from running `deploy_testnet` the first time. Used so we don't
/// overload testnet and have to go through a couple more cycles than we have to, to showcase spooning.
///
/// If you'd like a different account to deploy it to, run the following:
/// ```norun
/// #[runner::main(testnet)]
/// async fn deploy_testnet() {
///     let (contract_id, _) = deploy_status_contract("hello from testnet").await;
///     println!("{}", contract_id);
/// }
/// ```
const TESTNET_PREDEPLOYED_CONTRACT_ID: &str = "dev-20211013002148-59466083160385";

#[derive(Clone, Eq, PartialEq, Debug, BorshDeserialize, BorshSerialize)]
struct Record {
    k: String,
    v: String,
}

#[derive(Clone, Eq, PartialEq, Debug, BorshDeserialize, BorshSerialize)]
struct StatusMessage {
    records: Vec<Record>,
}

async fn deploy_status_contract(msg: &str) -> (AccountId, InMemorySigner) {
    let (contract_id, signer) = runner::dev_deploy(STATUS_MSG_WASM_FILEPATH).await.unwrap();

    runner::call(
        &signer,
        contract_id.clone(),
        contract_id.clone(),
        "set_status".into(),
        serde_json::json!({
            "message": msg,
        })
        .to_string()
        .into_bytes(),
        None,
    )
    .await
    .unwrap();

    (contract_id, signer)
}

#[runner::basic]
async fn main() -> anyhow::Result<()> {
    let (testnet_contract_id, status_msg) = runner::within("testnet", async {
        let contract_id: AccountId = TESTNET_PREDEPLOYED_CONTRACT_ID
            .to_string()
            .try_into()
            .unwrap();

        let mut state_items = runner::view_state(contract_id.clone(), None).await.unwrap();
        let state = state_items.remove("STATE").unwrap();
        let status_msg: StatusMessage =
            StatusMessage::try_from_slice(&state).expect("Expected to retrieve state");

        (contract_id, status_msg)
    })
    .await?;

    runner::within("sandbox", async move {
        let (sandbox_contract_id, _) = deploy_status_contract("hello from sandbox").await;

        let _outcome = runner::patch_state(
            sandbox_contract_id.clone(),
            "STATE".to_string(),
            &status_msg,
        )
        .await
        .unwrap();

        // TODO: here because patch state takes longer than most requests. backoff should help this.
        std::thread::sleep(std::time::Duration::from_secs(5));

        let result = runner::view(
            sandbox_contract_id.clone(),
            "get_status".into(),
            serde_json::json!({
                "account_id": testnet_contract_id.to_string(),
            })
            .to_string()
            .into_bytes()
            .into(),
        )
        .await
        .unwrap();

        let status: String = serde_json::from_value(result).unwrap();
        assert_eq!(status, "hello from testnet".to_string());
    })
    .await
}
