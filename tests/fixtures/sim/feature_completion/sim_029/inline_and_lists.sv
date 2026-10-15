// SIM-029: in-line constraints (IEEE 1800-2009 18.7), in-line random
// variable control and the checker form (18.11, 18.11.1), and scope
// randomization (18.12).
class ca_c;
  rand byte x, y;
  byte v, w;
  constraint c1 { x < v && y > w; }
endclass

class frame_c;
  rand bit [7:0] len;
  rand bit [7:0] hdr;
  constraint len_c { len > 4; }
  function int fill(bit [7:0] max_len);
    return randomize() with { len <= max_len; hdr == len - 1; };
  endfunction
endclass

module tb;
  bit [7:0] limit;
  int unsigned cnt;
  bit [3:0] nib;
  logic [5:0] six;
  initial begin
    static ca_c a = new;
    static frame_c f = new;
    int ok, bad, i;
    byte x0, y0, v0, w0;

    a.v = 10; a.w = -10;
    ok = a.randomize();
    $display("all rand: ok=%0d holds=%0d", ok, a.x < 10 && a.y > -10);
    x0 = a.x; y0 = a.y;
    ok = a.randomize(x);            // y is state
    $display("list x: ok=%0d y_same=%0d holds=%0d", ok, a.y == y0, a.x < 10);
    a.x = 3; a.y = 5;
    ok = a.randomize(v, w);         // x, y are state; v, w random
    $display("list v,w: ok=%0d x=%0d y=%0d holds=%0d", ok, a.x, a.y, a.x < a.v && a.y > a.w);
    v0 = a.v; w0 = a.w;
    ok = a.randomize(null);
    $display("checker true: ok=%0d unchanged=%0d", ok, a.v == v0 && a.w == w0 && a.x == 3);
    a.x = 127;
    ok = a.randomize(null);
    $display("checker false: ok=%0d x=%0d", ok, a.x);

    limit = 9;
    bad = 0;
    for (i = 0; i < 30; i++) begin
      ok = f.randomize() with { len < limit; };   // limit: the caller's scope
      if (ok != 1 || f.len <= 4 || f.len >= 9) bad++;
    end
    $display("with scope variable: bad=%0d", bad);
    ok = f.fill(6);
    $display("with in a method: ok=%0d len_ok=%0d hdr_ok=%0d", ok, f.len inside {5, 6}, f.hdr == f.len - 1);

    ok = std::randomize(cnt, nib) with { cnt < 3; nib == cnt + 4; };
    $display("std::randomize: ok=%0d holds=%0d", ok, cnt < 3 && nib == cnt + 4);
    ok = std::randomize(six);
    $display("std::randomize 4-state: ok=%0d known=%0d", ok, !$isunknown(six));
    ok = std::randomize() with { cnt < 3; };
    $display("std::randomize checker: ok=%0d", ok);
    $finish;
  end
endmodule
