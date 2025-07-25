//! Contains RPC handler implementations specific to state.

use reth_chainspec::{ChainSpecProvider, EthereumHardforks};
use reth_rpc_convert::RpcTypes;
use reth_storage_api::{BlockReader, StateProviderFactory};
use reth_transaction_pool::TransactionPool;

use reth_rpc_eth_api::{
    helpers::{EthState, LoadState, SpawnBlocking},
    EthApiTypes, RpcNodeCoreExt,
};

use crate::EthApi;

impl<Provider, Pool, Network, EvmConfig, Rpc> EthState
    for EthApi<Provider, Pool, Network, EvmConfig, Rpc>
where
    Self: LoadState + SpawnBlocking,
    Provider: BlockReader,
    Rpc: RpcTypes,
{
    fn max_proof_window(&self) -> u64 {
        self.inner.eth_proof_window()
    }
}

impl<Provider, Pool, Network, EvmConfig, Rpc> LoadState
    for EthApi<Provider, Pool, Network, EvmConfig, Rpc>
where
    Self: RpcNodeCoreExt<
            Provider: BlockReader
                          + StateProviderFactory
                          + ChainSpecProvider<ChainSpec: EthereumHardforks>,
            Pool: TransactionPool,
        > + EthApiTypes<NetworkTypes = Rpc>,
    Provider: BlockReader,
    Rpc: RpcTypes,
{
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_consensus::Header;
    use alloy_eips::eip1559::ETHEREUM_BLOCK_GAS_LIMIT_30M;
    use alloy_network::Ethereum;
    use alloy_primitives::{Address, StorageKey, StorageValue, U256};
    use reth_evm_ethereum::EthEvmConfig;
    use reth_network_api::noop::NoopNetwork;
    use reth_provider::test_utils::{ExtendedAccount, MockEthProvider, NoopProvider};
    use reth_rpc_eth_api::helpers::EthState;
    use reth_rpc_eth_types::{
        EthStateCache, FeeHistoryCache, FeeHistoryCacheConfig, GasPriceOracle,
    };
    use reth_rpc_server_types::constants::{
        DEFAULT_ETH_PROOF_WINDOW, DEFAULT_MAX_SIMULATE_BLOCKS, DEFAULT_PROOF_PERMITS,
    };
    use reth_tasks::pool::BlockingTaskPool;
    use reth_transaction_pool::test_utils::{testing_pool, TestPool};
    use std::collections::HashMap;

    fn noop_eth_api() -> EthApi<NoopProvider, TestPool, NoopNetwork, EthEvmConfig, Ethereum> {
        let pool = testing_pool();
        let evm_config = EthEvmConfig::mainnet();

        let cache = EthStateCache::spawn(NoopProvider::default(), Default::default());
        EthApi::new(
            NoopProvider::default(),
            pool,
            NoopNetwork::default(),
            cache.clone(),
            GasPriceOracle::new(NoopProvider::default(), Default::default(), cache),
            ETHEREUM_BLOCK_GAS_LIMIT_30M,
            DEFAULT_MAX_SIMULATE_BLOCKS,
            DEFAULT_ETH_PROOF_WINDOW,
            BlockingTaskPool::build().expect("failed to build tracing pool"),
            FeeHistoryCache::<Header>::new(FeeHistoryCacheConfig::default()),
            evm_config,
            DEFAULT_PROOF_PERMITS,
        )
    }

    fn mock_eth_api(
        accounts: HashMap<Address, ExtendedAccount>,
    ) -> EthApi<MockEthProvider, TestPool, (), EthEvmConfig, Ethereum> {
        let pool = testing_pool();
        let mock_provider = MockEthProvider::default();

        let evm_config = EthEvmConfig::new(mock_provider.chain_spec());
        mock_provider.extend_accounts(accounts);

        let cache = EthStateCache::spawn(mock_provider.clone(), Default::default());
        EthApi::new(
            mock_provider.clone(),
            pool,
            (),
            cache.clone(),
            GasPriceOracle::new(mock_provider, Default::default(), cache),
            ETHEREUM_BLOCK_GAS_LIMIT_30M,
            DEFAULT_MAX_SIMULATE_BLOCKS,
            DEFAULT_ETH_PROOF_WINDOW + 1,
            BlockingTaskPool::build().expect("failed to build tracing pool"),
            FeeHistoryCache::<Header>::new(FeeHistoryCacheConfig::default()),
            evm_config,
            DEFAULT_PROOF_PERMITS,
        )
    }

    #[tokio::test]
    async fn test_storage() {
        // === Noop ===
        let eth_api = noop_eth_api();
        let address = Address::random();
        let storage = eth_api.storage_at(address, U256::ZERO.into(), None).await.unwrap();
        assert_eq!(storage, U256::ZERO.to_be_bytes());

        // === Mock ===
        let storage_value = StorageValue::from(1337);
        let storage_key = StorageKey::random();
        let storage = HashMap::from([(storage_key, storage_value)]);

        let accounts =
            HashMap::from([(address, ExtendedAccount::new(0, U256::ZERO).extend_storage(storage))]);
        let eth_api = mock_eth_api(accounts);

        let storage_key: U256 = storage_key.into();
        let storage = eth_api.storage_at(address, storage_key.into(), None).await.unwrap();
        assert_eq!(storage, storage_value.to_be_bytes());
    }

    #[tokio::test]
    async fn test_get_account_missing() {
        let eth_api = noop_eth_api();
        let address = Address::random();
        let account = eth_api.get_account(address, Default::default()).await.unwrap();
        assert!(account.is_none());
    }
}
