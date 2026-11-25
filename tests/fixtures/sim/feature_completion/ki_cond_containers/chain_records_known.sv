// Conditional chains over records with string, real, handle, container, nested and array members: known predicates (IEEE 1800-2009 11.4.11).

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
  rec_t y, y4;
  sm_t z;
  rec_t list[$];
  function automatic rec_t make(int i);
    make.a = i; make.k = 7; make.b = (i / 2) % 2 ? 8'ha5 : 8'h5a;
    make.s = $sformatf("s%0d", i / 4); make.r = (i / 4) * 0.5; make.h = objs[i / 8];
    make.q = '{i / 4, 1}; make.in.n = "n"; make.in.v = i / 2; make.f = '{3, i};
  endfunction
  task automatic show(input string what, tag, input int n, i, input rec_t v);
    $write("%s n=%0d %s %0d: %0d %0d %h <%s> %.1f %0d", what, n, tag, i, v.a, v.k, v.b, v.s, v.r, v.h == null ? -1 : v.h.id);
    $display(" [%0d:%0d] <%s> %0d %0d %0d", v.q.size(), v.q.size() > 0 ? v.q[0] : -1, v.in.n, v.in.v, v.f[0], v.f[1]);
  endtask
  function automatic rec_t pick();
return s[0] ? d0 : s[1] ? d1 : s[2] ? d2 : s[3] ? d3 : d4;
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
    endcase
    if (n > 4) $display("z n=%0d %s %0d: %0d <%s>", n, tag, i, z.a, z.s);
    else show("m", tag, n, i, y);
    if (n == 4) begin
      rec_t l;
      l = pick();
      show("f", tag, n, i, l);
      y4 = d3;
y4 = s[0] ? d0 : s[1] ? d1 : s[2] ? d2 : s[3] ? y4 : y4;
      show("s", tag, n, i, y4);
list[1] = s[0] ? d0 : s[1] ? d1 : s[2] ? d2 : s[3] ? d3 : d4;
      show("e", tag, n, i, list[1]);
    end
  endtask
  task automatic drive(input int n);
    for (int i = 0; i < n; i++) begin
      s = '0; s[i] = 1'b1; check("1", n, i);
    end
    s = '0; check("none", n, 0);
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
    list = '{d0, d1};
    drive(4); drive(8);
  end
endmodule
