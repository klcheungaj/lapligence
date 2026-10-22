// Decision S26-D2: $fread from a descriptor that is not open ends the run.
//
// IEEE 1800-2009 21.3.4.4 (SystemVerilog-1800-2009.txt L37090): "If an error
//   occurs reading from the file, then code is set to zero." 21.3.1
//   (L36633-36634) gives no result for a file descriptor that is not open.
//
// llg follows industry-standard behaviour: $fread from a closed, never-opened,
// zero, unknown or multichannel descriptor is a fatal run-time error. The
// statement and the other processes do not continue, and the final
// procedures run. The other input functions return their error code
// instead (see S26-D2_invalid_descriptor_input).
module tb;
  integer fd, c;
  logic [7:0] a = 8'd7;
  final $display("final a=%0d c=%0d", a, c);
  initial begin
    fd = $fopen("s26_d2_fread.txt", "w");
    $fwrite(fd, "AB");
    $fclose(fd);
    c = 5;
    $display("before");
    c = $fread(a, fd);
    $display("not reached c=%0d", c);
    $finish;
  end
  initial #1 $display("other process");
endmodule
