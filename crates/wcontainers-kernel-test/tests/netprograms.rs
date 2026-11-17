//! The image's network programs: a server and a client inside the container talking over
//! loopback, and the tools that show the interfaces and routes, compared with the reference in
//! output, exit status and the files left behind.

use wcontainers_kernel_test::busybox::compare;

fn script(text: &str) {
    compare(&["sh", "-c", text]);
}

#[test]
fn a_server_and_a_client_inside() {
    script(
        "nc -l -p 8080 -e cat & sleep 0.5; echo hello | nc -w 2 127.0.0.1 8080; echo status $?; \
         wait",
    );
    script(
        "printf 'HTTP/1.0 200 OK\\r\\nContent-Length: 3\\r\\n\\r\\nhi\\n' | nc -l -p 8081 > /tmp/asked & \
         sleep 0.5; wget -q -O - http://127.0.0.1:8081/page; echo status $?; wait; \
         head -n 1 /tmp/asked",
    );
}

#[test]
fn interfaces_and_routes() {
    script("ip addr show lo; ip link show lo; ip route | head -n 1; ip -6 addr show lo");
}
