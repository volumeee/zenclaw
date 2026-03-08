use std::sync::Arc;
use crate::setup_bot_env;

pub async fn run_serve(
    cli_host: &str,
    cli_port: u16,
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    cli_api_key: Option<&str>,
) -> anyhow::Result<()> {
    let (agent, provider, memory, resolved_provider, resolved_model) = setup_bot_env(
        cli_provider,
        cli_model,
        cli_api_key,
        None,
        None
    ).await?;

    let data = crate::setup::data_dir();
    let rag_path = data.join("rag.db");
    let rag = zenclaw_hub::memory::RagStore::open(&rag_path).ok().map(Arc::new);

    let host = cli_host.to_string();
    let mut port = cli_port;

    let agent = Arc::new(agent);
    let provider = Arc::new(provider);
    let memory = Arc::new(memory);

    loop {
        let state = zenclaw_hub::api::ApiState {
            agent: agent.clone(),
            provider: provider.clone(),
            memory: memory.clone(),
            rag: rag.clone(),
        };

        // Fail-fast test to see if we can bind to the port
        let addr_str = format!("{}:{}", host, port);
        match tokio::net::TcpListener::bind(&addr_str).await {
            Ok(listener) => {
                drop(listener); // Close it so AXUM can take it

                // Run server in background
                let bg_host = host.clone();
                let bg_port = port;
                let bg_state = state; 
                tokio::spawn(async move {
                    let _ = zenclaw_hub::api::start_server_from_state(bg_state, &bg_host, bg_port).await;
                });

                // Interactively monitor via TUI
                let endpoint = format!("http://{}:{}", host, port);
                let details = [
                    ("Host", host.as_str()),
                    ("Port", &port.to_string()),
                    ("Status", "Listening"),
                    ("Endpoint", endpoint.as_str()),
                ];
                let _ = crate::tui_menu::run_bot_dashboard("REST API", &resolved_provider, &resolved_model, &details, None);
                break Ok(());
            }
            Err(e) => {
                let _ = crate::tui_menu::run_tui_error("Server Startup Failed", &format!("Address {} error: {}\n\nPlease try a different port.", addr_str, e));
                let input = crate::tui_menu::run_tui_input("Assign New Port", "Enter Port Number:", &port.to_string(), false)?;
                if let Some(p_str) = input && let Ok(p) = p_str.parse() {
                    port = p;
                    continue;
                }
                break Err(anyhow::anyhow!("Port binding failed. Aborting."));
            }
        }
    }
}
