# CPython against the system: child processes, threads, sockets on loopback and the file system.
# Everything printed is the same on every run.
import os
import queue
import shutil
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import threading


def processes():
    out = subprocess.run([sys.executable, "-c", "import sys; print('child', sys.argv[1:])", "a", "b"],
                         capture_output=True, text=True)
    print("child said", out.stdout.strip(), "status", out.returncode)
    out = subprocess.run(["/bin/sh", "-c", "echo shell $0; exit 3", "zero"], capture_output=True, text=True)
    print("shell said", out.stdout.strip(), "status", out.returncode)
    piped = subprocess.run("printf 'b\\na\\nc\\n' | sort | tr a-z A-Z", shell=True, capture_output=True,
                           text=True)
    print("pipeline", piped.stdout.split())
    child = subprocess.Popen([sys.executable, "-c", "import sys\nfor line in sys.stdin: print(line.strip()[::-1], flush=True)"],
                             stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    out, _ = child.communicate("hello\nworld\n")
    print("reversed", out.split(), "status", child.returncode)
    sleeper = subprocess.Popen(["/bin/sleep", "60"])
    sleeper.send_signal(signal.SIGTERM)
    print("killed sleeper status", sleeper.wait())
    try:
        subprocess.run(["/no/such/program"])
    except FileNotFoundError as error:
        print("missing program", error.errno)
    pid = os.fork()
    if pid == 0:
        os._exit(7)
    _, status = os.waitpid(pid, 0)
    print("forked child exit", os.waitstatus_to_exitcode(status))
    r, w = os.pipe()
    pid = os.fork()
    if pid == 0:
        os.close(r)
        os.write(w, b"through a pipe")
        os._exit(0)
    os.close(w)
    print("pipe read", os.read(r, 100))
    os.waitpid(pid, 0)


def threads():
    total = 0
    lock = threading.Lock()

    def add(count):
        nonlocal total
        for _ in range(count):
            with lock:
                total += 1

    workers = [threading.Thread(target=add, args=(2000,)) for _ in range(8)]
    for worker in workers:
        worker.start()
    for worker in workers:
        worker.join()
    print("locked total", total)
    work = queue.Queue()
    results = queue.Queue()

    def square():
        while True:
            item = work.get()
            if item is None:
                return
            results.put(item * item)

    pool = [threading.Thread(target=square) for _ in range(4)]
    for thread in pool:
        thread.start()
    for item in range(100):
        work.put(item)
    for _ in pool:
        work.put(None)
    for thread in pool:
        thread.join()
    print("squares", sum(results.get() for _ in range(100)))
    condition = threading.Condition()
    order = []

    def waiter(name):
        with condition:
            condition.wait_for(lambda: len(order) >= 1)
            order.append(name)

    waiters = [threading.Thread(target=waiter, args=(n,)) for n in "abc"]
    for thread in waiters:
        thread.start()
    with condition:
        order.append("start")
        condition.notify_all()
    for thread in waiters:
        thread.join()
    print("woken", sorted(order))
    barrier = threading.Barrier(5)
    passed = []
    ready = [threading.Thread(target=lambda: passed.append(barrier.wait() >= 0)) for _ in range(5)]
    for thread in ready:
        thread.start()
    for thread in ready:
        thread.join()
    print("barrier passed", len(passed), all(passed))
    event = threading.Event()
    timer = threading.Timer(0.05, event.set)
    timer.start()
    print("timer fired", event.wait(30))


def sockets():
    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind(("127.0.0.1", 0))
    server.listen()
    port = server.getsockname()[1]

    def serve():
        for _ in range(3):
            connection, _ = server.accept()
            with connection:
                data = b""
                while not data.endswith(b"\n"):
                    chunk = connection.recv(4096)
                    if not chunk:
                        break
                    data += chunk
                connection.sendall(data.upper() * 100)

    thread = threading.Thread(target=serve)
    thread.start()
    for word in ("one", "two", "three"):
        with socket.create_connection(("127.0.0.1", port)) as client:
            client.sendall(word.encode() + b"\n")
            reply = b""
            while True:
                chunk = client.recv(65536)
                if not chunk:
                    break
                reply += chunk
            print("tcp reply", word, len(reply), reply[:6])
    thread.join()
    server.close()
    a = socket.socket(socket.AF_INET6, socket.SOCK_DGRAM)
    a.bind(("::1", 0))
    b = socket.socket(socket.AF_INET6, socket.SOCK_DGRAM)
    b.sendto(b"datagram", a.getsockname())
    data, sender = a.recvfrom(100)
    print("udp6", data, sender[0])
    left, right = socket.socketpair()
    left.sendall(b"pair")
    print("pair", right.recv(10))
    for s in (a, b, left, right):
        s.close()
    print("resolved localhost", sorted({info[4][0] for info in socket.getaddrinfo("localhost", 80, socket.AF_INET)}))


def files():
    root = tempfile.mkdtemp(prefix="wc")
    os.makedirs(os.path.join(root, "a", "b", "c"))
    for index in range(20):
        with open(os.path.join(root, "a", f"file{index:02}"), "w") as out:
            out.write("line\n" * index)
    with open(os.path.join(root, "a", "b", "big"), "wb") as out:
        out.write(os.urandom(1 << 20))
    os.symlink("a/file03", os.path.join(root, "link"))
    os.chmod(os.path.join(root, "a", "file05"), 0o600)
    listing = []
    for directory, subdirectories, names in os.walk(root):
        subdirectories.sort()
        relative = os.path.relpath(directory, root)
        listing.append((relative, sorted(names)))
    print("walk", listing)
    print("sizes", sum(os.path.getsize(os.path.join(root, "a", f"file{index:02}")) for index in range(20)))
    copy = shutil.copytree(os.path.join(root, "a"), os.path.join(root, "copy"))
    with open(os.path.join(root, "a", "b", "big"), "rb") as one, open(os.path.join(copy, "b", "big"), "rb") as two:
        print("copied same", one.read() == two.read())
    print("mode kept", oct(stat.S_IMODE(os.stat(os.path.join(copy, "file05")).st_mode)))
    os.rename(os.path.join(root, "copy"), os.path.join(root, "moved"))
    print("link reads", os.readlink(os.path.join(root, "link")), open(os.path.join(root, "link")).read().count("line"))
    with open(os.path.join(root, "appended"), "a+") as out:
        out.write("x" * 10)
        out.seek(0)
        print("appended", out.read())
    descriptor = os.open(os.path.join(root, "sparse"), os.O_CREAT | os.O_RDWR)
    os.pwrite(descriptor, b"end", 1 << 20)
    print("sparse size", os.fstat(descriptor).st_size, os.pread(descriptor, 4, 10))
    os.close(descriptor)
    try:
        os.rmdir(os.path.join(root, "a"))
    except OSError as error:
        print("rmdir full", error.errno)
    shutil.rmtree(root)
    print("removed", not os.path.exists(root))


processes()
threads()
sockets()
files()
print("done")
