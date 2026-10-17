.DEFAULT_GOAL := help
.PHONY: help build release run check test lint fmt fmt-check clean doc audit watch

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

## clean: Очистка директории target/
clean:
	cargo clean