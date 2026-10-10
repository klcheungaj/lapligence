// SV 6.21, 13.3.2: an automatic variable cannot be a force target.
module tb;
  task automatic t;
    logic [3:0] a;
    force a = 4'h1;
  endtask
  initial t();
endmodule
