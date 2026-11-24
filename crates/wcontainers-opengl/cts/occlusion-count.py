"""Exact occlusion counts (GL_SAMPLES_PASSED) of a set of draws whose covered samples are known,
in a legacy OpenGL context on Windows, for each driver: the system's OpenGL and Mesa's drivers.
Each draw covers whole pixels (edges on pixel edges, points centred on pixel corners), so every
implementation gives the same count. Prints each case's count per driver and exits 1 if any count
differs from the known one.

    python occlusion-count.py <Mesa's bin folder> <driver> [<driver> ...]

A driver is `system` or a Gallium driver name (GALLIUM_DRIVER). Uses only the standard library.
"""

import ctypes
import os
import subprocess
import sys
from ctypes import wintypes

SYSTEM = "system"
ONE = "--one"

# The window's side in pixels; normalized coordinates map one pixel to this much.
SIDE = 64
HALF_SIDE = SIDE / 2

# Windows and WGL values (WinUser.h, wingdi.h).
WS_OVERLAPPEDWINDOW = 0x00CF0000
PFD_DOUBLEBUFFER = 0x00000001
PFD_DRAW_TO_WINDOW = 0x00000004
PFD_SUPPORT_OPENGL = 0x00000020
PFD_TYPE_RGBA = 0
COLOR_BITS = 32
DEPTH_BITS = 24
STENCIL_BITS = 8
SW_SHOWNOACTIVATE = 4

# OpenGL values (GL/gl.h, glext.h).
GL_POINTS = 0x0000
GL_TRIANGLES = 0x0004
GL_TRIANGLE_STRIP = 0x0005
GL_TRIANGLE_FAN = 0x0006
GL_QUADS = 0x0007
GL_LESS = 0x0201
GL_DEPTH_TEST = 0x0B71
GL_DEPTH_BUFFER_BIT = 0x00000100
GL_COLOR_BUFFER_BIT = 0x00004000
GL_VERSION = 0x1F02
GL_RENDERER = 0x1F01
GL_SAMPLES_PASSED = 0x8914
GL_QUERY_RESULT = 0x8866
GL_QUERY_RESULT_AVAILABLE = 0x8867
GL_TRUE = 1
GL_FALSE = 0


class PixelFormat(ctypes.Structure):
    _fields_ = [
        ("nSize", wintypes.WORD),
        ("nVersion", wintypes.WORD),
        ("dwFlags", wintypes.DWORD),
        ("iPixelType", ctypes.c_ubyte),
        ("cColorBits", ctypes.c_ubyte),
        ("cRedBits", ctypes.c_ubyte),
        ("cRedShift", ctypes.c_ubyte),
        ("cGreenBits", ctypes.c_ubyte),
        ("cGreenShift", ctypes.c_ubyte),
        ("cBlueBits", ctypes.c_ubyte),
        ("cBlueShift", ctypes.c_ubyte),
        ("cAlphaBits", ctypes.c_ubyte),
        ("cAlphaShift", ctypes.c_ubyte),
        ("cAccumBits", ctypes.c_ubyte),
        ("cAccumRedBits", ctypes.c_ubyte),
        ("cAccumGreenBits", ctypes.c_ubyte),
        ("cAccumBlueBits", ctypes.c_ubyte),
        ("cAccumAlphaBits", ctypes.c_ubyte),
        ("cDepthBits", ctypes.c_ubyte),
        ("cStencilBits", ctypes.c_ubyte),
        ("cAuxBuffers", ctypes.c_ubyte),
        ("iLayerType", ctypes.c_ubyte),
        ("bReserved", ctypes.c_ubyte),
        ("dwLayerMask", wintypes.DWORD),
        ("dwVisibleMask", wintypes.DWORD),
        ("dwDamageMask", wintypes.DWORD),
    ]


WNDPROC = ctypes.WINFUNCTYPE(
    ctypes.c_ssize_t, wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM
)


class WindowClass(ctypes.Structure):
    _fields_ = [
        ("style", wintypes.UINT),
        ("lpfnWndProc", WNDPROC),
        ("cbClsExtra", ctypes.c_int),
        ("cbWndExtra", ctypes.c_int),
        ("hInstance", wintypes.HINSTANCE),
        ("hIcon", wintypes.HICON),
        ("hCursor", wintypes.HANDLE),
        ("hbrBackground", wintypes.HBRUSH),
        ("lpszMenuName", wintypes.LPCWSTR),
        ("lpszClassName", wintypes.LPCWSTR),
    ]


