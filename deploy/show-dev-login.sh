#!/usr/bin/env bash
# For the OWNER to run in his own terminal: prints the DEV demo accounts and their passwords.
# (An agent never runs this: it would put the values in a transcript — rules §9.)
set -euo pipefail
ssh -i "${UG_DEV_SSH_KEY:-$HOME/.ssh/hetzner}" "${UG_DEV_HOST:-root@65.108.87.247}" \
  'sed -n -e "s/^UG_SEED_PASSWORD=/demo /p" -e "s/^UG_ADMIN_PASSWORD=/admin /p" /opt/underground-dev/.env.dev' | {
  read -r _ demo; read -r _ admin
  echo "DEV:   https://underground-dev.65-108-87-247.sslip.io"
  echo "Admin: https://underground-admin-dev.65-108-87-247.sslip.io"
  echo "user@underground.dev  (guest)      password: $demo"
  echo "maker@underground.dev (organiser)  password: $demo"
  echo "admin@underground.dev (admin)      password: $admin"
}
