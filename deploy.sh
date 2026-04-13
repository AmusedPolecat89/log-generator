#!/usr/bin/env bash
set -euo pipefail

# --- Configuration ---
INSTANCE_IP="${1:?Usage: ./deploy.sh <instance-ip>}"
PEM_KEY="$(cd "$(dirname "$0")" && pwd)/Demo.pem"
REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
REMOTE_USER="ubuntu"
REMOTE_DIR="~/log-generator"
DAEMON_PORT=8888
SSH_OPTS="-o StrictHostKeyChecking=no -o ConnectTimeout=10 -i $PEM_KEY"

# --- Helpers ---
ssh_cmd() { ssh $SSH_OPTS "$REMOTE_USER@$INSTANCE_IP" "$@"; }

echo "==> Deploying log-generator to $INSTANCE_IP"

# --- 1. Wait for instance to be SSH-ready ---
echo "--- Waiting for SSH to be ready..."
for i in $(seq 1 30); do
    if ssh_cmd "true" 2>/dev/null; then
        echo "    SSH is ready."
        break
    fi
    if [ "$i" -eq 30 ]; then
        echo "    ERROR: SSH not available after 30 attempts." >&2
        exit 1
    fi
    sleep 2
done

# --- 2. Install build dependencies ---
echo "--- Installing build dependencies..."
ssh_cmd "which cargo > /dev/null 2>&1 || (curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y)"
ssh_cmd "dpkg -s build-essential > /dev/null 2>&1 || (sudo apt-get update -qq && sudo apt-get install -y -qq build-essential pkg-config libssl-dev)"
echo "    Done."

# --- 3. Rsync the repo ---
echo "--- Syncing code to $REMOTE_USER@$INSTANCE_IP:$REMOTE_DIR..."
rsync -az --delete \
    --exclude target \
    --exclude .git \
    --exclude .DS_Store \
    -e "ssh $SSH_OPTS" \
    "$REPO_DIR/" "$REMOTE_USER@$INSTANCE_IP:$REMOTE_DIR/"
echo "    Done."

# --- 4. Build in release mode ---
echo "--- Building release binary (this may take a few minutes on first run)..."
ssh_cmd "source ~/.cargo/env && cd $REMOTE_DIR && cargo build --release 2>&1 | tail -3"
echo "    Done."

# --- 5. Stop any existing daemon, then start fresh ---
echo "--- Starting daemon on port $DAEMON_PORT..."
ssh_cmd "pkill -x log-generator 2>/dev/null || true"
sleep 1
ssh -n $SSH_OPTS "$REMOTE_USER@$INSTANCE_IP" \
    "cd $REMOTE_DIR && setsid -f ./target/release/log-generator --daemon $DAEMON_PORT > /tmp/daemon.log 2>&1 < /dev/null"
sleep 2

STATUS=$(ssh_cmd "curl -s http://localhost:$DAEMON_PORT/status" 2>/dev/null || echo "FAILED")
if echo "$STATUS" | grep -q '"state"'; then
    echo "    Daemon is running. Status: $STATUS"
else
    echo "    ERROR: Daemon failed to start. Check /tmp/daemon.log on the instance." >&2
    ssh_cmd "cat /tmp/daemon.log" 2>/dev/null
    exit 1
fi

# --- 6. Configure security group ---
echo "--- Configuring security group..."

# Find the security group for the instance
INSTANCE_ID=$(aws ec2 describe-instances \
    --filters "Name=ip-address,Values=$INSTANCE_IP" \
    --query 'Reservations[0].Instances[0].InstanceId' \
    --output text 2>/dev/null)

if [ -z "$INSTANCE_ID" ] || [ "$INSTANCE_ID" = "None" ]; then
    echo "    WARNING: Could not find instance by IP. Skipping security group setup."
    echo "    Make sure port $DAEMON_PORT is open in your security group manually."
else
    SG_ID=$(aws ec2 describe-instances \
        --instance-ids "$INSTANCE_ID" \
        --query 'Reservations[0].Instances[0].SecurityGroups[0].GroupId' \
        --output text)

    echo "    Instance: $INSTANCE_ID  Security Group: $SG_ID"

    # Open port 22 (SSH) if not already open
    aws ec2 authorize-security-group-ingress \
        --group-id "$SG_ID" --protocol tcp --port 22 --cidr 0.0.0.0/0 2>/dev/null \
        && echo "    Opened port 22 (SSH)" \
        || echo "    Port 22 (SSH) already open"

    # Open daemon port if not already open
    aws ec2 authorize-security-group-ingress \
        --group-id "$SG_ID" --protocol tcp --port "$DAEMON_PORT" --cidr 0.0.0.0/0 2>/dev/null \
        && echo "    Opened port $DAEMON_PORT (daemon API)" \
        || echo "    Port $DAEMON_PORT (daemon API) already open"
fi

# --- Done ---
echo ""
echo "==> Deployment complete!"
echo "    Daemon API: http://$INSTANCE_IP:$DAEMON_PORT"
echo "    Status:     curl http://$INSTANCE_IP:$DAEMON_PORT/status"
echo "    Start:      curl -X POST http://$INSTANCE_IP:$DAEMON_PORT/start -d '{...}'"
echo "    Stop:       curl -X POST http://$INSTANCE_IP:$DAEMON_PORT/stop"
