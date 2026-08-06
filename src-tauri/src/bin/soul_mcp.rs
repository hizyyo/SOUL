//! Локальный MCP-сервер SOUL (stdio transport).
//!
//! Запускается coding-клиентами (Claude Code, Codex, Cursor) как фоновый
//! процесс. Единственный канал — stdio (JSON-RPC 2.0, newline-delimited).
//! База данных читается только на чтение; в stderr пишутся только ошибки,
//! stdout зарезервирован за протоколом.

fn main() {
    let app_dir = match soul_lib::mcp::resolve_app_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("soul-mcp: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = soul_lib::mcp::authorize_process(&app_dir) {
        eprintln!("soul-mcp: {e}");
        std::process::exit(1);
    }
    if let Err(e) = soul_lib::mcp::serve_stdio(&app_dir) {
        eprintln!("soul-mcp: {e}");
        std::process::exit(1);
    }
}
