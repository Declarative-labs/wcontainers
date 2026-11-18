# Makes this kernel's configuration from the reference's: options whose features it does not
# build are "not set", and the options that depend on them are left out, as Kconfig writes them.
import re, sys
off = {"AIO": [], "IP_SCTP": ["SCTP_"], "VSOCKETS": ["VSOCKETS_", "VIRTIO_VSOCKETS", "HYPERV_VSOCKETS"],
       "CRYPTO_USER_API": ["CRYPTO_USER_API_"]}
dependent = [p for subs in off.values() for p in subs]
out = []
for line in open(sys.argv[1]):
    m = re.match(r'^(?:# )?CONFIG_([A-Z0-9_]+)[= ]', line)
    name = m.group(1) if m else None
    if name in off:
        out.append(f"# CONFIG_{name} is not set\n")
        continue
    if name and any(name.startswith(p) for p in dependent):
        continue
    line = line.replace("Linux/arm64 7.0.14 Kernel Configuration", "Linux/arm64 7.0.0 Kernel Configuration")
    line = line.replace('CONFIG_LOCALVERSION="-linuxkit"', 'CONFIG_LOCALVERSION="-wcontainers"')
    out.append(line)
open(sys.argv[2], "w").write("".join(out))
