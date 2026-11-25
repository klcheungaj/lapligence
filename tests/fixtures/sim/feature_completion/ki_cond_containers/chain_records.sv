// Conditional chains over records with string, real, handle, container, nested and array members: every predicate position, ambiguous ones included (IEEE 1800-2009 11.4.11).

module tb;
  class c_t;
    int id;
    function new(int id); this.id = id; endfunction
  endclass
  typedef struct { string n; int v; } in_t;
  typedef struct { int a, k; logic [7:0] b; string s; real r; c_t h; int q[$]; in_t in; int f[2]; } rec_t;
  typedef struct { int a; string s; } sm_t;
  logic [31:0] s;
  c_t objs[5];
  rec_t d0;
  rec_t d1;
  rec_t d2;
  rec_t d3;
  rec_t d4;
  sm_t e0;
  sm_t e1;
  sm_t e2;
  sm_t e3;
  sm_t e4;
  sm_t e5;
  sm_t e6;
  sm_t e7;
  sm_t e8;
  sm_t e9;
  sm_t e10;
  sm_t e11;
  sm_t e12;
  sm_t e13;
  sm_t e14;
  sm_t e15;
  sm_t e16;
  sm_t e17;
  sm_t e18;
  sm_t e19;
  sm_t e20;
  sm_t e21;
  sm_t e22;
  sm_t e23;
  sm_t e24;
  sm_t e25;
  sm_t e26;
  sm_t e27;
  sm_t e28;
  sm_t e29;
  sm_t e30;
  sm_t e31;
  sm_t e32;
  rec_t y;
  sm_t z;
  function automatic rec_t make(int i);
    make.a = i; make.k = 7; make.b = (i / 2) % 2 ? 8'ha5 : 8'h5a;
    make.s = $sformatf("s%0d", i / 4); make.r = (i / 4) * 0.5; make.h = objs[i / 8];
    make.q = '{i / 4, 1}; make.in.n = "n"; make.in.v = i / 2; make.f = '{3, i};
  endfunction
  task automatic show(input string what, tag, input int n, i, input rec_t v);
    $write("%s n=%0d %s %0d: %0d %0d %h <%s> %.1f %0d", what, n, tag, i, v.a, v.k, v.b, v.s, v.r, v.h == null ? -1 : v.h.id);
    $display(" [%0d:%0d] <%s> %0d %0d %0d", v.q.size(), v.q.size() > 0 ? v.q[0] : -1, v.in.n, v.in.v, v.f[0], v.f[1]);
  endtask
  rec_t ny;
  sm_t cz;
  typedef union tagged { string Text; int Num; } u_t;
  u_t u;
  u_t u0;
  u_t u1;
  u_t u2;
  u_t u3;
  u_t u4;
  string calls;
assign cz = s[0] ? e0 : s[1] ? e1 : s[2] ? e2 : s[3] ? e3 : e4;
  function automatic logic plus(input sm_t v);
    calls = {calls, " p"};
    return v.a > 0;
  endfunction
  function automatic sm_t ext(input sm_t v);
    calls = {calls, " e"};
    v.s = {v.s, "+"};
    return v;
  endfunction
  task automatic check(input string tag, input int n, i);
    case (n)
      4: begin
y = s[0] ? d0 : s[1] ? d1 : s[2] ? d2 : s[3] ? d3 : d4;
      end
      8: begin
z = s[0] ? e0 : s[1] ? e1 : s[2] ? e2 : s[3] ? e3 : s[4] ? e4 : s[5] ? e5 : s[6] ? e6 : s[7] ? e7 :
        e8;
      end
      32: begin
z = s[0] ? e0 : s[1] ? e1 : s[2] ? e2 : s[3] ? e3 : s[4] ? e4 : s[5] ? e5 : s[6] ? e6 : s[7] ? e7 :
        s[8] ? e8 : s[9] ? e9 : s[10] ? e10 : s[11] ? e11 : s[12] ? e12 : s[13] ? e13 : s[14] ? e14
        : s[15] ? e15 : s[16] ? e16 : s[17] ? e17 : s[18] ? e18 : s[19] ? e19 : s[20] ? e20 : s[21]
        ? e21 : s[22] ? e22 : s[23] ? e23 : s[24] ? e24 : s[25] ? e25 : s[26] ? e26 : s[27] ? e27 :
        s[28] ? e28 : s[29] ? e29 : s[30] ? e30 : s[31] ? e31 : e32;
      end
    endcase
    if (n > 4) $display("z n=%0d %s %0d: %0d <%s>", n, tag, i, z.a, z.s);
    else show("m", tag, n, i, y);
    if (n == 4) begin
