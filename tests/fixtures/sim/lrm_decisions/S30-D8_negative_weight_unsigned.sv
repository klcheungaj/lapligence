// IEEE 1800-2009 18.5.4 L29683: "The weight can be any integral
// SystemVerilog expression." The text is silent on a negative weight.
// Decision S30-D8 (llg choice): a weight is read as an unsigned value of its
// own width, so a negative weight is a large positive one: the `int` weight
// -1 is 32'hffff_ffff against 1, and the 4-bit signed weight -1 is 15
// against 1. The first line holds unless a draw of probability 2**-32 picks
// 2; the second prints whether the frequency of 1 lies within five standard
// deviations of 15/16.
class int_c;
  rand bit [3:0] x;
  int w = -1;
  constraint d { x dist { 1 := w, 2 := 1 }; }
endclass

class nibble_c;
  rand bit [3:0] x;
  bit signed [3:0] w = -1;
  constraint d { x dist { 1 := w, 2 := 1 }; }
endclass

module tb;
  function automatic bit near(int hits, int total, int num, int den);
    longint h, t, a, b, diff;
    h = longint'(hits);
    t = longint'(total);
    a = longint'(num);
    b = longint'(den);
    diff = h * b - t * a;
    return diff * diff <= 64'sd25 * t * a * (b - a);
  endfunction

  int_c ic;
  nibble_c nc;
  int ok, ones;

  initial begin
    ic = new;
    ic.x = 4'd7;
    ok = ic.randomize();
    $display("ok=%0d x=%0d", ok, ic.x);
    nc = new;
    for (int i = 0; i < 4000; i++) begin
      void'(nc.randomize());
      if (nc.x == 4'd1) ones = ones + 1;
    end
    $display("4-bit -1 weighs 15=%0b", near(ones, 4000, 15, 16));
    $finish;
  end
endmodule
