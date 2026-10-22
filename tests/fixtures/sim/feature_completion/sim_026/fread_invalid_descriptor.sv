// SIM-026 A02: $fread into a memory from a closed descriptor is a fatal
// run-time error (S26-D2, industry-standard behaviour; IEEE 1800-2009
// 21.3.4.4 gives no result for a descriptor that is not open). The memory
// keeps its contents, the rest of the statement and the other processes do
// not run, and the final procedures do.
module tb;
  integer fd, c;
  logic [7:0] m[0:1];
  final $display("final c=%0d m0=%h m1=%h", c, m[0], m[1]);
  initial begin
    fd = $fopen("fread_closed.txt", "w");
    $fwrite(fd, "AB");
    $fclose(fd);
    m[0] = 8'h77;
    m[1] = 8'h88;
    c = 3;
    $display("before");
    c = $fread(m, fd, 0, 2);
    $display("not reached c=%0d", c);
    $finish;
  end
  initial #1 $display("other process");
endmodule
