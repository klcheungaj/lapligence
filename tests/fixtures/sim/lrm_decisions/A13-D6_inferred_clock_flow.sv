// Decision A13-D6: $inferred_clock as a formal's default is resolved where
// the property is instantiated. For the top-level instance q(a) it is the
// assertion statement's inferred clock, here the default clocking
// posedge c2 (a checked at 20). For q(b) under @(posedge c1) it is the clock
// that flows to the instance, posedge c1 (b checked at 10), although q's own
// clocking event @clk then nullifies the juxtaposed @(posedge c1).
//
// IEEE 1800-2009 16.15.7 (SystemVerilog-1800-2009.txt L27208-27212): "if
// the property or sequence instance is the top-level property expression in
// an assertion statement, the event expression that is used to replace the
// default argument $inferred_clock is that as determined at the location of
// the assertion statement. If the property or sequence instance is not the
// top-level property expression in the assertion statement then the event
// expression determined by clock flow rules up to the instance location in
// the property expression is used as the default value of the argument."
// 16.14.3 (L26099): "Juxtaposing two clocking events nullifies the first of
// them".
`timescale 1ns / 1ns
module tb;
  logic c1 = 1'b0, c2 = 1'b0, a = 1'b0, b = 1'b0;
  default clocking dc @(posedge c2); endclocking
  property q(x, clk = $inferred_clock);
    @clk x;
  endproperty
  initial a1: assert property (q(a))
    $display("%0t a1 pass", $time); else $display("%0t a1 fail", $time);
  initial a2: assert property (@(posedge c1) q(b))
    $display("%0t a2 pass", $time); else $display("%0t a2 fail", $time);
  initial begin
    #5 b = 1'b1;
    #5 c1 = 1'b1;
    #2 begin b = 1'b0; a = 1'b1; end
    #8 c2 = 1'b1;
    #1 $finish;
  end
endmodule
