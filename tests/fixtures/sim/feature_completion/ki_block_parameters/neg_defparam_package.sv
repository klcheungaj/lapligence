package p;
  function int f();
    parameter P = 3;
    return P;
  endfunction
endpackage
module tb;
  defparam p::f.P = 2;
  initial $display("%0d", p::f());
endmodule
