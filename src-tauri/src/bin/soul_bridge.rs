//! Native Messaging host SOUL (Browser Companion bridge).
//!
//! Запускается браузером через зарегистрированный host
//! `com.soul.browser_companion` при подключении расширения. Каналы — stdin и
//! stdout с кадрами u32-LE длина + JSON (Chrome Native Messaging). stderr
//! используется только для ошибок и никогда не содержит пак контекста.

fn main() {
    let app_dir = match soul_lib::mcp::resolve_app_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("soul-bridge: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = soul_lib::bridge::serve_native_messaging(&app_dir) {
        eprintln!("soul-bridge: {e}");
        std::process::exit(1);
    }
}
