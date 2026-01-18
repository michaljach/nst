//! Service implementation for the NST node

use std::sync::Arc;

use nst_runtime::{self, opaque::Block, RuntimeApi};
use sc_client_api::Backend;
use sc_consensus::ImportQueue;
use sc_executor::WasmExecutor;
use sc_service::{error::Error as ServiceError, Configuration, TaskManager, TFullClient};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sc_transaction_pool_api::OffchainTransactionPoolFactory;

/// The full client type
pub type FullClient = TFullClient<Block, RuntimeApi, WasmExecutor<sp_io::SubstrateHostFunctions>>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;

/// Creates a new partial node
pub fn new_partial(
    config: &Configuration,
) -> Result<
    sc_service::PartialComponents<
        FullClient,
        FullBackend,
        FullSelectChain,
        sc_consensus::DefaultImportQueue<Block>,
        sc_transaction_pool::FullPool<Block, FullClient>,
        Option<Telemetry>,
    >,
    ServiceError,
> {
    let telemetry = config
        .telemetry_endpoints
        .clone()
        .filter(|x| !x.is_empty())
        .map(|endpoints| -> Result<_, sc_telemetry::Error> {
            let worker = TelemetryWorker::new(16)?;
            let telemetry = worker.handle().new_telemetry(endpoints);
            Ok((worker, telemetry))
        })
        .transpose()?;

    let executor = sc_service::new_wasm_executor(&config);

    let (client, backend, keystore_container, task_manager) =
        sc_service::new_full_parts::<Block, RuntimeApi, _>(
            config,
            telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
            executor,
        )?;
    let client = Arc::new(client);

    let telemetry = telemetry.map(|(worker, telemetry)| {
        task_manager
            .spawn_handle()
            .spawn("telemetry", None, worker.run());
        telemetry
    });

    let select_chain = sc_consensus::LongestChain::new(backend.clone());

    let transaction_pool = sc_transaction_pool::BasicPool::new_full(
        config.transaction_pool.clone(),
        config.role.is_authority().into(),
        config.prometheus_registry(),
        task_manager.spawn_essential_handle(),
        client.clone(),
    );

    let import_queue = sc_consensus::import_queue::BasicQueue::new(
        sc_consensus::DefaultBlockImporter::new(client.clone()),
        Box::new(client.clone()),
        None,
        &task_manager.spawn_essential_handle(),
        config.prometheus_registry(),
    );

    Ok(sc_service::PartialComponents {
        client,
        backend,
        task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        other: telemetry,
    })
}

/// Build a full node
pub fn new_full(config: Configuration) -> Result<TaskManager, ServiceError> {
    let sc_service::PartialComponents {
        client,
        backend,
        mut task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        other: mut telemetry,
    } = new_partial(&config)?;

    let net_config = sc_network::config::FullNetworkConfiguration::<
        Block,
        <Block as sp_runtime::traits::Block>::Hash,
        sc_network::NetworkWorker<Block, <Block as sp_runtime::traits::Block>::Hash>,
    >::new(&config.network);

    let (network, system_rpc_tx, tx_handler_controller, network_starter, sync_service) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &config,
            net_config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            import_queue,
            block_announce_validator_builder: None,
            warp_sync_params: None,
            block_relay: None,
        })?;

    let rpc_extensions_builder = {
        let client = client.clone();
        let pool = transaction_pool.clone();

        Box::new(move |_deny_unsafe, _| {
            let deps = crate::rpc::FullDeps {
                client: client.clone(),
                pool: pool.clone(),
            };
            crate::rpc::create_full(deps).map_err(Into::into)
        })
    };

    let _rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
        network,
        client: client.clone(),
        keystore: keystore_container.keystore(),
        task_manager: &mut task_manager,
        transaction_pool: transaction_pool.clone(),
        rpc_builder: rpc_extensions_builder,
        backend,
        system_rpc_tx,
        tx_handler_controller,
        sync_service,
        config,
        telemetry: telemetry.as_mut(),
    })?;

    // Start manual seal for dev mode (instant block production)
    if config.role.is_authority() {
        let proposer_factory = sc_basic_authorship::ProposerFactory::new(
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool.clone(),
            config.prometheus_registry(),
            telemetry.as_ref().map(|x| x.handle()),
        );

        // Simple block production - produces a block for each transaction
        let commands_stream = transaction_pool.import_notification_stream().map(|_| {
            sc_consensus_manual_seal::EngineCommand::SealNewBlock {
                create_empty: false,
                finalize: true,
                parent_hash: None,
                sender: None,
            }
        });

        task_manager.spawn_essential_handle().spawn_blocking(
            "manual-seal",
            None,
            sc_consensus_manual_seal::run_manual_seal(sc_consensus_manual_seal::ManualSealParams {
                block_import: client.clone(),
                env: proposer_factory,
                client,
                pool: transaction_pool,
                commands_stream: Box::pin(commands_stream),
                select_chain,
                consensus_data_provider: None,
                create_inherent_data_providers: move |_, ()| async move {
                    Ok(sp_timestamp::InherentDataProvider::from_system_time())
                },
            }),
        );
    }

    network_starter.start_network();
    Ok(task_manager)
}
