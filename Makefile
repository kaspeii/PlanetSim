.DEFAULT_GOAL := help
.PHONY: help check build release run dev test lint fmt fmt-check setup-linux doc audit watch ci profiling run-profiling flamegraph release-thin setup-windows clean setup install-rustup install-cargo-tools install-llvm-tools setup-tools setup-all

## help: Показать список доступных команд
help:
	@echo "Использование: make [цель]"
	@echo ""
	@echo "Доступные цели:"
	@sed -n 's/^##//p' $(MAKEFILE_LIST) | column -t -s ':' | sed -e 's/^/ /'

## setup: Минимальная настройка — Rust + компоненты + системные библиотеки
setup: install-rustup
	@if [ "$$(uname)" = "Linux" ]; then $(MAKE) setup-linux; fi
	@echo "🎉 Готово. Можно делать make build / make run / make ci"

## setup-tools: Опциональные dev-утилиты (watch, audit, flamegraph)
setup-tools: install-rustup install-cargo-tools

## setup-all: Всё сразу (setup + setup-tools)
setup-all: setup setup-tools

## install-rustup: Установка rustup и тулчейна stable (если ещё не установлены)
install-rustup:
	@if command -v rustup >/dev/null 2>&1; then \
		echo "✅ rustup уже установлен: $$(rustup --version | head -n1)"; \
	else \
		echo "📦 Устанавливаю rustup..."; \
		curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y; \
		echo "🔁 Добавляю ~/.cargo/bin в PATH для текущей сессии..."; \
		. "$$HOME/.cargo/env"; \
	fi
	@echo "🔧 Настраиваю тулчейн stable по умолчанию..."
	@rustup default stable
	@rustup component add rustfmt clippy
	@echo "✅ Rust stable готов."

## install-cargo-tools: Установка вспомогательных cargo-утилит
install-cargo-tools: install-rustup
	@echo "📦 Проверяю и устанавливаю cargo-watch, cargo-audit, flamegraph..."
	@command -v cargo-watch >/dev/null 2>&1 || cargo install cargo-watch
	@command -v cargo-audit >/dev/null 2>&1 || cargo install cargo-audit
	@command -v flamegraph >/dev/null 2>&1 || cargo install flamegraph
	@echo "✅ Утилиты cargo установлены."

## install-llvm-tools: Установка LLVM-инструментов (актуально для Windows/MSVC)
install-llvm-tools: install-rustup
	@if [ "$$(uname -s)" = "MINGW64_NT" ] || [ "$$(uname -s)" = "MSYS_NT" ]; then \
		echo "🪟 Windows: добавляю llvm-tools-preview..."; \
		rustup component add llvm-tools-preview; \
	else \
		echo "ℹ️  llvm-tools-preview не требуется для текущей ОС."; \
	fi

## check: Быстрая проверка компилируемости без генерации бинарника
check:
	cargo check

## build: Сборка отладочной версии (debug)
build:
	cargo build

## release: Оптимизированная сборка (release)
release:
	cargo build --release

## run: Запуск отладочной сборки
run:
	cargo run

## dev: Запуск с динамической линковкой для быстрой компиляции (Bevy)
dev:
	cargo run --features bevy/dynamic_linking

## test: Запуск всех тестов
test:
	cargo test -- --nocapture

## lint: Проверка кода через Clippy (ошибки при любых предупреждениях)
lint:
	cargo clippy --all-targets --all-features -- -D warnings

## fmt: Автоматическое форматирование исходников через rustfmt
fmt:
	cargo fmt --all

## fmt-check: Проверка стиля кода без изменения файлов (для CI)
fmt-check:
	cargo fmt --all -- --check

## setup-linux: Установка системных C-библиотек для Ubuntu/Debian
setup-linux:
	sudo apt-get update
	sudo apt-get install -y g++ pkg-config libx11-dev libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev

## doc: Генерация документации проекта (без сторонних зависимостей)
doc:
	cargo doc --no-deps --open

## audit: Проверка зависимостей на уязвимости (требуется: cargo install cargo-audit)
audit:
	cargo audit

## watch: Непрерывная проверка и тесты при изменении файлов (требуется: cargo install cargo-watch)
watch:
	cargo watch -x check -x "test -- --nocapture"

## ci: Полный прогон проверок для CI (проверка стиля, линтинг, тесты)
ci: fmt-check lint test

## profiling: Сборка с оптимизациями и отладочными символами
profiling:
	cargo build --profile profiling

## run-profiling: Запуск профилировочной сборки
run-profiling:
	cargo run --profile profiling

## flamegraph: Записать flamegraph (требуется: cargo install flamegraph)
flamegraph:
	RUSTFLAGS='-C force-frame-pointers=y' cargo flamegraph --profile profiling --bin SocSim

## release-thin: Быстрая оптимизированная сборка (thin-LTO)
release-thin:
	cargo build --profile release-thin

## setup-windows: Установка компонентов LLVM (lld) для Windows
setup-windows:
	rustup component add llvm-tools-preview

## clean: Очистка директории target/
clean:
	cargo clean