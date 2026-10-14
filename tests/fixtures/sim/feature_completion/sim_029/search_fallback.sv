// Multiplications whose exact (BDD) model exceeds its node budget and whose
// solutions are too rare for rejection sampling: the CDCL search fallback
// must still find solutions (IEEE 1800-2009 18.6.1 L30399-30400: randomize()
// "generates random values for all the active random variables in the object,
// subject to the active constraints") and prove infeasibility, after which
// "the random variables retain their previous values" (18.6.3 L30471-30472).
// The search is not uniform (S29-D10); only solution membership is printed.
class m_c;
  rand bit [11:0] x, y;
  constraint c { x * y == 12'd391; x > 1; y > 1; }
endclass
class f_c;
  rand bit [15:0] x, y;
  constraint c { 32'(x) * 32'(y) == 32'd60491; x > 1; y > 1; }
endclass
class p_c;
  rand bit [11:0] x, y;
  constraint c { 24'(x) * 24'(y) == 24'd4093; x > 1; y > 1; }
endclass
module tb;
  initial begin
    static m_c m = new;
    static f_c f = new;
    static p_c p = new;
    int ok, bad;
    bad = 0;
    repeat (8) begin
      ok = m.randomize();
      if (!ok || m.x * m.y != 12'd391 || m.x <= 1 || m.y <= 1) bad++;
    end
    $display("modular product bad=%0d", bad);
    bad = 0;
    repeat (4) begin
      ok = f.randomize();
      if (!ok || !((f.x == 251 && f.y == 241) || (f.x == 241 && f.y == 251))) bad++;
    end
    $display("factors of 60491 bad=%0d", bad);
    p.x = 7;
    p.y = 9;
    ok = p.randomize();
    $display("prime 4093 ok=%0d x=%0d y=%0d", ok, p.x, p.y);
    $finish;
  end
endmodule
