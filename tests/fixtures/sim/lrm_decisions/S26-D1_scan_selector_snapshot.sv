// Decision S26-D1: the selector expressions of a $fscanf/$sscanf destination
// are evaluated when the conversion result is assigned to it, after the
// destinations before it have been assigned.
//
// IEEE 1800-2009 21.3.4.3 (SystemVerilog-1800-2009.txt L36880-36881):
//   "A conversion specification directs the conversion of the next input
//   field; the result is placed in the variable specified in the
//   corresponding argument"
//
// The text does not say when an index of a destination is evaluated relative
// to earlier assignments of the same call. llg follows industry-standard
// behaviour: the results are placed in argument order, and each destination
// is selected when its result is placed, so `a[i]` names a[2] after the
// first conversion assigns i = 2. A destination whose selectors read no
// earlier destination behaves the same either way.
module tb;
  integer fd, c, i, j;
  int a[4];
  int b[3][3];
  logic [7:0] v;
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
    i = 0;
    j = 0;
    b = '{default: 0};
    c = $sscanf("1 2 9", "%d %d %d", i, j, b[i][j]);
    $display("c=%0d b[1][2]=%0d b[0][0]=%0d", c, b[1][2], b[0][0]);
    i = 0;
    v = 0;
    c = $sscanf("3 1 4 11", "%d %b %d %b", i, v[i], j, v[j+:2]);
    $display("c=%0d v=%b", c, v);
    $finish;
  end
endmodule
