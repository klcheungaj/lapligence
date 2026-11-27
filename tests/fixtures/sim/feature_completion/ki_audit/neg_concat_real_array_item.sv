// Remaining items of unpacked array concatenations: a fixed array of reals
// as an item.
module tb;
  real r[2] = '{1.0, 2.0};
  real q[$];
  initial begin
    q = {r, 3.0};
    $display("%p", q);
  end
endmodule