ny <= s[0] ? d0 : s[1] ? d1 : s[2] ? d2 : s[3] ? d3 : d4;
u = s[0] ? u0 : s[1] ? u1 : s[2] ? u2 : s[3] ? u3 : u4;
      calls = "";
z = plus(s[0] ? e0 : e0) ? ext(s[0] ? e0 : e1) : plus(s[1] ? e1 : e0) ? ext(s[1] ? e1 : e2) :
        plus(s[2] ? e2 : e0) ? ext(s[2] ? e2 : e3) : plus(s[3] ? e3 : e0) ? ext(s[3] ? e3 : e4) :
        e2;
      $display("a n=%0d %s %0d:%s %0d <%s>", n, tag, i, calls, z.a, z.s);
      #1;
      show("n", tag, n, i, ny);
      $display("c n=%0d %s %0d: %0d <%s>", n, tag, i, cz.a, cz.s);
      if (u matches tagged Text .t) $display("u n=%0d %s %0d: text <%s>", n, tag, i, t);
      else if (u matches tagged Num .v) $display("u n=%0d %s %0d: num %0d", n, tag, i, v);
      else $display("u n=%0d %s %0d: none", n, tag, i);
    end
  endtask
  // Predicates: one of them 1, X or Z; an X followed by a 1; an X and a Z two further on; none; all X.
  task automatic drive(input int n);
    for (int i = 0; i < n; i++) begin
      s = '0; s[i] = 1'b1; check("1", n, i);
      s[i] = 1'bx; check("x", n, i);
      s[i] = 1'bz; check("z", n, i);
      if (i + 1 < n) begin s[i] = 1'bx; s[i + 1] = 1'b1; check("x1", n, i); s[i + 1] = 1'b0; end
      if (i + 2 < n) begin s[i] = 1'bx; s[i + 2] = 1'bz; check("xz", n, i); end
    end
    s = '0; check("none", n, 0);
    s = 'x; check("allx", n, 0);
  endtask
  initial begin
    foreach (objs[i]) objs[i] = new(i);
    d0 = make(0);
    d1 = make(1);
    d2 = make(2);
    d3 = make(3);
    d4 = make(4);
    e0 = '{0, "s0"};
    e1 = '{0, "s0"};
    e2 = '{0, "s0"};
    e3 = '{1, "s0"};
    e4 = '{1, "s1"};
    e5 = '{1, "s1"};
    e6 = '{2, "s1"};
    e7 = '{2, "s1"};
    e8 = '{2, "s2"};
    e9 = '{3, "s2"};
    e10 = '{3, "s2"};
    e11 = '{3, "s2"};
    e12 = '{4, "s3"};
    e13 = '{4, "s3"};
    e14 = '{4, "s3"};
    e15 = '{5, "s3"};
    e16 = '{5, "s4"};
    e17 = '{5, "s4"};
    e18 = '{6, "s4"};
    e19 = '{6, "s4"};
    e20 = '{6, "s5"};
    e21 = '{7, "s5"};
    e22 = '{7, "s5"};
    e23 = '{7, "s5"};
    e24 = '{8, "s6"};
    e25 = '{8, "s6"};
    e26 = '{8, "s6"};
    e27 = '{9, "s6"};
    e28 = '{9, "s7"};
    e29 = '{9, "s7"};
    e30 = '{10, "s7"};
    e31 = '{10, "s7"};
    e32 = '{10, "s8"};
    u0 = tagged Text ("t0");
    u1 = tagged Text ("t0");
    u2 = tagged Num (0);
    u3 = tagged Num (1);
    u4 = tagged Text ("t1");
    drive(4); drive(8); drive(32);
  end
endmodule
