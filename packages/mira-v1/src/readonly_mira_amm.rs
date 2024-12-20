use crate::constants::{DEFAULT_AMM_CONTRACT_ID, READONLY_PRIVATE_KEY};
use crate::interface::{
    AddLiquidityScript, AddLiquidityScriptConfigurables, AmmFees, Asset, LpAssetInfo,
    MiraAmmContract, PoolId, PoolMetadata, RemoveLiquidityScript,
    RemoveLiquidityScriptConfigurables, State, SwapExactInputScript,
    SwapExactInputScriptConfigurables, SwapExactOutputScript, SwapExactOutputScriptConfigurables,
    ADD_LIQUIDITY_SCRIPT_BINARY_PATH, REMOVE_LIQUIDITY_SCRIPT_BINARY_PATH,
    SWAP_EXACT_INPUT_SCRIPT_BINARY_PATH, SWAP_EXACT_OUTPUT_SCRIPT_BINARY_PATH,
};
use crate::utils::{get_asset_id_in, get_lp_asset_id, get_transaction_inputs_outputs};
use fuels::crypto::SecretKey;
use fuels::prelude::{
    AssetId, Bech32ContractId, Execution, Provider, Result, TxPolicies, WalletUnlocked,
};
use fuels::types::transaction_builders::VariableOutputPolicy;
use fuels::types::{ContractId, Identity};
use std::str::FromStr;

pub struct ReadonlyMiraAmm {
    provider: Provider,
    simulation_account: WalletUnlocked,
    amm_contract: MiraAmmContract<WalletUnlocked>,
    add_liquidity_script: AddLiquidityScript<WalletUnlocked>,
    remove_liquidity_script: RemoveLiquidityScript<WalletUnlocked>,
    swap_exact_input_script: SwapExactInputScript<WalletUnlocked>,
    swap_exact_output_script: SwapExactOutputScript<WalletUnlocked>,
}

fn sufficient_tx_policies() -> TxPolicies {
    TxPolicies::default().with_max_fee(1_000_000_000)
}

impl ReadonlyMiraAmm {
    pub fn connect(provider: &Provider, contract_id: Option<ContractId>) -> Result<Self> {
        let readonly_secret_key = SecretKey::from_str(READONLY_PRIVATE_KEY)?;
        let readonly_wallet =
            WalletUnlocked::new_from_private_key(readonly_secret_key, Some(provider.clone()));
        let amm_contract = MiraAmmContract::new(
            contract_id.unwrap_or(ContractId::from_str(DEFAULT_AMM_CONTRACT_ID).unwrap()),
            readonly_wallet.clone(),
        );
        let add_liquidity_script =
            AddLiquidityScript::new(readonly_wallet.clone(), ADD_LIQUIDITY_SCRIPT_BINARY_PATH)
                .with_configurables(
                    AddLiquidityScriptConfigurables::default()
                        .with_AMM_CONTRACT_ID(amm_contract.contract_id().into())
                        .unwrap(),
                );
        let remove_liquidity_script = RemoveLiquidityScript::new(
            readonly_wallet.clone(),
            REMOVE_LIQUIDITY_SCRIPT_BINARY_PATH,
        )
        .with_configurables(
            RemoveLiquidityScriptConfigurables::default()
                .with_AMM_CONTRACT_ID(amm_contract.contract_id().into())
                .unwrap(),
        );
        let swap_exact_input_script =
            SwapExactInputScript::new(readonly_wallet.clone(), SWAP_EXACT_INPUT_SCRIPT_BINARY_PATH)
                .with_configurables(
                    SwapExactInputScriptConfigurables::default()
                        .with_AMM_CONTRACT_ID(amm_contract.contract_id().into())
                        .unwrap(),
                );
        let swap_exact_output_script = SwapExactOutputScript::new(
            readonly_wallet.clone(),
            SWAP_EXACT_OUTPUT_SCRIPT_BINARY_PATH,
        )
        .with_configurables(
            SwapExactOutputScriptConfigurables::default()
                .with_AMM_CONTRACT_ID(amm_contract.contract_id().into())
                .unwrap(),
        );

        Ok(Self {
            provider: provider.clone(),
            simulation_account: readonly_wallet,
            amm_contract,
            add_liquidity_script,
            remove_liquidity_script,
            swap_exact_input_script,
            swap_exact_output_script,
        })
    }

