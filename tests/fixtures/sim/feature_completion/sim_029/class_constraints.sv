// SIM-029: class randomize() with constraint blocks (IEEE 1800-2009 18.4-18.6).
// Every result is a relation that holds for any legal solution, so the
// output does not depend on the values drawn.
class sum_c;
  rand bit [7:0] x, y, z;
  constraint c { z == x + y; }
endclass

class packet_c;
  rand bit [3:0] kind;
  rand bit [7:0] len;
  rand bit [15:0] addr;
  bit [7:0] max_len;
  constraint len_c { len >= 2; len <= max_len; }
  constraint kind_c { kind inside {[1:3], 7}; }
  constraint impl_c { (kind == 7) -> len == 2; }
  constraint cond_c { if (kind == 1) addr[1:0] == 2'b00; else addr[0] == 1'b1; }
  constraint proto_c;
endclass
constraint packet_c::proto_c { addr != 16'hffff; }

class base_c;
  rand bit [7:0] v;
  rand bit [3:0] w;
  constraint range_c { v < 16; }
  constraint w_c { w > 3; }
endclass

class derived_c extends base_c;
  rand bit [3:0] extra;
  constraint range_c { v >= 200; }   // replaces base_c::range_c (18.5.2)
  constraint extra_c { extra == w; }
endclass

class counter_c;
  static rand bit [3:0] shared;
  rand bit [3:0] own;
  static constraint shared_c { shared > 12; }
endclass

module tb;
  initial begin
    static sum_c s = new;
    static packet_c p = new;
    static derived_c d = new;
    static base_c through_base = d;
    static base_c plain = new;
    static counter_c c1 = new, c2 = new;
    int ok, bad, distinct, i;
    bit [7:0] first;

    bad = 0;
    for (i = 0; i < 40; i++) begin
      ok = s.randomize();
      if (ok != 1 || s.z != 8'(s.x + s.y)) bad++;
    end
    $display("sum: bad=%0d", bad);

    p.max_len = 9;
    bad = 0;
    distinct = 0;
    first = 0;
    for (i = 0; i < 60; i++) begin
      ok = p.randomize();
      if (ok != 1) bad++;
      if (!(p.len >= 2 && p.len <= 9)) bad++;
      if (!(p.kind inside {1, 2, 3, 7})) bad++;
      if (p.kind == 7 && p.len != 2) bad++;
      if (p.kind == 1 && p.addr[1:0] != 0) bad++;
      if (p.kind != 1 && p.addr[0] != 1) bad++;
      if (p.addr == 16'hffff) bad++;
      if (i == 0) first = p.len;
      else if (p.len != first) distinct = 1;
    end
    $display("packet: bad=%0d varies=%0d", bad, distinct);

    bad = 0;
    for (i = 0; i < 40; i++) begin
      ok = through_base.randomize();
      if (ok != 1 || d.v < 200 || d.w <= 3 || d.extra != d.w) bad++;
      ok = plain.randomize();
      if (ok != 1 || plain.v >= 16 || plain.w <= 3) bad++;
    end
    $display("inheritance: bad=%0d", bad);

    ok = c1.randomize();
    $display("static: ok=%0d shared=%0d same=%0d", ok, c1.shared > 12, c2.shared == c1.shared);
    $finish;
  end
endmodule
