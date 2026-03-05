mod flow_manager;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rust_decimal::Decimal;
use tokio::sync::{broadcast, watch};
use tracing_subscriber::EnvFilter;

use talon_broker::ibkr::IbkrBroker;
use talon_broker::session::BrokerSessionManager;
use talon_broker::traits::{BrokerCommands, BrokerStreams};
use talon_exec::exec_core::ExecCore;
use talon_types::strategy::TradingModule;
use talon_risk::mesh::RiskMesh;
use talon_risk::stress::StressEngine;
use talon_db::store::EventStore;
use talon_types::broker::{BrokerId, QuoteEvent};
use talon_types::channel::{ChannelBus, ModuleStateEntry};
use talon_types::module::ModuleId;
use talon_types::position::Symbol;
use talon_types::risk::{ModuleRiskAllocation, StressParams, TierRiskParams};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // --- Load .env (Polygon API key, etc.) ---
    let _ = dotenvy::dotenv();

    // --- Data directory (needed by log + event store) ---
    let data_dir = PathBuf::from("data");
    std::fs::create_dir_all(&data_dir)?;

    // --- Logging (to file — stdout is owned by the TUI) ---
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(data_dir.join("talon.log"))?;
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(log_file)
        .with_ansi(false)
        .init();

    tracing::info!("TALON — Trade Autonomously with Limited Override Necessity");
    tracing::info!("Phase 0: Hatch tier, IBKR Gateway on 4002");

    // --- Event Store ---
    let event_store = EventStore::open(&data_dir.join("events.db"))?;

    // --- Channel Bus ---
    let bus = ChannelBus::new();
    let ChannelBus {
        intent_tx,
        intent_rx,
        fill_tx,
        quote_tx,
        event_tx: _event_tx,
        event_rx: _event_rx,
        app_state_tx,
        app_state_rx,
        flow_cmd_tx,
        flow_cmd_rx,
        supervision_tx,
        supervision_rx,
        chart_cmd_tx,
        chart_cmd_rx,
    } = bus;

    // --- IBKR Broker ---
    let ibkr = IbkrBroker::connect("127.0.0.1", 4002, 1).await?;
    let mut session_manager = BrokerSessionManager::new();
    session_manager.register(
        BrokerId::Ibkr,
        Arc::clone(&ibkr) as Arc<dyn talon_broker::traits::BrokerCommands>,
        Arc::clone(&ibkr) as Arc<dyn talon_broker::traits::BrokerStreams>,
    );

    // --- Watchlist (from config/talon.toml) ---
    let watchlist = load_watchlist("config/talon.toml")?;
    tracing::info!(count = watchlist.len(), "Watchlist loaded from config");

    // --- IBKR Quote Stream (mpsc -> broadcast bridge) ---
    let (quote_mpsc_tx, mut quote_mpsc_rx) =
        tokio::sync::mpsc::channel::<QuoteEvent>(4096);
    let _quote_handle = ibkr
        .subscribe_quotes(&watchlist, quote_mpsc_tx)
        .await?;

    let quote_tx_bridge = quote_tx.clone();
    tokio::spawn(async move {
        while let Some(q) = quote_mpsc_rx.recv().await {
            let _ = quote_tx_bridge.send(q);
        }
    });

    // --- IBKR Fill Stream (mpsc -> broadcast bridge) ---
    let (fill_mpsc_tx, mut fill_mpsc_rx) =
        tokio::sync::mpsc::channel(1024);
    let _fill_handle = ibkr.subscribe_fills(fill_mpsc_tx).await?;

    let fill_tx_bridge = fill_tx.clone();
    tokio::spawn(async move {
        while let Some(f) = fill_mpsc_rx.recv().await {
            let _ = fill_tx_bridge.send(f);
        }
    });

    // --- Polygon.io (optional, supplementary quotes) ---
    if let Ok(api_key) = std::env::var("POLYGON_API_KEY") {
        tracing::info!("Polygon.io API key found — starting quote polling (15s)");
        let polygon = talon_data::polygon::PolygonClient::new(api_key);
        let polygon_tx = quote_tx.clone();
        polygon.start_polling(watchlist.clone(), polygon_tx, Duration::from_secs(15));
    } else {
        tracing::warn!("POLYGON_API_KEY not set — Polygon data disabled");
    }

    // --- Yahoo Finance (always-on fallback, 30s interval) ---
    match talon_data::yahoo::YahooClient::new() {
        Ok(yahoo) => {
            tracing::info!("Yahoo Finance fallback started (30s interval)");
            let yahoo_tx = quote_tx.clone();
            yahoo.start_fallback_polling(watchlist.clone(), yahoo_tx, Duration::from_secs(30));
        }
        Err(e) => {
            tracing::warn!(error = %e, "Yahoo Finance client failed to initialize");
        }
    }

    // --- State Update Loop ---
    let state_update_tx = app_state_tx.clone();
    let mut quote_rx = quote_tx.subscribe();
    let ibkr_for_state = Arc::clone(&ibkr);
    let startup_time = Instant::now();

    // Set initial connection status
    app_state_tx.send_modify(|state| {
        state.connection_status = talon_types::channel::ConnectionStatus::Connected;
    });

    tokio::spawn(async move {
        let mut latest_quotes: HashMap<Symbol, QuoteEvent> = HashMap::new();
        let mut account_tick = tokio::time::interval(Duration::from_secs(5));

        loop {
            tokio::select! {
                result = quote_rx.recv() => {
                    if let Ok(q) = result {
                        latest_quotes.insert(q.symbol.clone(), q);
                        let wq: Vec<QuoteEvent> = latest_quotes.values().cloned().collect();
                        let uptime = startup_time.elapsed().as_secs_f64();
                        state_update_tx.send_modify(|state| {
                            state.watchlist_quotes = wq;
                            state.uptime_secs = uptime;
                        });
                    }
                }
                _ = account_tick.tick() => {
                    // Fetch account + positions via spawn_blocking
                    // (BrokerCommands methods use Handle::current().block_on())
                    let broker = Arc::clone(&ibkr_for_state);
                    let uptime = startup_time.elapsed().as_secs_f64();
                    if let Ok((account, positions)) = tokio::task::spawn_blocking(move || {
                        let _guard = tokio::runtime::Handle::current().enter();
                        let account = broker.account_snapshot().ok();
                        let positions = broker.positions().unwrap_or_default();
                        (account, positions)
                    }).await {
                        state_update_tx.send_modify(|state| {
                            state.account = account;
                            state.positions = positions;
                            state.uptime_secs = uptime;
                        });
                    }
                }
            }
        }
    });

    // --- Risk Engine ---
    let starting_balance = Decimal::from(10_000);
    let tier_params = TierRiskParams {
        max_single_position_risk_pct: Decimal::from(5),
        max_total_exposure_pct: Decimal::from(60),
        max_concurrent_positions: 5,
        drawdown_circuit_breaker_pct: Decimal::from(-15),
        daily_loss_limit_pct: Decimal::from(5),
    };

    let stress_params = StressParams {
        tier_0_threshold_pct: Decimal::new(30, 1),
        tier_2_threshold_pct: Decimal::new(50, 1),
        tier_3_threshold_pct: Decimal::new(80, 1),
        circuit_breaker_pct: Decimal::new(150, 1),
        override_cooldown_mins: 15,
    };

    let risk_mesh = RiskMesh::new(tier_params, ModuleRiskAllocation::default());
    let stress = StressEngine::new(stress_params, starting_balance);

    // --- ExecCore (intent processing with supervision gate) ---
    let exec_core = ExecCore::new(risk_mesh, stress, session_manager, BrokerId::Ibkr);
    let exec_app_state_tx = app_state_tx.clone();
    tokio::spawn(exec_core.run(intent_rx, supervision_rx, exec_app_state_tx));

    // --- Modules (orchestration loop: quotes → modules → intents + approaching) ---
    let mut firebird = talon_firebird::Firebird::new();
    let mut thunderbird = talon_thunderbird::Thunderbird::new();
    let mut taxi = talon_taxi::Taxi::new();

    let mut module_quote_rx = quote_tx.subscribe();
    let module_intent_tx = intent_tx.clone();
    let module_state_tx = app_state_tx.clone();

    tokio::spawn(async move {
        loop {
            match module_quote_rx.recv().await {
                Ok(quote) => {
                    // Feed quote to all modules
                    let fb = firebird.on_quote(&quote).await;
                    let tb = thunderbird.on_quote(&quote).await;
                    let tx = taxi.on_quote(&quote).await;

                    // Collect intents → send to governor
                    for intent in fb
                        .intents
                        .into_iter()
                        .chain(tb.intents)
                        .chain(tx.intents)
                    {
                        if module_intent_tx.send(intent).await.is_err() {
                            tracing::warn!("intent channel closed");
                            return;
                        }
                    }

                    // Merge approaching setups
                    let mut approaching = Vec::new();
                    approaching.extend(fb.approaching);
                    approaching.extend(tb.approaching);
                    approaching.extend(tx.approaching);

                    // Update AppState with module states + approaching setups
                    module_state_tx.send_modify(|state| {
                        state.module_states = vec![
                            ModuleStateEntry {
                                module: ModuleId::Firebird,
                                state: firebird.state(),
                                signals_generated: firebird.signals_generated(),
                                signals_approved: 0,
                                signals_rejected: 0,
                                pending_intent: None,
                            },
                            ModuleStateEntry {
                                module: ModuleId::Thunderbird,
                                state: thunderbird.state(),
                                signals_generated: thunderbird.signals_generated(),
                                signals_approved: 0,
                                signals_rejected: 0,
                                pending_intent: None,
                            },
                            ModuleStateEntry {
                                module: ModuleId::Taxi,
                                state: taxi.state(),
                                signals_generated: taxi.signals_generated(),
                                signals_approved: 0,
                                signals_rejected: 0,
                                pending_intent: None,
                            },
                        ];
                        state.approaching_setups = approaching;
                    });
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(lagged = n, "module orchestration loop lagged");
                }
                Err(_) => break,
            }
        }
    });

    // --- FlowManager (L2 subscription lifecycle) ---
    let flow_mgr = flow_manager::FlowManager::new(
        Arc::clone(&ibkr),
        flow_cmd_rx,
        app_state_tx.clone(),
    );
    tokio::spawn(flow_mgr.run());

    // --- Chart Data Manager (Polygon aggregates for candlestick chart) ---
    let chart_state_tx = app_state_tx.clone();
    let polygon_key_for_chart = std::env::var("POLYGON_API_KEY").ok();
    tokio::spawn(async move {
        run_chart_manager(chart_cmd_rx, chart_state_tx, polygon_key_for_chart).await;
    });

    // --- TUI ---
    talon_triminl::render::run_tui(app_state_rx, flow_cmd_tx, supervision_tx, chart_cmd_tx).await?;

    // --- Shutdown ---
    event_store.shutdown()?;
    tracing::info!("TALON shutdown complete");

    Ok(())
}

