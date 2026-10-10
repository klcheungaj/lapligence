// Decision S26-D2: input functions on a descriptor that is not open for
// reading return their error codes; $fscanf and $feof return EOF (-1).
//
// IEEE 1800-2009 21.3.4.3 (SystemVerilog-1800-2009.txt L37024-37025):
//   "If the input ends before the first matching failure or conversion, EOF
//   (-1) is returned."
// 21.3.8 (L37231-37232): $feof "returns a nonzero value when EOF has
//   previously been detected reading the input file fd. It returns zero
//   otherwise."
//
// The text gives no result for a closed or never-opened descriptor. llg
// treats such a descriptor as input that has already ended: $fgetc, $ungetc
// and $fscanf return EOF, $fgets and $fread return 0 (their error code), and
// $feof returns -1 (nonzero). Destinations are left unchanged.
module tb;
  integer fd, c, g, e;
  logic [7:0] a;
  initial begin
    fd = $fopen("s26_d2.txt", "w");
    $fwrite(fd, "5\n");
    $fclose(fd);
    fd = $fopen("s26_d2.txt", "r");
    $fclose(fd);
    a = 8'd9;
    c = $fscanf(fd, "%d", a);
    g = $fgetc(fd);
    e = $feof(fd);
    $display("fscanf=%0d fgetc=%0d feof=%0d a=%0d", c, g, e, a);
    $finish;
  end
endmodule
