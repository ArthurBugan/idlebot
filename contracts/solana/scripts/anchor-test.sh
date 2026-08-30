#!/usr/bin/env bash
# Build + run the IdleBot Solana contract tests against the `solana`
# test-validator service in docker-compose.
set -euo pipefail

cd /code/contracts/solana

# Reuse the already-running validator instead of spawning our own.
export ANCHOR_SKIP_LOCAL_VALIDATOR=1
export ANCHOR_PROVIDER_URL=http://solana:8899
export ANCHOR_PROVIDER_WS_URL=ws://solana:8900
solana config set --url http://solana:8899

echo "Waiting for Solana test validator at http://solana:8899 ..."
for _ in $(seq 1 30); do
    if solana cluster-version --url http://solana:8899 >/dev/null 2>&1; then
        echo "Validator is up."
        break
    fi
    sleep 2
done

# Provider wallet referenced by Anchor.toml ([provider] wallet).
mkdir -p ~/.config/solana
if [ ! -f ~/.config/solana/idlebot-wallet.json ]; then
    solana-keygen new -o ~/.config/solana/idlebot-wallet.json \
        --force --no-bip39-passphrase
fi
solana airdrop 5 || true

# Point the provider at localnet for this run (container-only copy of
# Anchor.toml, so the host repo is untouched).
sed -i 's/^cluster = .*/cluster = "localnet"/' Anchor.toml

exec anchor test
