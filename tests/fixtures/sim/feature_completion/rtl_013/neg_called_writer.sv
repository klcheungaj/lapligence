// IEEE 1800-2009 9.2.2.2: variables written inside functions that always_comb
// calls belong to that always_comb; always_ff may not also write them.
module tb;
  logic [7:0] g, x, y; logic c;
  function automatic logic [7:0] f(input logic [7:0] v);
    g = v;
    return v + 1;
  endfunction
  always_comb y = f(x);
  always_ff @(posedge c) g <= 8'h1;
  initial $finish(0);
endmodule
