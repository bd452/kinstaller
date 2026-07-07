/* glibc < 2.16 lacks getauxval; Rust std still references it at link time. */
unsigned long getauxval(unsigned long type) {
    (void)type;
    return 0;
}
