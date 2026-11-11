#!/usr/bin/env bash
# For the OWNER to run in his own terminal: prints the DEV demo accounts and their password.
# (An agent never runs this: it would put the value in a transcript — rules §9.)
set -euo pipefail
ssh -i "${UG_DEV_SSH_KEY:-$HOME/.ssh/hetzner}" "${UG_DEV_HOST:-root@65.108.87.247}" \
  'sed -n "s/^UG_SEED_PASSWORD=//p" /opt/underground-dev/.env.dev' | {
  read -r pw
  echo "DEV: https://underground-dev.65-108-87-247.sslip.io   admin: https://underground-admin-dev.65-108-87-247.sslip.io"
  echo "accounts: user@underground.dev (guest), maker@underground.dev (organiser), admin@underground.dev (admin)"
  echo "password: $pw"
}
