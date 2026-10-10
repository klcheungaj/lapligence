// Decision S26-D1: selector expressions of $fscanf/$sscanf destinations are
// evaluated once, when the call starts.
//
// IEEE 1800-2009 21.3.4.3 (SystemVerilog-1800-2009.txt L36880-36881):
//   "A conversion specification directs the conversion of the next input
//   field; the result is placed in the variable specified in the
//   corresponding argument"
//
// The text does not say when an index of a destination is evaluated relative
// to earlier assignments of the same call. llg evaluates every destination's
// index and select expressions when the call starts, as for the arguments of
// any function call (13.5.1), so `a[i]` names a[0] although the first
// conversion assigns i = 2 first.
module tb;
  integer fd, c, i;
  int a[4];
  initial begin
    fd = $fopen("s26_d1.txt", "w");
    $fwrite(fd, "2 7\n");
    $fclose(fd);
    a = '{0, 0, 0, 0};
    i = 0;
    fd = $fopen("s26_d1.txt", "r");
    c = $fscanf(fd, "%d %d", i, a[i]);
    $fclose(fd);
    $display("c=%0d i=%0d a=%0d %0d %0d %0d", c, i, a[0], a[1], a[2], a[3]);
    $finish;
  end
endmodule
