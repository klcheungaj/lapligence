// llg-test-fixture: tests/fixtures/sim/container_selects/wide_index_queue.sv
// An index of a queue element is never truncated (IEEE 1800-2009 7.10.1,
// L9042-9047). A value that needs more than 32 bits, or an unsigned value
// above 2**31-1, is outside 0...$ and so is an X or Z index: a read returns
// the element default and a write is ignored with a warning, while a write at
// $+1 still appends. Bounds of a slice clip whatever their width.
module tb;
  int q[$];
  logic [15:0] lq[$];
  string sq[$];
  real rq[$];
  typedef struct { int a; string s; } rec_t;
  rec_t rr[$];
  typedef struct { int items[$]; } holder_t;
  holder_t h;
  class C; int dq[$]; endclass
  C c;
  int out[$];
  logic [32:0] w33;
  logic [32:0] w33e;
  logic [63:0] w64;
  logic [63:0] hi;
  logic [64:0] w65;
  int unsigned u32;
  longint lneg;
  logic signed [64:0] m65;
  logic [3:0] ux;
  int x;

  task automatic bump(ref int r);
    r = r + 100;
  endtask

  initial begin
    w33 = 33'h1_0000_0003;
    w33e = 33'h1_0000_0004;
    w64 = 64'h1_0000_0003;
    hi = 64'hFFFF_FFFF_0000_0001;
    w65 = 65'h1_0000_0000_0000_0003;
    u32 = 32'hFFFF_FFFF;
    lneg = -2;
    m65 = -5;
    ux = 4'bx001;
    q = {5, 6, 7, 8};
    lq = {16'h11, 16'h22, 16'h33, 16'h44};
    sq = {"a", "b", "c", "d"};
    rq = {1.5, 2.5, 3.5, 4.5};
    rr = '{'{1, "x"}, '{2, "y"}, '{3, "z"}, '{4, "w"}};
    h.items = {10, 20, 30, 40};
    c = new;
    c.dq = {50, 60, 70, 80};

    // Reads: the low bits would address element 3, 1 or 0 but are not used.
    $display("int  %0d %0d %0d %0d %0d %0d %0d", q[w33], q[w64], q[w65], q[hi], q[u32], q[ux], q[lneg]);
    $display("logic %h %h %h %h", lq[w33], lq[hi], lq[u32], lq[ux]);
    $display("string [%s] [%s] [%s]", sq[w33], sq[w65], sq[u32]);
    $display("real %0.1f %0.1f", rq[w33], rq[hi]);
    $display("record %0d [%s]", rr[w33].a, rr[w65].s);
    $display("struct %0d class %0d", h.items[w33], c.dq[w64]);
    $display("valid %0d %0d %0d", q[3], q[0], q[$]);

    // Writes are ignored, with the warning on stderr; compare the queues.
    q[w33] = 901; q[w64] = 902; q[w65] = 903; q[hi] = 904; q[u32] = 905; q[ux] = 906; q[lneg] = 907;
    lq[w33] = 16'hA1; lq[hi] = 16'hA2;
    sq[w33] = "X"; sq[hi] = "Y";
    rq[w33] = 9.5;
    rr[w33].a = 99; rr[hi].s = "Q";
    h.items[w33] = 98;
    c.dq[w64] = 97;
    $display("q=%p", q);
    $display("lq=%p sq=%p", lq, sq);
    $display("rq=%p", rq);
    $display("rr=%p", rr);
    $display("h=%p dq=%p", h.items, c.dq);

    // Compound assignments and a ref argument are a read plus a write.
    q[w33] += 5;
    q[hi]++;
    bump(q[w33]);
    bump(q[1]);
    $display("after q=%p", q);

    // $+1 and the size still append; the same low bits behind a wide value do not.
    q[w33e] = 200;
    $display("not appended: size=%0d", q.size());
    q[q.size()] = 201;
    q[$+1] = 202;
    $display("appended: %p", q);

    // $ arithmetic.
    $display("last=%0d before=%0d", q[$], q[$-1]);
    q[$-1] = 203;
    $display("%p", q);
    x = q[$-w33];
    $display("wide offset from $: %0d", x);

    // insert and delete take the same index: a wide or unknown one changes nothing.
    out = q;
    q.insert(w33, 55);
    q.insert(hi, 56);
    q.insert(ux, 57);
    q.delete(w33);
    q.delete(w65);
    q.delete(u32);
    $display("methods ignored %p", q);
    q.insert(1, 58);
    q.delete(q.size() - 1);
    $display("methods valid %p", q);
    q = out;

    // Slices clip a bound of any width: a < 0 reads as 0 and b > $ as $.
    out = q[1:2];       $display("slice %p", out);
    out = q[0:100];     $display("clipped high %0d", out.size());
    out = q[-3:1];      $display("clipped low %p", out);
    out = q[0:w33];     $display("wide high %0d", out.size());
    out = q[0:w65];     $display("wide high 65 %0d", out.size());
    out = q[1:u32];     $display("unsigned high %p", out);
    out = q[lneg:1];    $display("wide low 64 %p", out);
    out = q[m65:1];     $display("wide low 65 %p", out);
    out = q[hi:3];      $display("wide left beyond $ %0d", out.size());
    out = q[w33:w33];   $display("wide both %0d", out.size());
    out = q[m65:w65];   $display("wide both ends %0d", out.size());
    out = q[ux:2];      $display("x bound %0d", out.size());
    out = q[2:ux];      $display("x bound right %0d", out.size());
    $finish;
  end
endmodule
