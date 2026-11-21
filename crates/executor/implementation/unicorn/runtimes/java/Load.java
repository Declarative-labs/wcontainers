// A Java program against the system: threads sharing work and locks, garbage collection under
// load on a small heap, and safepoints: a thread in a compiled loop is stopped for each collection
// by the polling page its loop reads, which the safepoint makes unreadable, so the read faults.
// Everything printed is the same on every run.
import java.lang.management.GarbageCollectorMXBean;
import java.lang.management.ManagementFactory;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.locks.ReentrantLock;

public class Load {
    static long counter;

    static long collections() {
        long total = 0;
        for (GarbageCollectorMXBean bean : ManagementFactory.getGarbageCollectorMXBeans()) {
            total += Math.max(0, bean.getCollectionCount());
        }
        return total;
    }

    static void threads() throws Exception {
        ExecutorService pool = Executors.newFixedThreadPool(4);
        List<Future<Long>> results = new ArrayList<>();
        for (int task = 0; task < 16; task++) {
            final long start = task * 100_000L;
            results.add(pool.submit(() -> {
                long sum = 0;
                for (long n = start; n < start + 100_000; n++) {
                    sum += n % 13;
                }
                return sum;
            }));
        }
        long sum = 0;
        for (Future<Long> result : results) {
            sum += result.get();
        }
        System.out.println("pool sum " + sum);
        Object lock = new Object();
        ReentrantLock reentrant = new ReentrantLock();
        AtomicLong atomic = new AtomicLong();
        ConcurrentHashMap<Integer, Integer> map = new ConcurrentHashMap<>();
        CountDownLatch done = new CountDownLatch(8);
        for (int t = 0; t < 8; t++) {
            final int id = t;
            pool.submit(() -> {
                for (int i = 0; i < 20_000; i++) {
                    synchronized (lock) {
                        counter++;
                    }
                    reentrant.lock();
                    try {
                        atomic.incrementAndGet();
                    } finally {
                        reentrant.unlock();
                    }
                    map.merge(i % 100, 1, Integer::sum);
                }
                done.countDown();
                return id;
            });
        }
        done.await();
        System.out.println("synchronized " + counter + " locked " + atomic.get() + " map "
                + map.size() + " " + map.values().stream().mapToInt(Integer::intValue).sum());
        pool.shutdown();
        Thread waiter = new Thread(() -> {
            synchronized (lock) {
                try {
                    lock.wait();
                } catch (InterruptedException e) {
                    System.out.println("waiter interrupted");
                }
            }
        });
        waiter.start();
        waiter.interrupt();
        waiter.join();
    }

    static void garbage() throws Exception {
        long before = collections();
        // Threads allocate while others hold onto some of what they made, so collections stop
        // running threads at safepoints again and again.
        Thread[] makers = new Thread[4];
        long[] kept = new long[4];
        for (int t = 0; t < makers.length; t++) {
            final int id = t;
            makers[t] = new Thread(() -> {
                List<byte[]> held = new ArrayList<>();
                long checksum = 0;
                for (int i = 0; i < 20_000; i++) {
                    byte[] block = new byte[1024 + (i % 7) * 512];
                    block[0] = (byte) i;
                    checksum += block[0] + block.length;
                    if (i % 10 == 0) {
                        held.add(block);
                    }
                    if (held.size() > 500) {
                        held.subList(0, 250).clear();
                    }
                }
                kept[id] = checksum;
            });
            makers[t].start();
        }
        for (Thread maker : makers) {
            maker.join();
        }
        System.gc();
        long sum = 0;
        for (long value : kept) {
            sum += value;
        }
        System.out.println("allocated checksum " + sum);
        System.out.println("collected " + (collections() > before));
    }

    static volatile long progress;
    static volatile boolean stop;

    static void safepoints() throws Exception {
        long before = collections();
        // A loop that calls nothing, so compiled it reaches safepoints only through its poll.
        Thread spinner = new Thread(() -> {
            long i = 0;
            while (!stop) {
                i++;
                if ((i & 0xffff) == 0) {
                    progress = i;
                }
            }
        });
        spinner.start();
        // Long enough for the loop to be compiled, then a collection while it runs, three times.
        long wanted = 1L << 21;
        for (int k = 0; k < 3; k++) {
            while (progress < wanted) {
                Thread.sleep(1);
            }
            System.gc();
            wanted = progress + (1L << 18);
        }
        stop = true;
        spinner.join();
        System.out.println("spinner stopped for collections " + (collections() - before >= 3));
    }

    public static void main(String[] arguments) throws Exception {
        System.out.println("java " + Runtime.version().feature());
        threads();
        garbage();
        safepoints();
        System.out.println("done");
    }
}
