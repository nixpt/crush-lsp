//! crush-lsp server binary — run as `crush-lsp` (stdio transport, standard
//! LSP convention). Point your editor's language client at this binary.

use crush_lsp::CrushLanguageServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    tracing::info!("Starting crush-lsp...");

    let (service, socket) = tower_lsp::LspService::new(CrushLanguageServer::new);

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    tower_lsp::Server::new(stdin, stdout, socket)
        .serve(service)
        .await;

    tracing::info!("crush-lsp stopped");
    Ok(())
}
