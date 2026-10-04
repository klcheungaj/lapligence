// IEEE 1800-2009 13.5.2, 7.12.2: a descriptor-backed const ref formal is a
// read-only receiver for reverse.
module tb;
  localparam int N = 65537;
  bit [31:0] a [N];
  function automatic void f(const ref bit [31:0] r [N]);
    r.reverse();
  endfunction
  initial begin
    f(a);
    $finish;
  end
endmodule