    pub fn id(&self) -> &Bech32ContractId {
        self.amm_contract.contract_id()
    }

    pub async fn pool_metadata(&self, pool_id: PoolId) -> Result<Option<PoolMetadata>> {
        Ok(self
            .amm_contract
            .methods()
            .pool_metadata(pool_id)
            .with_tx_policies(sufficient_tx_policies())
            .simulate(Execution::StateReadOnly)
            .await?
            .value)
    }

    pub async fn fees(&self) -> Result<AmmFees> {
        let (lp_fee_volatile, lp_fee_stable, protocol_fee_volatile, protocol_fee_stable) = self
            .amm_contract
            .methods()
            .fees()
            .with_tx_policies(sufficient_tx_policies())
            .simulate(Execution::StateReadOnly)
            .await?
            .value;
        Ok(AmmFees {
            lp_fee_volatile,
            lp_fee_stable,
            protocol_fee_volatile,
            protocol_fee_stable,
        })
    }

    pub async fn hook(&self) -> Result<Option<ContractId>> {
        Ok(self
            .amm_contract
            .methods()
            .hook()
            .with_tx_policies(sufficient_tx_policies())
            .simulate(Execution::StateReadOnly)
            .await?
            .value)
    }

    pub async fn total_assets(&self) -> Result<u64> {
        Ok(self
            .amm_contract
            .methods()
            .total_assets()
            .with_tx_policies(sufficient_tx_policies())
            .simulate(Execution::StateReadOnly)
            .await?
            .value)
    }

    pub async fn lp_asset_info(&self, asset_id: AssetId) -> Result<Option<LpAssetInfo>> {
        let name = self
            .amm_contract
            .methods()
            .name(asset_id)
            .with_tx_policies(sufficient_tx_policies())
            .simulate(Execution::StateReadOnly)
            .await?
            .value;
        let symbol = self
            .amm_contract
            .methods()
            .symbol(asset_id)
            .with_tx_policies(sufficient_tx_policies())
            .simulate(Execution::StateReadOnly)
            .await?
            .value;
        let decimals = self
            .amm_contract
            .methods()
            .decimals(asset_id)
            .with_tx_policies(sufficient_tx_policies())
            .simulate(Execution::StateReadOnly)
            .await?
            .value;
        let total_supply = self
            .amm_contract
            .methods()
            .total_supply(asset_id)
            .with_tx_policies(sufficient_tx_policies())
            .simulate(Execution::StateReadOnly)
            .await?
            .value;

        match (name, symbol, decimals, total_supply) {
            (Some(name), Some(symbol), Some(decimals), Some(total_supply)) => {
                Ok(Some(LpAssetInfo {
                    asset_id,
                    name,
                    symbol,
                    decimals,
                    total_supply,
                }))
            }
            _ => Ok(None),
        }
    }

    pub async fn owner(&self) -> Result<Option<Identity>> {
        let ownership_state = self
            .amm_contract
            .methods()
            .owner()
            .with_tx_policies(sufficient_tx_policies())
            .simulate(Execution::StateReadOnly)
            .await?
            .value;
        match ownership_state {
            State::Uninitialized => Ok(None),
            State::Initialized(owner) => Ok(Some(owner)),
            State::Revoked => Ok(None),
        }
    }

