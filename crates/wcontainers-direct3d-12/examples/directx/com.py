# Writes the body of com.rs from the windows crate's *_Impl traits: python3 com.py 'Type:IInterface:Method,...' ...
# (each argument names an object, its interface and the methods objects.rs does; see the status file).
import re, sys, glob
W = glob.glob('/Users/thomaseaton/.cargo/registry/src/*/windows-0.62.2/src/Windows/Win32/Graphics')[0]
src = {m: open(f'{W}/{m}/mod.rs').read() for m in ['Direct3D12', 'Dxgi', 'Direct3D']}

def trait(name):
    for mod, s in src.items():
        m = re.search(r'^pub trait %s_Impl: ([\w:]+) \{(\}|\n(.*?)^\})' % name, s, re.S | re.M)
        if m:
            parent = None if 'IUnknownImpl' in m.group(1) else m.group(1).split('::')[-1][:-len('_Impl')]
            body = m.group(3) or ''
            return mod, parent, [l.strip() for l in body.splitlines() if l.strip().startswith('fn ')]
    raise SystemExit('no trait ' + name)

def qualify(sig, mod):
    sig = sig.replace('super::super::Foundation::', 'windows::Win32::Foundation::')
    sig = sig.replace('super::Dxgi::Common::', 'windows::Win32::Graphics::Dxgi::Common::')
    sig = sig.replace('super::Direct3D::', 'windows::Win32::Graphics::Direct3D::')
    sig = sig.replace('super::Dxgi::', 'windows::Win32::Graphics::Dxgi::')
    sig = sig.replace('super::super::System::', 'windows::Win32::System::')
    sig = sig.replace('super::super::Security::', 'windows::Win32::Security::')
    sig = re.sub(r'super::Common::', 'windows::Win32::Graphics::Dxgi::Common::', sig)
    sig = re.sub(r'super::Gdi::', 'windows::Win32::Graphics::Gdi::', sig)
    for name in re.findall(r'\b(I[A-Z0-9][A-Za-z0-9_]*|D3D12_[A-Z0-9_]+|DXGI_[A-Z0-9_]+|D3D_[A-Z0-9_]+)\b', sig):
        pass
    return sig

def unqualified_types(sig, mod):
    prefix = {'Direct3D12': 'windows::Win32::Graphics::Direct3D12::', 'Dxgi': 'windows::Win32::Graphics::Dxgi::', 'Direct3D': 'windows::Win32::Graphics::Direct3D::'}[mod]
    # Bare names in the trait's own module.
    def fix(m):
        t = m.group(0)
        return t
    return sig

def body(sig, kind):
    ret = sig.split('->')[-1].strip().rstrip(';') if '->' in sig else '()'
    name = re.match(r'fn (\w+)', sig).group(1)
    if ret.startswith('windows_core::Result'):
        return 'Err(self.unsupported("%s"))' % name
    if ret == 'windows_core::HRESULT':
        return 'self.unsupported("%s").code()' % name
    if ret == '()':
        return 'self.unsupported("%s");' % name
    return 'self.unsupported("%s");\n        Default::default()' % name

def args(sig):
    inner = sig[sig.index('(') + 1: sig.index(')')]
    parts = [p.strip() for p in inner.split(',')][1:]
    return ', '.join(p.split(':')[0] for p in parts if p)

out = []
for spec in sys.argv[1:]:
    ty, iface, hand = spec.split(':')
    hand = set(hand.split(',')) if hand else set()
    chain = []
    name = iface
    while name:
        mod, parent, methods = trait(name)
        chain.append((name, mod, methods))
        name = parent
    for name, mod, methods in chain:
        out.append('impl %s_Impl for %s_Impl {' % (name, ty))
        for sig in methods:
            sig = qualify(sig.rstrip(';'), mod)
            m = re.match(r'fn (\w+)', sig).group(1)
            if m in hand:
                out.append('    %s {\n        %s::%s(self, %s)\n    }' % (sig, ty, m, args(sig)))
                hand.discard(m)
            else:
                stub = re.sub(r'([(,] )(\w+): ', lambda m: m.group(1) + '_' + m.group(2) + ': ', sig)
                out.append('    %s {\n        %s\n    }' % (stub, body(sig, ty)))
        out.append('}\n')
    if hand:
        raise SystemExit('not in %s: %s' % (iface, hand))
print('\n'.join(out))
