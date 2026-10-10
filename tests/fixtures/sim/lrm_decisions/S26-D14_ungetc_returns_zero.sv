// Decision S26-D14: $ungetc returns 0 on success.
//
// IEEE 1800-2009 21.3.4.1 (SystemVerilog-1800-2009.txt L36816-36818):
//   "If an error occurs pushing a character onto a file descriptor, then code
//   is set to EOF. Otherwise, code is set to zero."
// IEEE 1364-2001 17.2.4.1 (Verilog-1364-2001.txt L18994-18995) says the
// same.
//
// llg follows the text. The C library ungetc returns the character, which
// llg returned before.
module tb;
  integer fd, a, u, b, n;
  initial begin
    fd = $fopen("s26_d14.txt", "w");
    $fwrite(fd, "ab");
    $fclose(fd);
    fd = $fopen("s26_d14.txt", "r");
    a = $fgetc(fd);
    u = $ungetc(122, fd);
    b = $fgetc(fd);
    n = $fgetc(fd);
    $fclose(fd);
    $display("a=%0d u=%0d b=%0d n=%0d", a, u, b, n);
    $finish;
  end
endmodule
