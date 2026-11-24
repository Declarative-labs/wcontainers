# tools.sh: installs a nightly with Miri, and Kani, into the folder given (rustup/, cargo/, kani/), for a machine that
# has neither; miri.sh and kani.sh take the folder as MIRI_HOME and KANI_HOME.
set -eu
home=$1
export RUSTUP_HOME=$home/rustup CARGO_HOME=$home/cargo KANI_HOME=$home/kani PATH=$home/cargo/bin:$PATH
mkdir -p "$home"
[ -x "$home/cargo/bin/rustup" ] || curl -sSf https://sh.rustup.rs | sh -s -- -y -q --no-modify-path --default-toolchain stable --profile minimal
rustup toolchain install nightly --profile minimal --component miri,rust-src
cargo +nightly miri setup 2>&1 | tail -1
[ -x "$home/cargo/bin/cargo-kani" ] || cargo install --locked kani-verifier 2>&1 | tail -2
cargo kani setup 2>&1 | tail -2
echo "tools: $(cargo +nightly miri --version), $(cargo kani --version 2>&1 | tail -1)"
