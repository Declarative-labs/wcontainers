# shellcheck shell=bash disable=SC2034
# Shared by the rig's Mac commands: the desktop's address, this checkout's branch name there, and
# ssh and scp through one shared connection per checkout, which is opened first and waited for when
# the desktop refuses new connections for a while.
host=tom@192.168.2.183
branch=$(git symbolic-ref --short HEAD)
name=$(printf %s "$branch" | tr -c 'A-Za-z0-9._-' '-')
socket=$HOME/.ssh/wc-rig-$(git rev-parse --absolute-git-dir | shasum | cut -c1-16)
ssh_options=(-o BatchMode=yes -o ConnectTimeout=20 -o ServerAliveInterval=30
    -o ControlMaster=auto -o "ControlPath=$socket" -o ControlPersist=600)

# Opens the shared connection, retrying every 15 s for up to 20 minutes.
connect() {
    local start=$SECONDS said=
    until ssh "${ssh_options[@]}" "$host" exit 2>/dev/null; do
        if ((SECONDS - start > 1200)); then
            echo "rig: the desktop has not taken an ssh connection for 20 minutes" >&2
            return 1
        fi
        [ -n "$said" ] || { echo "rig: waiting for ssh to the desktop" >&2; said=1; }
        sleep 15
    done
    [ -z "$said" ] || echo "rig: ssh to the desktop is back after $((SECONDS - start)) s" >&2
}

# shellcheck disable=SC2029
rig_ssh() { ssh "${ssh_options[@]}" "$host" "$@"; }
rig_scp() { scp -q "${ssh_options[@]}" "$@"; }

# Runs a desktop PowerShell script with arguments, its output and error with carriage returns removed;
# returns its exit code.
rig_powershell() {
    rig_ssh powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$@" \
        2> >(tr -d '\r' >&2) | tr -d '\r'
    return "${PIPESTATUS[0]}"
}

# The tree id of this checkout's working tree as it is: untracked files included, ignored ones left
# out. Leaves the index it built at $1 for the caller to remove.
working_tree() {
    local index=$1
    cp "$(git rev-parse --absolute-git-dir)/index" "$index"
    GIT_INDEX_FILE=$index git add -A
    GIT_INDEX_FILE=$index git write-tree
}
