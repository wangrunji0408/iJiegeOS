void _start() {
    const char msg[] = "Hello from JiegeOS!\n";
    register long a0 __asm__("a0") = 1;
    register long a1 __asm__("a1") = (long)msg;
    register long a2 __asm__("a2") = sizeof(msg) - 1;
    register long a7 __asm__("a7") = 64;
    __asm__ volatile("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a7) : "memory");
    a0 = 0;
    a7 = 93;
    __asm__ volatile("ecall" : : "r"(a0), "r"(a7));
    __builtin_unreachable();
}