use talon_types::flow::ChartCmd;

/// Chart data manager — fetches Polygon aggregates on TUI navigation.
async fn run_chart_manager(
    mut cmd_rx: tokio::sync::mpsc::Receiver<ChartCmd>,
    state_tx: watch::Sender<talon_types::channel::AppState>,
    polygon_key: Option<String>,
) {
    let polygon = polygon_key.map(talon_data::polygon::PolygonClient::new);

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            ChartCmd::FetchCandles { symbol, timeframe } => {
                let Some(ref client) = polygon else {
                    tracing::warn!("Chart data requested but POLYGON_API_KEY not set");
                    state_tx.send_modify(|s| {
                        s.chart_candles.clear();
                        s.chart_symbol = Some(symbol.clone());
                        s.chart_timeframe = timeframe;
                    });
                    continue;
                };

                // Calculate date range based on timeframe
                let today = chrono::Utc::now().date_naive();
                let from = match timeframe {
                    talon_types::broker::Timeframe::Min1
                    | talon_types::broker::Timeframe::Min5
                    | talon_types::broker::Timeframe::Min15
                    | talon_types::broker::Timeframe::Min30
                    | talon_types::broker::Timeframe::Hour1 => {
                        today - chrono::Duration::days(5)
                    }
                    talon_types::broker::Timeframe::Day => {
                        today - chrono::Duration::days(365)
                    }
                    talon_types::broker::Timeframe::Week => {
                        today - chrono::Duration::days(730)
                    }
                    talon_types::broker::Timeframe::Month => {
                        today - chrono::Duration::days(1825)
                    }
                };

                match client.aggregates(&symbol, timeframe, from, today).await {
                    Ok(candles) => {
                        tracing::debug!(
                            symbol = %symbol,
                            timeframe = %timeframe,
                            count = candles.len(),
                            "Chart candles fetched"
                        );
                        state_tx.send_modify(|s| {
                            s.chart_candles = candles;
                            s.chart_symbol = Some(symbol.clone());
                            s.chart_timeframe = timeframe;
                        });
                    }
                    Err(e) => {
                        tracing::warn!(
                            symbol = %symbol,
                            error = %e,
                            "Failed to fetch chart candles"
                        );
                        state_tx.send_modify(|s| {
                            s.chart_candles.clear();
                            s.chart_symbol = Some(symbol.clone());
                            s.chart_timeframe = timeframe;
                        });
                    }
                }
            }
        }
    }
}

/// Load the watchlist from config/talon.toml [watchlist] symbols array.
fn load_watchlist(path: &str) -> anyhow::Result<Vec<Symbol>> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read {path}: {e}"))?;
    let table: toml::Table = raw.parse()
        .map_err(|e| anyhow::anyhow!("Failed to parse {path}: {e}"))?;
    let symbols = table
        .get("watchlist")
        .and_then(|w| w.get("symbols"))
        .and_then(|s| s.as_array())
        .ok_or_else(|| anyhow::anyhow!("{path} missing [watchlist] symbols array"))?;
    let list: Vec<Symbol> = symbols
        .iter()
        .filter_map(|v| v.as_str().map(Symbol::new))
        .collect();
    if list.is_empty() {
        anyhow::bail!("{path} [watchlist] symbols is empty");
    }
    Ok(list)
}
