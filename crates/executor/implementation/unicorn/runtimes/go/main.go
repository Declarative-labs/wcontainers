// A Go program against the system: the runtime's scheduler with many goroutines on several
// threads, its network poller on loopback, its signal handling, and its use of futex and clone.
// Everything printed is the same on every run.
package main

import (
	"bufio"
	"fmt"
	"io"
	"net"
	"os"
	"os/exec"
	"os/signal"
	"runtime"
	"sort"
	"strings"
	"sync"
	"sync/atomic"
	"syscall"
	"time"
)

func goroutines() {
	// Many goroutines passing values along a ring of channels.
	const count = 1000
	first := make(chan int)
	in := first
	for i := 0; i < count; i++ {
		out := make(chan int)
		go func(in, out chan int) { out <- 1 + <-in }(in, out)
		in = out
	}
	first <- 0
	fmt.Println("ring of", count, "goroutines gave", <-in)
	// Goroutines sharing a mutex and a wait group, blocking and waking through futexes.
	var mutex sync.Mutex
	var group sync.WaitGroup
	total := 0
	for i := 0; i < 64; i++ {
		group.Add(1)
		go func() {
			defer group.Done()
			for j := 0; j < 500; j++ {
				mutex.Lock()
				total++
				mutex.Unlock()
				if j%100 == 0 {
					runtime.Gosched()
				}
			}
		}()
	}
	group.Wait()
	fmt.Println("mutex total", total)
	// Work spread over the threads the runtime starts (clone) for GOMAXPROCS.
	var sum atomic.Int64
	results := make(chan int64, 8)
	for i := 0; i < 8; i++ {
		go func(start int64) {
			var local int64
			for n := start; n < start+200000; n++ {
				local += n % 7
			}
			sum.Add(local)
			results <- local
		}(int64(i) * 200000)
	}
	for i := 0; i < 8; i++ {
		<-results
	}
	fmt.Println("spread sum", sum.Load())
	condition := sync.NewCond(&mutex)
	ready := 0
	for i := 0; i < 10; i++ {
		go func() {
			mutex.Lock()
			ready++
			condition.Broadcast()
			mutex.Unlock()
		}()
	}
	mutex.Lock()
	for ready < 10 {
		condition.Wait()
	}
	mutex.Unlock()
	fmt.Println("condition saw", ready)
	timer := time.NewTimer(20 * time.Millisecond)
	ticker := time.NewTicker(2 * time.Millisecond)
	ticks := 0
	for done := false; !done; {
		select {
		case <-ticker.C:
			ticks++
		case <-timer.C:
			done = true
		}
	}
	ticker.Stop()
	fmt.Println("ticked before the timer", ticks > 0)
}

func network() {
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		panic(err)
	}
	go func() {
		for {
			connection, err := listener.Accept()
			if err != nil {
				return
			}
			go func(connection net.Conn) {
				defer connection.Close()
				reader := bufio.NewReader(connection)
				for {
					line, err := reader.ReadString('\n')
					if err != nil {
						return
					}
					fmt.Fprint(connection, strings.ToUpper(line))
				}
			}(connection)
		}
	}()
	var group sync.WaitGroup
	answers := make([]string, 50)
	for i := 0; i < 50; i++ {
		group.Add(1)
		go func(i int) {
			defer group.Done()
			connection, err := net.Dial("tcp", listener.Addr().String())
			if err != nil {
				panic(err)
			}
			defer connection.Close()
			reader := bufio.NewReader(connection)
			var replies []string
			for j := 0; j < 3; j++ {
				fmt.Fprintf(connection, "client %d line %d\n", i, j)
				reply, err := reader.ReadString('\n')
				if err != nil {
					panic(err)
				}
				replies = append(replies, strings.TrimSpace(reply))
			}
			answers[i] = strings.Join(replies, ",")
		}(i)
	}
	group.Wait()
	listener.Close()
	fmt.Println("tcp clients", len(answers), answers[0], answers[49])
	// A large transfer through the poller, with deadlines.
	listener, _ = net.Listen("tcp6", "[::1]:0")
	go func() {
		connection, _ := listener.Accept()
		connection.Write(make([]byte, 4<<20))
		connection.Close()
	}()
	connection, err := net.Dial("tcp6", listener.Addr().String())
	if err != nil {
		panic(err)
	}
	connection.SetReadDeadline(time.Now().Add(time.Hour))
	n, _ := io.Copy(io.Discard, connection)
	fmt.Println("received over ipv6", n)
	connection.SetReadDeadline(time.Now().Add(-time.Second))
	_, err = connection.Read(make([]byte, 1))
	fmt.Println("past deadline is a timeout", os.IsTimeout(err))
	connection.Close()
	listener.Close()
	packets, _ := net.ListenPacket("udp", "127.0.0.1:0")
	sender, _ := net.Dial("udp", packets.LocalAddr().String())
	sender.Write([]byte("datagram"))
	buffer := make([]byte, 100)
	packets.SetReadDeadline(time.Now().Add(time.Hour))
	got, _, _ := packets.ReadFrom(buffer)
	fmt.Println("udp", string(buffer[:got]))
	sender.Close()
	packets.Close()
}

func signals() {
	caught := make(chan os.Signal, 4)
	signal.Notify(caught, syscall.SIGUSR1, syscall.SIGUSR2, syscall.SIGCHLD)
	syscall.Kill(os.Getpid(), syscall.SIGUSR1)
	fmt.Println("caught", <-caught)
	process, _ := os.FindProcess(os.Getpid())
	process.Signal(syscall.SIGUSR2)
	fmt.Println("caught", <-caught)
	output, err := exec.Command("/bin/sh", "-c", "echo child of go; exit 2").Output()
	exit, _ := err.(*exec.ExitError)
	fmt.Println("child said", strings.TrimSpace(string(output)), "status", exit.ExitCode())
	select {
	case got := <-caught:
		fmt.Println("caught", got)
	case <-time.After(10 * time.Second):
		fmt.Println("no SIGCHLD")
	}
	signal.Stop(caught)
	signal.Ignore(syscall.SIGHUP)
	syscall.Kill(os.Getpid(), syscall.SIGHUP)
	fmt.Println("ignored SIGHUP")
	// A nil dereference becomes a panic through the runtime's SIGSEGV handler.
	func() {
		defer func() { fmt.Println("recovered:", recover()) }()
		var pointer *int
		fmt.Println(*pointer)
	}()
	command := exec.Command("/bin/sleep", "60")
	command.Start()
	command.Process.Signal(syscall.SIGTERM)
	err = command.Wait()
	status := err.(*exec.ExitError).Sys().(syscall.WaitStatus)
	fmt.Println("sleeper ended by", status.Signal())
}

func files() {
	directory, _ := os.MkdirTemp("", "go")
	for i := 0; i < 10; i++ {
		os.WriteFile(fmt.Sprintf("%s/file%d", directory, i), []byte(strings.Repeat("x", i*100)), 0o644)
	}
	entries, _ := os.ReadDir(directory)
	var names []string
	size := int64(0)
	for _, entry := range entries {
		info, _ := entry.Info()
		names = append(names, entry.Name())
		size += info.Size()
	}
	sort.Strings(names)
	fmt.Println("files", names, size)
	os.RemoveAll(directory)
}

func main() {
	runtime.GOMAXPROCS(4)
	fmt.Println("go", runtime.Version(), runtime.GOOS, runtime.GOARCH, "threads", runtime.GOMAXPROCS(0))
	goroutines()
	network()
	signals()
	files()
	fmt.Println("done")
}
