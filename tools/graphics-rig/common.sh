# shellcheck shell=bash disable=SC2034
# Shared by the rig's Mac commands: the desktop's address, this checkout's branch name there, and
# ssh and scp through one long-lived connection per branch, which is opened first and waited for
# when the desktop refuses new connections for a while.
host=tom@192.168.2.183
branch=$(git symbolic-ref --short HEAD)
name=$(printf %s "$branch" | tr -c 'A-Za-z0-9._-' '-')
socket=$HOME/.ssh/wc-rig-$name
ssh_options=(-o BatchMode=yes -o ConnectTimeout=20 -o ServerAliveInterval=30 -o "ControlPath=$socket")

# Opens the branch's shared connection unless it is open, and keeps it for an hour after its last
# use. One script of the branch at a time tries; after a refused attempt it waits 30 s, then twice
# as long after each next one, at most 5 minutes, for up to 30 minutes: a refused logon may count
# towards the account's lockout (ten in ten minutes), so this makes at most five in ten minutes.
connect() {
    ssh "${ssh_options[@]}" -O check "$host" 2>/dev/null && return 0
    local start=$SECONDS wait=30 lock=$socket.lock said=
    until mkdir "$lock" 2>/dev/null; do
        if [ -f "$lock/pid" ] && ! kill -0 "$(cat "$lock/pid" 2>/dev/null)" 2>/dev/null; then
            rm -rf "$lock"
            continue
        fi
        sleep 2
        ssh "${ssh_options[@]}" -O check "$host" 2>/dev/null && return 0
        if ((SECONDS - start > 1800)); then
            echo "rig: another script of this branch has waited 30 minutes for ssh to the desktop" >&2
            return 1
        fi
    done
    echo $$ >"$lock/pid"
    until ssh "${ssh_options[@]}" -o ControlMaster=yes -o ControlPersist=3600 -f -N "$host" </dev/null >/dev/null 2>&1; do
        if ((SECONDS - start + wait > 1800)); then
            rm -rf "$lock"
            echo "rig: the desktop has not taken an ssh connection for 30 minutes" >&2
            return 1
        fi
        [ -n "$said" ] || { echo "rig: waiting for ssh to the desktop" >&2; said=1; }
        sleep "$wait"
        wait=$((wait * 2 > 300 ? 300 : wait * 2))
    done
    rm -rf "$lock"
    [ -z "$said" ] || echo "rig: ssh to the desktop is back after $((SECONDS - start)) s" >&2
}

# shellcheck disable=SC2029
rig_ssh() { connect && ssh "${ssh_options[@]}" "$host" "$@"; }
rig_scp() { connect && scp -q "${ssh_options[@]}" "$@"; }

# Runs a desktop PowerShell script with arguments, its output and error with carriage returns removed
# and passed on line by line as they come; returns its exit code.
rig_powershell() {
    rig_ssh powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$@" \
        2> >(LC_ALL=C sed -u $'s/\r//g' >&2) | LC_ALL=C sed -u $'s/\r//g'
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