class Gl:
    """A current legacy context on a window of SIDE pixels, through `library`."""

    def __init__(self, library):
        # Loaded first, so that gdi32's pixel format calls use this opengl32.dll.
        self.gl = ctypes.WinDLL(library)
        user32 = ctypes.WinDLL("user32")
        gdi32 = ctypes.WinDLL("gdi32")
        user32.DefWindowProcW.argtypes = [
            wintypes.HWND,
            wintypes.UINT,
            wintypes.WPARAM,
            wintypes.LPARAM,
        ]
        user32.DefWindowProcW.restype = ctypes.c_ssize_t
        self.procedure = WNDPROC(lambda *a: user32.DefWindowProcW(*a))
        name = "occlusion-count"
        window_class = WindowClass(lpfnWndProc=self.procedure, lpszClassName=name)
        user32.RegisterClassW(ctypes.byref(window_class))
        user32.CreateWindowExW.restype = wintypes.HWND
        user32.CreateWindowExW.argtypes = [wintypes.DWORD, wintypes.LPCWSTR, wintypes.LPCWSTR]
        user32.CreateWindowExW.argtypes += [wintypes.DWORD] + [ctypes.c_int] * 4
        user32.CreateWindowExW.argtypes += [wintypes.HWND, wintypes.HMENU]
        user32.CreateWindowExW.argtypes += [wintypes.HINSTANCE, wintypes.LPVOID]
        window = user32.CreateWindowExW(
            0, name, name, WS_OVERLAPPEDWINDOW, 0, 0, SIDE * 4, SIDE * 4, None, None, None, None
        )
        user32.ShowWindow(window, SW_SHOWNOACTIVATE)
        user32.GetDC.restype = wintypes.HDC
        user32.GetDC.argtypes = [wintypes.HWND]
        dc = user32.GetDC(window)
        pfd = PixelFormat(
            nSize=ctypes.sizeof(PixelFormat),
            nVersion=1,
            dwFlags=PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
            iPixelType=PFD_TYPE_RGBA,
            cColorBits=COLOR_BITS,
            cDepthBits=DEPTH_BITS,
            cStencilBits=STENCIL_BITS,
        )
        gdi32.ChoosePixelFormat.argtypes = [wintypes.HDC, ctypes.POINTER(PixelFormat)]
        gdi32.SetPixelFormat.argtypes = [wintypes.HDC, ctypes.c_int, ctypes.POINTER(PixelFormat)]
        chosen = gdi32.ChoosePixelFormat(dc, ctypes.byref(pfd))
        if not chosen or not gdi32.SetPixelFormat(dc, chosen, ctypes.byref(pfd)):
            raise SystemExit("no pixel format")
        self.gl.wglCreateContext.restype = wintypes.HGLRC
        self.gl.wglCreateContext.argtypes = [wintypes.HDC]
        self.gl.wglMakeCurrent.argtypes = [wintypes.HDC, wintypes.HGLRC]
        context = self.gl.wglCreateContext(dc)
        if not context or not self.gl.wglMakeCurrent(dc, context):
            raise SystemExit("no context")
        self.gl.wglGetProcAddress.restype = ctypes.c_void_p
        self.gl.wglGetProcAddress.argtypes = [ctypes.c_char_p]
        self.gl.glGetString.restype = ctypes.c_char_p
        self.gl.glVertex3f.argtypes = [ctypes.c_float] * 3
        self.gl.glClearDepth.argtypes = [ctypes.c_double]
        self.gl.glPointSize.argtypes = [ctypes.c_float]
        query_calls = {
            "glGenQueries": [ctypes.c_int, ctypes.POINTER(ctypes.c_uint)],
            "glBeginQuery": [ctypes.c_uint, ctypes.c_uint],
            "glEndQuery": [ctypes.c_uint],
            "glGetQueryObjectuiv": [ctypes.c_uint, ctypes.c_uint, ctypes.POINTER(ctypes.c_uint)],
        }
        for f, arguments in query_calls.items():
            address = self.gl.wglGetProcAddress(f.encode())
            if not address:
                raise SystemExit(f"no {f}")
            setattr(self, f, ctypes.WINFUNCTYPE(None, *arguments)(address))
        self.gl.glViewport(0, 0, SIDE, SIDE)

    def describe(self):
        version = self.gl.glGetString(GL_VERSION).decode()
        return f"{self.gl.glGetString(GL_RENDERER).decode()}, {version}"

    def vertex(self, x, y, z):
        self.gl.glVertex3f(x / HALF_SIDE - 1, y / HALF_SIDE - 1, z)

    def rect(self, x0, y0, x1, y1, z=0.0, mode=GL_TRIANGLES):
        """The rectangle of pixels x0..x1, y0..y1 at depth z, as `mode` draws it."""
        corners = {
            GL_TRIANGLES: [(x0, y0), (x1, y0), (x1, y1), (x0, y0), (x1, y1), (x0, y1)],
            GL_TRIANGLE_FAN: [(x0, y0), (x1, y0), (x1, y1), (x0, y1)],
            GL_TRIANGLE_STRIP: [(x0, y0), (x1, y0), (x0, y1), (x1, y1)],
            GL_QUADS: [(x0, y0), (x1, y0), (x1, y1), (x0, y1)],
        }[mode]
        self.gl.glBegin(mode)
        for x, y in corners:
            self.vertex(x, y, z)
        self.gl.glEnd()

    def points(self, size, corners):
        """Points of `size` pixels centred on pixel corners."""
        self.gl.glPointSize(size)
        self.gl.glBegin(GL_POINTS)
        for x, y in corners:
            self.vertex(x, y, 0.0)
        self.gl.glEnd()

    def counted(self, draw):
        """The samples `draw`'s draws passed, counted by one query."""
        query = ctypes.c_uint()
        self.glGenQueries(1, ctypes.byref(query))
        self.glBeginQuery(GL_SAMPLES_PASSED, query.value)
        draw()
        self.glEndQuery(GL_SAMPLES_PASSED)
        available = ctypes.c_uint(GL_FALSE)
        while available.value != GL_TRUE:
            self.glGetQueryObjectuiv(query.value, GL_QUERY_RESULT_AVAILABLE, ctypes.byref(available))
        count = ctypes.c_uint()
        self.glGetQueryObjectuiv(query.value, GL_QUERY_RESULT, ctypes.byref(count))
        return count.value

    def fresh(self, depth_test):
        self.gl.glClearDepth(1.0)
        self.gl.glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT)
        (self.gl.glEnable if depth_test else self.gl.glDisable)(GL_DEPTH_TEST)
        self.gl.glDepthFunc(GL_LESS)