    pub async fn preview_add_liquidity(
        &self,
        pool_id: PoolId,
        amount_0_desired: u64,
        amount_1_desired: u64,
        amount_0_min: u64,
        amount_1_min: u64,
        deadline: u32,
        tx_policies: Option<TxPolicies>,
    ) -> Result<Asset> {
        let (inputs, outputs) = get_transaction_inputs_outputs(
            &self.simulation_account,
            &vec![(pool_id.0, amount_0_desired), (pool_id.1, amount_1_desired)],
        )
        .await;
        let asset = self
            .add_liquidity_script
            .main(
                pool_id,
                amount_0_desired,
                amount_1_desired,
                amount_0_min,
                amount_1_min,
                self.simulation_account.address().into(),
                deadline,
            )
            .with_tx_policies(tx_policies.unwrap_or_default())
            .with_contracts(&[&self.amm_contract])
            .with_inputs(inputs)
            .with_outputs(outputs)
            .with_variable_output_policy(VariableOutputPolicy::Exactly(1))
            .simulate(Execution::StateReadOnly)
            .await
            .unwrap()
            .value;
        Ok(asset)
    }

    pub async fn preview_remove_liquidity(
        &self,
        pool_id: PoolId,
        liquidity: u64,
        amount_0_min: u64,
        amount_1_min: u64,
        deadline: u32,
        tx_policies: Option<TxPolicies>,
    ) -> Result<(u64, u64)> {
        let lp_asset_id = get_lp_asset_id(self.id().into(), &pool_id);
        let (inputs, outputs) = get_transaction_inputs_outputs(
            &self.simulation_account,
            &vec![(lp_asset_id, liquidity)],
        )
        .await;
        let (asset_0, asset_1) = self
            .remove_liquidity_script
            .main(
                pool_id,
                liquidity,
                amount_0_min,
                amount_1_min,
                self.simulation_account.address().into(),
                deadline,
            )
            .with_tx_policies(tx_policies.unwrap_or_default())
            .with_contracts(&[&self.amm_contract])
            .with_inputs(inputs)
            .with_outputs(outputs)
            .with_variable_output_policy(VariableOutputPolicy::Exactly(1))
            .simulate(Execution::StateReadOnly)
            .await
            .unwrap()
            .value;
        Ok((asset_0, asset_1))
    }

    pub async fn preview_swap_exact_input(
        &self,
        amount_in: u64,
        asset_in: AssetId,
        amount_out_min: u64,
        pools: Vec<PoolId>,
        deadline: u32,
        tx_policies: Option<TxPolicies>,
    ) -> Result<Vec<(u64, AssetId)>> {
        let (inputs, outputs) =
            get_transaction_inputs_outputs(&self.simulation_account, &vec![(asset_in, amount_in)])
                .await;
        let assets = self
            .swap_exact_input_script
            .main(
                amount_in,
                asset_in,
                amount_out_min,
                pools,
                self.simulation_account.address().into(),
                deadline,
            )
            .with_tx_policies(tx_policies.unwrap_or_default())
            .with_contracts(&[&self.amm_contract])
            .with_inputs(inputs)
            .with_outputs(outputs)
            .with_variable_output_policy(VariableOutputPolicy::Exactly(1))
            .simulate(Execution::StateReadOnly)
            .await
            .unwrap()
            .value;
        Ok(assets)
    }

    pub async fn swap_exact_output(
        &self,
        amount_out: u64,
        asset_out: AssetId,
        amount_in_max: u64,
        pools: Vec<PoolId>,
        deadline: u32,
        tx_policies: Option<TxPolicies>,
    ) -> Result<Vec<(u64, AssetId)>> {
        let asset_in = get_asset_id_in(asset_out, &pools);
        let (inputs, outputs) = get_transaction_inputs_outputs(
            &self.simulation_account,
            &vec![(asset_in, amount_in_max)],
        )
        .await;
        let assets = self
            .swap_exact_output_script
            .main(
                amount_out,
                asset_out,
                amount_in_max,
                pools,
                self.simulation_account.address().into(),
                deadline,
            )
            .with_tx_policies(tx_policies.unwrap_or_default())
            .with_contracts(&[&self.amm_contract])
            .with_inputs(inputs)
            .with_outputs(outputs)
            .with_variable_output_policy(VariableOutputPolicy::Exactly(1))
            .simulate(Execution::Realistic)
            .await
            .unwrap()
            .value;
        Ok(assets)
    }
}
