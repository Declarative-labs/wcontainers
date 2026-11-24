# overcommit.sh: boots this machine's own kernel in a 512 MiB KVM guest whose init is the overcommit probe, so the guest's
# vm.overcommit_memory changes and the machine's does not.
set -u
here=$(pwd)
out=/work/target/rprim-overcommit; rm -rf $out; mkdir -p $out
echo "machine's own overcommit_memory: $(cat /proc/sys/vm/overcommit_memory)"
( cd crates/shared-memory/checks/overcommit && CARGO_TARGET_DIR=$out/build cargo build -q --release --target aarch64-unknown-linux-musl 2>&1 | tail -20 )
bin=$out/build/aarch64-unknown-linux-musl/release/overcommit-probe
file $bin
python3 - $bin $out/initramfs.cpio <<'PY'
import sys, os
binary, dest = sys.argv[1:]
def entry(name, mode, data=b"", rdev=(0, 0)):
    head = "070701" + "".join("%08x" % v for v in [0, mode, 0, 0, 1, 0, len(data), 0, 0, rdev[0], rdev[1], len(name) + 1, 0])
    b = head.encode() + name.encode() + b"\0"
    b += b"\0" * (-len(b) % 4) + data
    return b + b"\0" * (-len(b) % 4)
blob = entry("dev", 0o40755) + entry("dev/console", 0o20600, rdev=(5, 1)) + entry("proc", 0o40755)
blob += entry("init", 0o100755, open(binary, "rb").read()) + entry("TRAILER!!!", 0)
open(dest, "wb").write(blob)
PY
sudo cp /boot/vmlinuz-$(uname -r) $out/vmlinuz && sudo chown $(id -u) $out/vmlinuz
gunzip -c $out/vmlinuz > $out/vmlinuz.efi; file $out/vmlinuz.efi
q=$(nix build --no-link --print-out-paths nixpkgs#qemu_kvm 2>&1 | tail -1); echo "qemu $q"
timeout 600 $q/bin/qemu-system-aarch64 -M virt -cpu host -accel kvm -smp 2 -m 512 -nographic -no-reboot \
  -kernel $out/vmlinuz.efi -initrd $out/initramfs.cpio -append "console=ttyAMA0 rdinit=/init panic=-1 loglevel=4" 2>&1 \
  | grep -vE "^\s*$" | tail -60
echo "qemu status ${PIPESTATUS[0]}"
echo "machine's own overcommit_memory after: $(cat /proc/sys/vm/overcommit_memory)"
rm -rf $out
