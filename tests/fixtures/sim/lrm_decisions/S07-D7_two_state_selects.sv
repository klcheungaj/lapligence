// Decision S07-D7: a select of a two-state vector whose position lies outside
// the vector reads 0 in every context: bit-selects, part-selects and indexed
// part-selects of `bit` vectors, packed arrays and structures, `byte`,
// `shortint`, `int` and `longint`, two-state members and elements, and
// enumerations. Positions inside the vector keep their value, and a
// four-state vector reads x. No event or edge is seen when the index of a
// two-state select moves from a 0 bit to a missing one, and a write outside
// the vector does nothing.
//
// IEEE 1800-2009 11.5.1 (SystemVerilog-1800-2009.txt L15738-15740): "If the
// bit-select is out of the address bounds or the bit-select is x or z, then
// the value returned by the reference shall be x."
// IEEE 1800-2009 7.4.6 (L8235-8238): "The result of reading from an array
// with an invalid index shall return the default uninitialized value for the
// array element type."
// IEEE 1800-2009 6.11.2 (L5476-5477): "The other types do not have unknown
// values and are called 2-state types": the x of 11.5.1 has no representation
// in the select.
module sub(input logic a, input logic [3:0] b, output logic [3:0] o);
  assign o = 4'b0001 + b;
endmodule
module tb;
  typedef struct packed { bit [7:0] a; bit [3:0] b; } ps_t;
  typedef struct packed { bit [7:0] a; logic [3:0] b; } ms_t;
  typedef union packed { bit [11:0] w; ps_t s; } pu_t;
  typedef enum bit [2:0] { E0, E1 = 5, E2 = 7 } en_t;
  typedef struct { bit [7:0] a; logic [7:0] l; byte bb; bit [3:0] g[2]; } us_t;
  bit [32:0] v;
  bit [0:15] asc;
  bit [39:8] off;
  bit [3:0][7:0] pa;
  bit [1:0][2:0][3:0] p3;
  bit [200:0] wide;
  bit signed [7:0] sg;
  logic [7:0] four;
  int n, i, neg;
  byte b8;
  shortint sh;
  longint ll;
  ps_t ps;
  ps_t [2:0] psa;
  ms_t ms;
  pu_t pu;
  en_t en;
  us_t us;
  us_t usq[$];
  bit [7:0] mem[4];
  bit [3:0][7:0] pmem[2];
  bit [7:0] q[$];
  bit [7:0] da[];
  bit [7:0] aa[int];
  logic [3:0] r, so;
  logic l, comb;
  wire w1 = v[i];
  wire [3:0] w4 = v[i +: 4];
  wire wn;
  int rises, falls, changes;
  not (wn, v[i]);
  sub s0(.a(v[i]), .b(v[i +: 4]), .o(so));
  always @(posedge v[i]) rises++;
  always @(negedge v[i]) falls++;
  always @(v[i]) changes++;
  always_comb comb = v[i];
  class C;
    bit [7:0] f;
    static bit [7:0] st;
    bit [7:0] arr[3];
    function logic [3:0] add(int k); return 4'b0001 + f[k]; endfunction
  endclass
  C c;
  function automatic logic [3:0] fv(input bit [7:0] a, input int k);
    return 4'b0001 + a[k];
  endfunction
  function automatic logic [3:0] fr(ref bit [7:0] a, input int k);
    return 4'b0001 + a[k];
  endfunction
  function automatic logic [3:0] fl(input int k);
    ps_t t;
    t = '1;
    return 4'b0001 + t.a[k];
  endfunction
  task automatic inout_bit(inout logic a, input logic [3:0] b);
    $display("task %b %b", a, b);
  endtask
  bit [7:0] one;
  logic nl;
  logic [3:0] nr;
  initial #2 begin
    nl <= v[i];
    nr <= 4'b0001 + v[i +: 4];
  end
  initial begin
    v = '1; asc = '1; off = '1; pa = '1; p3 = '1; wide = '1; sg = '1; four = '1; one = '1;
    n = -1; b8 = -1; sh = -1; ll = -1;
    ps = '1; psa = '1; ms = '1; pu = '1; en = E2;
    us.a = '1; us.l = '1; us.bb = -1; us.g[0] = '1; us.g[1] = '1; usq.push_back(us);
    foreach (mem[k]) mem[k] = '1;
    foreach (pmem[k]) pmem[k] = '1;
    q = {8'hff, 8'hff};
    da = new[2];
    da[0] = '1; da[1] = '1;
    aa[3] = '1;
    c = new;
    c.f = '1; C::st = '1;
    foreach (c.arr[k]) c.arr[k] = '1;
    i = 0; neg = -2;
    #1 i = 40;
    #1 i = 41;
    #1 $display("events rises=%0d falls=%0d changes=%0d", rises, falls, changes);
    $display("bit %b %b %b", v[i], v[neg], 4'b0001 + v[i]);
    r = 4'b0001 + v[i];
    $display("assigned %b", r);
    $display("part %b %b %b %b", v[i +: 4], v[i -: 4], v[i - 10 +: 4], v[neg +: 4]);
    $display("constant part %b %b", v[40:38], v[34:31]);
    $display("ascending %b %b %b", asc[i], asc[i +: 3], asc[14:17]);
    $display("offset %b %b %b", off[i], off[neg], off[neg + 8 +: 4]);
    $display("packed array %h %b %b %b %h", pa[i], pa[1][i], pa[i][1], pa[i][3:0], pa[1][i +: 4]);
    $display("three dims %h %h %b", p3[i], p3[1][i], p3[1][1][i]);
    $display("int %b %b %b %b %b %b", n[i], b8[i], sh[i], ll[i + 60], n[i +: 8], b8[i -: 2]);
    $display("wide %b %h %b", wide[i + 400], wide[i + 400 +: 100], wide[i + 154 +: 10]);
    $display("signed %b %0d", sg[i], sg[i +: 3]);
    $display("struct %b %b %b %b", ps[i], ps.a[i], ps.a[i +: 4], psa[i].a);
    $display("mixed struct %b %b", ms.a[i], ms.a[i +: 4]);
    $display("union %b %b %b", pu[i], pu.w[i], pu.s.b[i]);
    $display("enum %b", en[i]);
    $display("members %b %b %b %b", us.a[i], us.bb[i], us.g[1][i], usq[0].a[i]);
    $display("elements %b %b %b %b", mem[1][i], mem[1][i +: 4], mem[i][1], pmem[1][1][i]);
    $display("containers %b %b %b %b", q[1][i], da[1][i +: 4], aa[3][i], aa[3][i -: 2]);
    $display("class %b %b %b %b", c.f[i], C::st[i], c.arr[1][i], c.add(i));
    $display("functions %b %b %b", fv(8'hff, i), fr(one, i), fl(i));
    $display("four-state %b %b %b", four[i], four[i +: 4], us.l[i]);
    $display("nets %b %b %b %b %b", w1, w4, wn, so, comb);
    $display("nonblocking %b %b", nl, nr);
    $display("operators %b %b %b %b", v[i] === 1'bx, !v[i], ^v[i +: 4], ~v[i +: 4]);
    $display("conditional %b %b", v[i] ? 4'b1010 : 4'b0101, {v[i], 1'b1});
    case (v[i])
      1'bx: $display("case x");
      1'b0: $display("case 0");
      default: $display("case other");
    endcase
    $display("functions of it %b %0d %0.1f", $isunknown(v[i]), $countones(v[i +: 4]), v[i] + 0.5);
    inout_bit(v[i], v[i +: 4]);
    l = #1 v[i];
    $display("intra-assignment %b", l);
    force l = v[i];
    #1 $display("force %b", l);
    release l;
    r = v[i]++;
    $display("increment %b", r);
    r = mem[1][i]++;
    $display("element increment %b", r);
    v[i] = 1'b0; v[i +: 4] = '0; v[i - 10 +: 4] = '0;
    pa[i] = '0; pa[1][i] = 1'b0; mem[1][i] = 1'b0; ps.a[i] = 1'b0; n[i] = 1'b0;
    $display("writes %h %h %h %h %h", v, pa, mem[1], ps, n);
    $finish;
  end
endmodule
