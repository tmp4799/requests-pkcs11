# Linux build/test environment for requests-pkcs11.
# Authoritative build target is Linux; this mirrors it for offline development.
FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# System dependencies:
#   softhsm2  - software PKCS#11 token, so we can test the whole mTLS path
#               offline without the real CSPID module.
#   opensc    - provides pkcs11-tool for loading/inspecting token objects.
#   openssl   - cert generation and the local mTLS test server (s_server).
#   cmake/libssl-dev/pkg-config/clang - build aws-lc-sys, the default crypto
#               provider pulled in by rustls 0.23.
#   python3 + venv - target runtime for the PyO3 extension.
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install -y --no-install-recommends \
        softhsm2 opensc \
        openssl libssl-dev pkg-config cmake clang \
        python3 python3-pip python3-venv \
    && rm -rf /var/lib/apt/lists/*

# maturin builds the Python extension from crates/py.
RUN pip3 install --no-cache-dir --break-system-packages 'maturin>=1.5,<2.0'
