// SIM-024: tagged unions have no assignment-pattern text in this simulator;
// `%p` of one is rejected explicitly rather than printed incorrectly.
module tb;
  typedef union tagged { int a; bit [3:0] b; } u_t;
  u_t u;
  initial begin
    u = tagged a 5;
    $display("%p", u);
  end
endmodule
