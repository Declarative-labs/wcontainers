//! The image's shell running pipelines, subshells, background jobs, traps and waits, compared with
//! the reference in output, exit status and the files left behind.

use wcontainers_kernel_test::busybox::compare;

fn script(text: &str) {
    compare(&["sh", "-c", text]);
}

#[test]
fn pipelines() {
    script("cat /etc/passwd | grep root | cut -d: -f1");
    script("seq 1 2000 | sort -rn | head -n 3; echo status $?");
    script("seq 1 100000 | wc -l");
    script("yes | head -n 5");
    script("false | true; echo $?; true | false; echo $?");
    script("echo a b c | tr ' ' '\\n' | sort -r | tr '\\n' ,");
    script("ls /bin | wc -l > /tmp/count; cat /tmp/count");
}

#[test]
fn processes() {
    script("(echo sub; exit 3); echo after $?");
    script("x=$(echo inner | tr a-z A-Z); echo $x");
    script("sleep 0.2 & echo started; wait; echo waited $?");
    script("sh -c 'exit 7' & wait $!; echo status $?");
    // Which background job prints first is a race in the reference.
    script("{ for i in 1 2 3; do (echo job $i) & done; wait; } | sort; echo all");
    script("trap 'echo got usr1' USR1; kill -USR1 $$; echo after trap");
    script("sleep 5 & kill $!; wait $!; echo killed $?");
    script("exec 3>/tmp/fd3; echo to3 >&3; exec 3>&-; cat /tmp/fd3");
    script("ulimit -n; umask; cd /tmp && pwd");
    script("echo $$ $PPID | wc -w; read x < /etc/hostname; echo $x");
}
