// SIM-026 A01: $sscanf into elements of queues, dynamic arrays, fixed string
// arrays and associative arrays: packed, string and real elements and bit
// selections of packed elements (IEEE 1800-2009 21.3.4.3). Element keys are
// evaluated once when the call starts, so `sq[i]` in E names `sq[0]` although
// the same call assigns `i` first. A failed conversion leaves its element
// unchanged (F), and an element of an invalid index is not created (G).
module tb;
  int q[$];
  logic [7:0] d[];
  int aa[int];
  int as[string];
  string sq[$];
  string sa[2];
  string sd[];
  string smap[string];
  real rq[$];
  real rd[];
  real rmap[string];
  logic [15:0] dq[];
  integer c, i;
  initial begin
    q = '{0, 0};
    d = new[3];
    sq = '{"", ""};
    sd = new[2];
    rq = '{0.0};
    rd = new[2];
    dq = new[2];
    c = $sscanf("5 ab 3 4", "%d %h %d %d", q[1], d[2], aa[7], as["k"]);
    $display("A c=%0d q=%0d,%0d d2=%h aa7=%0d aanum=%0d ask=%0d", c, q[0], q[1], d[2], aa[7],
             aa.num(), as["k"]);
    c = $sscanf("alpha beta gamma delta", "%s %s %s %s", sq[1], sa[0], sd[1], smap["m"]);
    $display("B c=%0d sq1=%s sa0=%s sd1=%s smapm=%s smapnum=%0d", c, sq[1], sa[0], sd[1],
             smap["m"], smap.num());
    c = $sscanf("1.5 -2.5 0.125", "%f %e %g", rq[0], rd[1], rmap["r"]);
    $display("C c=%0d rq0=%f rd1=%f rmapr=%f", c, rq[0], rd[1], rmap["r"]);
    c = $sscanf("a 5c 1", "%h %h %b", dq[1][3:0], dq[0][15:8], d[0][7]);
    $display("D c=%0d dq1=%h dq0=%h d0=%b", c, dq[1], dq[0], d[0]);
    i = 0;
    c = $sscanf("1 omega", "%d %s", i, sq[i]);
    $display("E c=%0d i=%0d sq0=%s sq1=%s", c, i, sq[0], sq[1]);
    c = $sscanf("9 x", "%d %f", q[0], rq[0]);
    $display("F c=%0d q0=%0d rq0=%f", c, q[0], rq[0]);
    c = $sscanf("7 w", "%d %s", d[5], sd[9]);
    $display("G c=%0d dsize=%0d sdsize=%0d", c, d.size(), sd.size());
    $finish;
  end
endmodule
