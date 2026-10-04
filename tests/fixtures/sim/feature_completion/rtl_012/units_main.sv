// RTL-012: separate compilation units start without the directive of
// units_pull.sv (IEEE 1800-2009 3.12.1, 22.9); merged units inherit it.
module ub(input a);
  initial #1 $display("ub %v", a);
endmodule
module tb;
  ua x();
  ub y();
  initial #2 $finish(0);
endmodule
