# ingjoo 开发命令
# 安装: cargo install just

set dotenv-load

# 默认：构建 + 测试
default: build test

# 构建
build:
    cargo build

# 构建 release
build-release:
    cargo build --release

# 运行全部测试
test:
    cargo test --all-features

# 运行指定 crate 的测试
test-crate crate:
    cargo test -p {{crate}} --all-features

# 运行集成测试
test-integration:
    cargo test -p ingjoo-bin --test integration_test

# 运行 infra 单元测试
test-infra:
    cargo test -p ingjoo-infra --all-features

# lint: clippy
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# 格式检查
fmt-check:
    cargo fmt --all -- --check

# 格式化
fmt:
    cargo fmt --all

# 运行开发服务器
dev:
    cargo run -p ingjoo-bin

# 全量检查 (CI 本地版)
ci: fmt-check lint test
    @echo "全量检查通过 ✓"

# 清理构建产物
clean:
    cargo clean

# 检查依赖是否有安全漏洞
audit:
    cargo install cargo-audit --quiet && cargo audit

# 查看依赖树
tree:
    cargo tree --all-features
