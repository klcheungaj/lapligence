// IEEE 1800-2009 18.5.4 L29683: "The weight can be any integral
// SystemVerilog expression." The text is silent on a negative weight.
// Decision S30-D8 (llg choice): a negative dist weight is an evaluation error:
// randomize() returns 0, the random variables keep their values, the error
// "negative or unevaluated dist weight" is reported and the run ends with
// exit status 1.
class c_c;
  rand bit [3:0] x;
  int w = -1;
  constraint d { x dist { 1 := w, 2 := 1 }; }
endclass
module tb;
  initial begin
    static c_c c = new;
    int ok;
    c.x = 4'd7;
    ok = c.randomize();
    $display("ok=%0d x=%0d", ok, c.x);
    $finish;
  end
endmodule