def cases(g):
    """Each case's name, known count and the count `g` gives."""

    def plain():
        g.fresh(False)
        return g.counted(lambda: g.rect(8, 8, 40, 24))

    def with_mode(mode):
        def case():
            g.fresh(False)
            return g.counted(lambda: g.rect(8, 8, 40, 24, mode=mode))

        return case

    def two_draws():
        g.fresh(False)
        return g.counted(lambda: (g.rect(0, 0, 10, 10), g.rect(20, 20, 30, 40)))

    def behind():
        g.fresh(True)
        g.rect(0, 0, 32, 64, z=-0.5)
        return g.counted(lambda: g.rect(16, 16, 48, 48))

    def written_then_tested():
        # The second draw meets the depth the first wrote, and fails the less test.
        g.fresh(True)
        return g.counted(lambda: (g.rect(0, 0, 32, 32), g.rect(0, 0, 32, 32)))

    def cleared_between():
        def draws():
            g.rect(0, 0, 10, 10)
            g.gl.glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT)
            g.rect(0, 0, 5, 5)

        g.fresh(True)
        return g.counted(draws)

    def empty():
        g.fresh(False)
        return g.counted(lambda: None)

    def many():
        g.fresh(False)
        return g.counted(lambda: [g.rect(3, 3, 4, 4) for _ in range(100)])

    def points():
        g.fresh(False)
        return g.counted(lambda: g.points(4.0, [(10, 10), (30, 40)]))

    area = 32 * 16
    return [
        ("triangles", area, plain()),
        ("fan", area, with_mode(GL_TRIANGLE_FAN)()),
        ("strip", area, with_mode(GL_TRIANGLE_STRIP)()),
        ("quads", area, with_mode(GL_QUADS)()),
        ("two draws", 10 * 10 + 10 * 20, two_draws()),
        ("half behind a nearer draw", 16 * 32, behind()),
        ("depth written then tested", 32 * 32, written_then_tested()),
        ("cleared between", 10 * 10 + 5 * 5, cleared_between()),
        ("no draw", 0, empty()),
        ("one pixel a hundred times", 100, many()),
        ("points of 4 pixels", 2 * 4 * 4, points()),
    ]


def one(library):
    g = Gl(library)
    print(f"renderer: {g.describe()}")
    for name, known, got in cases(g):
        print(f"case: {name}: {known} {got}")


def main():
    if len(sys.argv) > 2 and sys.argv[1] == ONE:
        one(sys.argv[2])
        return
    mesa_bin, drivers = sys.argv[1], sys.argv[2:]
    failed = False
    for driver in drivers:
        environment = dict(os.environ)
        if driver == SYSTEM:
            library = "opengl32.dll"
        else:
            library = os.path.join(mesa_bin, "opengl32.dll")
            environment["GALLIUM_DRIVER"] = driver
        run = subprocess.run(
            [sys.executable, "-I", __file__, ONE, library],
            env=environment,
            capture_output=True,
            text=True,
        )
        print(f"== {driver} (exit {run.returncode})")
        print(run.stdout + run.stderr)
        lines = [l for l in run.stdout.splitlines() if l.startswith("case: ")]
        counts = [l.rsplit(" ", 2)[1:] for l in lines]
        wrong = [l for l, (known, got) in zip(lines, counts) if known != got]
        for l in wrong:
            print(f"WRONG on {driver}: {l}")
        failed |= run.returncode != 0 or not lines or bool(wrong)
    print("OCCLUSION-COUNT " + ("FAIL" if failed else "PASS"))
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
