/* R18: pointers returned here alias still-live caller-owned string buffers. */
const char *priority_echo(char **s) {
    return *s;
}

const char *priority_swap(char **a, char **b) {
    char *old_a = *a;
    *a = *b;
    *b = old_a;
    return old_a;
}

void priority_swap_void(char **a, char **b) {
    char *old_a = *a;
    *a = *b;
    *b = old_a;
}

const char *priority_share(char **source, char **first, char **second) {
    char *old_source = *source;
    *first = old_source;
    *second = old_source;
    *source = "replaced";
    return old_source;
}
