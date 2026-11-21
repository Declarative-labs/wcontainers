// A program whose zeroed data is larger than the small ceiling the embedder's tests give.

char data[128 << 20];

int main(int argc, char **argv) {
    (void)argv;
    data[argc] = 1;
    return data[sizeof data - 1];
}
