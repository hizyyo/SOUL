# Windows Release Hardening

## Статус

`PARTIAL`. Windows release gate закрыт, но этап не означает production readiness: signing, notarization, updater publication, SBOM/license review и внешняя P0 validation не завершены.

## Исправлено

- Browser Companion корректно работает как MV3 service worker, перехватывает Enter только внутри composer и использует delegated send-button handling после SPA rerender.
- Вставляемый SOUL context явно маркируется недоверенными данными; reserved markers экранируются.
- B1 baseline больше не сохраняет raw claims в `localStorage`; delete очищает legacy storage.
- Добавлен доступный modal primitive с focus trap, Escape и восстановлением фокуса; исправлены double-submit, tab/modal semantics и responsive layouts.
- Device keypair теперь fail-closed при неполной или несовместимой паре. На Windows приватный ключ и MCP capability защищены DPAPI; legacy raw secrets мигрируют атомарно.
- SQLCipher startup восстанавливает прерванную миграцию и не удаляет нечитаемую базу.
- Import/export усилены лимитами, полной проверкой event chain, восстановлением policy/preview invariants и backend-owned file dialogs. Restore token привязан к hash проверенного plaintext, поэтому подмена пакета после preview отклоняется.
- Full wipe сохраняет installation key; candidate mutation отзывает preview confirmation.
- Контекст использует explicit active SOUL; sensitive/restricted исключены по умолчанию.
- MCP требует локальную capability, а конфигурации Claude/Codex/Cursor получают app dir, client id и capability.
- Replay nonce cache Browser Companion ограничен 4096 элементами.
- Windows config replacement получил backup/rollback/recovery; recovery выполняется до чтения config при connect/detect/disconnect/rollback.

## Release hardening

- Добавлены pinned Node `26.4.0`, pnpm `11.12.0` и Rust `1.97.1`.
- Добавлен Windows GitHub Actions quality workflow для frontend, Rust и release gates.
- `soul-mcp` и `soul-bridge` объявлены Tauri sidecars; `scripts/prepare-sidecars.mjs` готовит target-triple binaries через проверенный MSVC bootstrap.
- Production CSP удаляет `unsafe-inline` из `script-src` и не содержит localhost/WebSocket origins; development origins вынесены в `devCsp`.
- Vite config переведён с `__dirname` на `import.meta.dirname`.
- Реальные binary E2E tests перенесены в release gate, чтобы `cargo test --lib` не запускал stale executables из `target`.
- Release gate готовит sidecars до lib-тестов для clean checkout, затем повторно после них; hash-проверка связывает prepared binaries с финальными Cargo outputs до packaging.
- Tauri release gate собирает именно NSIS bundle и выполняет silent clean-install smoke: проверяет `SOUL.exe`, SHA-256 каждого установленного sidecar против staged payload и реальные MCP/native-messaging responses из установленного каталога.

## Проверки 6 августа 2026 года

- `cargo fmt --check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `cargo test --lib`: PASS - 245 passed, 5 release-only ignored.
- `pnpm typecheck`, `pnpm lint`, `pnpm build`, `pnpm build:companion`: PASS.
- `pnpm test`: PASS - 270/270.
- Release-only tests: PASS - 5/5, включая реальные `soul-mcp` и `soul-bridge`; policy p95 28.1 us, cold context p95 29.85 ms, cached context p95 22 us.
- Windows release build: PASS. Собран `SOUL_0.1.0_x64-setup.exe`; до packaging проверены SHA-256 application payload и обоих sidecars.
- NSIS smoke: PASS. Silent install завершился с кодом 0; установленный каталог содержит непустые `SOUL.exe`, `soul-mcp.exe` и `soul-bridge.exe`. Установленные sidecars совпали по SHA-256 со staged payload и прошли реальные MCP/native-messaging smoke tests.
- `git diff --check`: PASS.

## Оставшиеся блокеры

- P0 validation: `BLOCKED` - нет внешних участников, day-7/day-28 evidence и решения основателя.
- Signing/notarization: нет сертификатов и CI secret storage.
- Secure updater: нет update signing key, HTTPS release channel и rollback infrastructure.
- SBOM/license audit: не реализован.
- Cross-platform CI и OS keychain для macOS/Linux: не реализованы; `cargo audit` требует closure или documented risk acceptance для transitive GTK3/glib advisories на не-Windows platforms.
