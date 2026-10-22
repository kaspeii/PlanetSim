.DEFAULT_GOAL := help
.PHONY: help check build release run dev test lint fmt fmt-check setup-linux doc audit watch ci profiling run-profiling flamegraph release-thin setup-windows clean

## help: Показать список доступных команд
help:
	@echo "Использование: make [цель]"
	@echo ""
	@echo "Доступные цели:"
	@sed -n 's/^##//p' $(MAKEFILE_LIST) | column -t -s ':' | sed -e 's/^/ /'

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