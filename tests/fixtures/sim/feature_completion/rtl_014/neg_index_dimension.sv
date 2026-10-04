// IEEE 1800-2009 7.12.4: a one-dimensional receiver's iterator has no
// second index dimension.
module tb;
  int a [4];
  initial begin
    $display("%0d", a.sum() with (item.index(2)));
    $finish;
  end
endmodule
