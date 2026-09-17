/* Static-review counterexample; NOT EXECUTED. */
/* Pair with Dpi_String_Alias.sv using the project's DPI library option.
 * The returned string aliases an input buffer; it is not freed by this function.
 */
const char *review_echo(const char **s) {
    return *s;
}
