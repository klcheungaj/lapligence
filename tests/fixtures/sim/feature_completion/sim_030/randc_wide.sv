// randc variables of 17 to 32 bits (IEEE 1800-2009 18.4.2, decision S30-D18).
// Every line prints properties that hold for any permutation the solver
// draws: no value repeats inside a pass, every value is legal, a pass covers
// the legal values.
class small_c;
  randc bit [23:0] v;
  constraint c { v inside {[0:5]}; }
endclass

class top_c;
  randc bit [31:0] w;
  constraint c { w inside {[100:103]}; }
endclass

class full_c;
  randc bit [16:0] v;
endclass

class free_c;
  randc bit [31:0] v;
endclass

class walk_c;
  randc bit [19:0] v;
  constraint c { v[3:0] == 4'd5; }
endclass

class signed_c;
  randc int s;
  constraint c { s inside {[-3:2]}; }
endclass

typedef enum bit [19:0] {
  E_ONE = 20'd1,
  E_MID = 20'd70000,
  E_TOP = 20'd500000
} wide_e;

class enum_c;
  randc wide_e e;
endclass

class pair_c;
  randc bit [16:0] a;
  randc bit [16:0] b;
  constraint c { a < 300; b < 300; a != b; }
endclass

class shared_c;
  static randc bit [17:0] s;
  constraint c { s inside {18'd7, 18'd70000, 18'd200000}; }
endclass

class mode_c;
  randc bit [23:0] v;
  rand bit [3:0] x;
  constraint lo { v < 4; }
  constraint hi { v < 6; }
endclass

virtual class width_base;
  pure virtual function bit [31:0] value();
endclass

class width_c #(int W = 17) extends width_base;
  randc bit [W-1:0] v;
  constraint c { (v >> 2) == (32'd5 << (W - 5)); }
  virtual function bit [31:0] value();
    return v;
  endfunction
endclass

module tb;
  width_c #(17) w17 = new;
  width_c #(18) w18 = new;
  width_c #(19) w19 = new;
  width_c #(20) w20 = new;
  width_c #(21) w21 = new;
  width_c #(22) w22 = new;
  width_c #(23) w23 = new;
  width_c #(24) w24 = new;
  width_c #(25) w25 = new;
  width_c #(26) w26 = new;
  width_c #(27) w27 = new;
  width_c #(28) w28 = new;
  width_c #(29) w29 = new;
  width_c #(30) w30 = new;
  width_c #(31) w31 = new;
  width_c #(32) w32 = new;
  width_base widths[16];
  bit [31:0] got;
  small_c s1, s2;
  top_c t;
  full_c f;
  free_c fr;
  walk_c wk;
  signed_c sg;
  enum_c en;
  pair_c pr;
  shared_c h1, h2;
  mode_c md;
  bit [5:0] seen_1, seen_2;
  bit [3:0] seen_t;
  bit seen_f[131072];
  bit seen_k[65536];
  bit seen_p[300];
  bit [31:0] drawn[$];
  bit dup, bad;
  bit hi_seen, lo_seen;
  int ok, count;

  initial begin
    s1 = new;
    s2 = new;
    // Two passes of the six legal values per object; the objects interleave.
    for (int pass = 0; pass < 2; pass++) begin
      seen_1 = '0;
      seen_2 = '0;
      for (int i = 0; i < 6; i++) begin
        if (!s1.randomize() || !s2.randomize()) bad = 1'b1;
        if (s1.v > 5 || s2.v > 5) bad = 1'b1;
        else begin
          if (seen_1[s1.v] || seen_2[s2.v]) dup = 1'b1;
          seen_1[s1.v] = 1'b1;
          seen_2[s2.v] = 1'b1;
        end
      end
      $display("24 bits, pass %0d: a=%b b=%b dup=%0b bad=%0b", pass, seen_1, seen_2, dup, bad);
    end

    t = new;
    for (int pass = 0; pass < 2; pass++) begin
      seen_t = '0;
      for (int i = 0; i < 4; i++) begin
        if (!t.randomize() || t.w < 100 || t.w > 103) bad = 1'b1;
        else begin
          if (seen_t[t.w - 100]) dup = 1'b1;
          seen_t[t.w - 100] = 1'b1;
        end
      end
      $display("32 bits, pass %0d: %b dup=%0b bad=%0b", pass, seen_t, dup, bad);
    end

    // An unconstrained 17-bit variable returns every value once per pass.
    f = new;
    count = 0;
    for (int i = 0; i < 131072; i++) begin
      if (!f.randomize()) bad = 1'b1;
      if (seen_f[f.v]) dup = 1'b1;
      else count++;
      seen_f[f.v] = 1'b1;
    end
    ok = f.randomize();
    $display("17 bits, whole range: distinct=%0d dup=%0b next pass ok=%0d", count, dup, ok);

    // 32 bits: no repeat among the draws, and both halves of the range occur.
    fr = new;
    for (int i = 0; i < 2000; i++) begin
      if (!fr.randomize()) bad = 1'b1;
      if (fr.v[31]) hi_seen = 1'b1;
      else lo_seen = 1'b1;
      drawn.push_back(fr.v);
    end
    drawn.sort();
    for (int i = 1; i < drawn.size(); i++) if (drawn[i] == drawn[i-1]) dup = 1'b1;
    $display("32 bits, unconstrained: dup=%0b halves=%0b%0b", dup, hi_seen, lo_seen);

    // 65536 legal values out of 2**20: one pass returns each of them once.
    wk = new;
    count = 0;
    for (int i = 0; i < 65536; i++) begin
      if (!wk.randomize() || wk.v[3:0] != 4'd5) bad = 1'b1;
      if (seen_k[wk.v[19:4]]) dup = 1'b1;
      else count++;
      seen_k[wk.v[19:4]] = 1'b1;
    end
    ok = wk.randomize();
    $display("20 bits, one in sixteen: distinct=%0d dup=%0b bad=%0b next pass ok=%0d legal=%0b",
             count, dup, bad, ok, wk.v[3:0] == 4'd5);

    sg = new;
    for (int pass = 0; pass < 2; pass++) begin
      seen_1 = '0;
      for (int i = 0; i < 6; i++) begin
        if (!sg.randomize() || sg.s < -3 || sg.s > 2) bad = 1'b1;
        else begin
          if (seen_1[sg.s + 3]) dup = 1'b1;
          seen_1[sg.s + 3] = 1'b1;
        end
      end
      $display("signed, pass %0d: %b dup=%0b bad=%0b", pass, seen_1, dup, bad);
    end

    en = new;
    for (int pass = 0; pass < 2; pass++) begin
      seen_t = '0;
      for (int i = 0; i < 3; i++) begin
        if (!en.randomize()) bad = 1'b1;
        case (en.e)
          E_ONE: begin if (seen_t[0]) dup = 1'b1; seen_t[0] = 1'b1; end
          E_MID: begin if (seen_t[1]) dup = 1'b1; seen_t[1] = 1'b1; end
          E_TOP: begin if (seen_t[2]) dup = 1'b1; seen_t[2] = 1'b1; end
          default: bad = 1'b1;
        endcase
      end
      $display("enum, pass %0d: %b dup=%0b bad=%0b", pass, seen_t, dup, bad);
    end

    // Two variables of one constraint: the first cycles over its 300 values;
    // the second never repeats a value while its pass lasts.
    pr = new;
    count = 0;
    for (int i = 0; i < 300; i++) begin
      if (!pr.randomize() || pr.a >= 300 || pr.b >= 300 || pr.a == pr.b) bad = 1'b1;
      else begin
        if (seen_p[pr.a]) dup = 1'b1;
        else count++;
        seen_p[pr.a] = 1'b1;
      end
      if (i < 200) drawn[i] = pr.b;
    end
    for (int i = 0; i < 200; i++)
      for (int j = i + 1; j < 200; j++) if (drawn[i] == drawn[j]) dup = 1'b1;
    $display("two variables: first distinct=%0d dup=%0b bad=%0b", count, dup, bad);

    // A static variable has one cycle for all instances.
    h1 = new;
    h2 = new;
    seen_t = '0;
    for (int i = 0; i < 3; i++) begin
      if (i == 1) ok = h2.randomize();
      else ok = h1.randomize();
      if (!ok) bad = 1'b1;
      case (shared_c::s)
        18'd7: begin if (seen_t[0]) dup = 1'b1; seen_t[0] = 1'b1; end
        18'd70000: begin if (seen_t[1]) dup = 1'b1; seen_t[1] = 1'b1; end
        18'd200000: begin if (seen_t[2]) dup = 1'b1; seen_t[2] = 1'b1; end
        default: bad = 1'b1;
      endcase
    end
    $display("static: %b dup=%0b bad=%0b", seen_t, dup, bad);

    // Two draws into a pass, then the constraints change: a new pass over the
    // six values now legal starts. An inactive variable and a failed call
    // leave value and pass alone.
    md = new;
    ok = md.randomize();
    ok = md.randomize();
    md.lo.constraint_mode(0);
    seen_1 = '0;
    for (int i = 0; i < 3; i++) begin
      if (!md.randomize() || md.v > 5) bad = 1'b1;
      else begin
        if (seen_1[md.v]) dup = 1'b1;
        seen_1[md.v] = 1'b1;
      end
    end
    count = md.v;
    md.v.rand_mode(0);
    ok = md.randomize();
    if (md.v != count) bad = 1'b1;
    md.v.rand_mode(1);
    if (md.randomize() with { v == 24'd9; }) bad = 1'b1;
    if (md.v != count) bad = 1'b1;
    for (int i = 0; i < 3; i++) begin
      if (!md.randomize() || md.v > 5) bad = 1'b1;
      else begin
        if (seen_1[md.v]) dup = 1'b1;
        seen_1[md.v] = 1'b1;
      end
    end
    $display("constraints changed, inactive, failed: %b dup=%0b bad=%0b", seen_1, dup, bad);

    // Every width: the top three bits fixed, the two low bits free, so four
    // legal values spread over the range; three passes each.
    widths = '{w17, w18, w19, w20, w21, w22, w23, w24, w25, w26, w27, w28, w29, w30, w31, w32};
    count = 0;
    foreach (widths[i]) begin
      for (int pass = 0; pass < 3; pass++) begin
        seen_t = '0;
        for (int k = 0; k < 4; k++) begin
          if (!widths[i].randomize()) bad = 1'b1;
          got = widths[i].value();
          if (got >> 2 != 32'd5 << (i + 12)) bad = 1'b1;
          if (seen_t[got[1:0]]) dup = 1'b1;
          seen_t[got[1:0]] = 1'b1;
        end
        if (seen_t == 4'b1111) count++;
      end
    end
    $display("widths 17 to 32: full passes=%0d dup=%0b bad=%0b", count, dup, bad);
    $finish;
  end
endmodule
