// Members of packed-array elements (IEEE 1800-2009 7.2.1, 7.4.1, 7.4.5,
// 11.5.1): constant and runtime element selects, every range direction,
// nested members and dimensions, signed, union and two-state members,
// out-of-range and X elements, member sub-selects, blocking and
// nonblocking writes, locals, formals, parameters, unpacked arrays of
// packed arrays and packed-array structure members.
module tb;
  typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pair_t;
  typedef struct packed { logic signed [3:0] s; pair_t p; } rec_t;
  typedef union packed { logic [7:0] b; pair_t p; } u_t;
  typedef struct packed { pair_t a; pair_t b; } nest_t;
  typedef struct packed { logic [3:0] f; bit [3:0] t; } mixed_t;
  typedef struct packed { pair_t [1:0] arr; logic [3:0] tag; } holder_t;
  typedef pair_t [1:0] pair2_t;
  localparam pair2_t P = pair2_t'(16'hA5C3);
  pair_t [3:0] ps;
  pair_t [0:3] asc;
  pair_t [5:2] nz;
  pair_t [1:0][1:0] w;
  rec_t [1:0] r;
  u_t [1:0] u;
  nest_t [1:0] ns;
  mixed_t [1:0] mx;
  pair_t [1:0] q [2];
  holder_t h;
  pair_t [3:0] g;
  integer i, j;
  logic [1:0] xi;

  function automatic logic [3:0] pick(input pair_t [3:0] v, input integer k);
    return v[k].hi;
  endfunction

  task automatic poke(ref pair_t [3:0] v, input integer k);
    v[k].lo = 4'hE;
  endtask

  initial begin
    automatic pair2_t l = pair2_t'(16'h1234);
    ps = type(ps)'(32'h87654321);
    i = 3;
    j = 1;
    xi = 2'bx;
    $display("ps const %h %h", ps[1].hi, ps[2].lo);
    $display("ps runtime %h %h", ps[i].hi, ps[i].lo);
    ps[0].hi = 4'hA;
    ps[i].lo = 4'hB;
    $display("ps written %h", ps);
    ps[i-1].hi[3] = 1'b0;
    ps[1].lo[1:0] = 2'b00;
    $display("ps sub %h %b %b", ps, ps[3].hi[3], ps[i-2].hi[2:1]);
    asc = type(asc)'(32'h01234567);
    $display("asc %h %h %h", asc[0].hi, asc[3].lo, asc[j].hi);
    nz = type(nz)'(32'h89abcdef);
    $display("nz %h %h %h", nz[5].hi, nz[2].lo, nz[j+2].lo);
    $display("oor %h %h %h", nz[j+6].hi, nz[j-1].lo, nz[xi].lo);
    nz[j+8].lo = 4'h0;
    nz[j-1].hi = 4'h0;
    nz[xi].lo = 4'h0;
    $display("oor written %h", nz);
    w = type(w)'(32'h87654321);
    $display("w %h %h %h", w[1][0].hi, w[0][1].lo, w[j][0].hi);
    w[1][1].hi = 4'hA;
    w[j][j-1].lo = 4'hB;
    $display("w written %h", w);
    r = '0;
    r[1].s = -3;
    r[0].p.lo = 4'h5;
    r[j-1].p.hi = 4'hC;
    $display("r %h %0d %0d", r, r[1].s, r[j].s);
    u = type(u)'(16'h1234);
    $display("u %h %h %h", u[1].p.hi, u[0].b, u[j-1].p.lo);
    ns = '0;
    ns[j].b.hi = 4'h7;
    ns[0].a.lo = 4'h3;
    $display("ns %h %h", ns, ns[j].b);
    mx = type(mx)'(16'hxx_x5);
    $display("mx %h %h", mx[0].t, mx[j-1].f);
    mx[j].t = 4'bx01z;
    $display("mx written %b %h", mx[1].t, mx[j].f);
    $display("l %h %h", l[j].lo, l[0].hi);
    l[j].hi = 4'h9;
    $display("l written %h", l);
    $display("P %h %h", P[1].hi, P[j-1].lo);
    q[0] = type(q[0])'(16'h1122);
    q[1] = type(q[1])'(16'h3344);
    $display("q %h %h", q[1][0].hi, q[0][j].lo);
    q[1][j].lo = 4'hF;
    $display("q written %h", q[1]);
    h = '0;
    h.arr[1].hi = 4'h7;
    h.arr[j-1].lo = 4'h2;
    $display("h %h %h", h, h.arr[j].hi);
    g = type(g)'(32'h01234567);
    $display("pick %h", pick(g, 2));
    poke(g, j);
    $display("poke %h", g);
    u[1].p.lo <= 4'hF;
    w[j][0].hi <= 4'h0;
    #1 $display("nba %h %h", u, w);
    $finish(0);
  end
endmodule
