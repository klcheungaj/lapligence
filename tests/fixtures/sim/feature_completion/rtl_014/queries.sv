// IEEE 1800-2009 20.6-20.7: array queries over mixed packed/unpacked fixed
// arrays, selected rows, formals and descriptor-backed storage. A dimension
// outside the queried type yields X.
module tb;
  logic [7:0][3:0] m [-1:2][5:3];
  bit s [0:0];
  bit [15:0] big [0:1][70000:1];
  integer d;

  function automatic int formal_size(input int a [3:-4]);
    return $size(a) * 100 + $left(a) * 10 + $increment(a);
  endfunction

  initial begin
    automatic int f [3:-4];
    $display("%0d %0d %0d %0d", $dimensions(m), $unpacked_dimensions(m), $size(m), $size(m, 2));
    $display("%0d %0d %0d %0d %0d", $left(m, 1), $right(m, 2), $low(m, 3), $high(m, 4),
             $increment(m, 2));
    $display("%0d %0d", $size(m[0]), $left(m[0]));
    $display("%0d %0d", $size(m[0][4]), $left(m[0][4], 1));
    for (d = 1; d <= 5; d++)
      $display("d=%0d size=%0d left=%0d inc=%0d", d, $size(m, d), $left(m, d), $increment(m, d));
    d = 'x;
    $display("unknown=%0d", $size(m, d));
    $display("%0d %0d %0d", $size(s), $increment(s), $bits(m));
    $display("%0d %0d %0d %0d", $size(big, 2), $left(big[1]), $right(big, 2), $bits(big));
    $display("formal=%0d", formal_size(f));
    $finish(0);
  end
endmodule
