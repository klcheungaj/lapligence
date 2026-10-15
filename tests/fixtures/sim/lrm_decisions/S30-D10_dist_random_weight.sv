// IEEE 1800-2009 18.5.4 L29681-29683: "The distribution set is a
// comma-separated list of integral expressions and ranges. ... The weight can
// be any integral SystemVerilog expression." The text does not say how a
// weight or range bound that is itself random would weigh the solutions.
// Decision S30-D10 (llg restriction): dist item values, range bounds and
// weights must not read random variables; llg rejects them with a located
// compile error. This is a negative case: llg prints nothing. A simulator
// that accepts it prints "x=1 or 2".
class c_c;
  rand bit [3:0] x, w;
  constraint d { x dist { 1 := w, 2 := 1 }; }
endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    if (c.x == 4'd1 || c.x == 4'd2) $display("x=1 or 2");
    $finish;
  end
endmodule
