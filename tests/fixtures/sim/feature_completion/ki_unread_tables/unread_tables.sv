// Objects that are declared and only named by $typename, which evaluates
// nothing (IEEE 1800-2009 20.6.1): the model must not define a constant table
// that nothing reads, because C compilers warn about it. The generated-C
// frame lint compiles this model without warnings.
package pk;
  typedef struct packed {logic [3:0] a; logic b;} ps_t;
  typedef struct {byte a; int b; string s; real r;} us_t;
  typedef union tagged {int A; void B;} ut_t;
  typedef enum {X0, X1, X9 = 9} en_t;
  class C; int v; string s; int q[$]; us_t u; event e[2]; endclass
  class P #(type T = int); T v; T aa[string]; endclass
endpackage
interface ifc(input clk);
  logic [3:0] d;
  event ie[2];
  clocking cb @(posedge clk); input d; endclocking
  modport m(input d);
endinterface
module tb;
  import pk::*;
  logic clk;
  ifc fi(clk);
  ifc fia[2](clk);
  virtual ifc vi, via[2], viq[$];
  event ev1, ea[3], ea2[2][3], eq[$], ed[], eaa[string];
  // Read through its elements: its tables stay.
  event er[2];
  logic [3:0] l, la[4];
  logic lb, la2[2][3];
  wire [3:0] wn, wa[3];
  int q[$], qb[$:5], d[], aa[string], dq[][$], qa[$][3];
  string sq[$], sa[3], saa[string];
  real rq[$], ra[3];
  chandle ch, ca[3];
  ps_t psa[2], psq[$];
  us_t us, usa[2], usq[$], usd[], usaa[int];
  en_t ena[3], enq[$];
  ut_t ut;
  C c, ca2[2], cq[$];
  P #(us_t) p;
  P #(string) p2;
  mailbox #(int) mba[2];
  semaphore smq[$];
  process prq[$];
  covergroup cg @(posedge clk); coverpoint l; endgroup
  cg cgi;
  struct {int x; logic [3:0] y[2]; int q[$];} as;
  class D; rand int x; randc bit [3:0] y; constraint cc {x > 0;} endclass
  D dd;
  parameter int PA[3] = '{1, 2, 3};
  localparam us_t PU = '{1, 2, "s", 1.0};
  const int KA[2] = '{4, 5};
  clocking cb @(posedge clk); input l; endclocking
  sequence sq1; l[0] ##1 lb; endsequence
  property pp; @(posedge clk) l[0] |-> lb; endproperty
  function automatic int fa(int x[$], ref us_t u, ref logic [3:0] z[4]);
    int loc[$]; us_t lu; return $bits(x[0]);
  endfunction
  task automatic ta(input int x[], event e, output string o[2]);
    int loc[string]; us_t lu[2];
  endtask
  function int fs(int x);
    int loc[3]; us_t lu; string ls[2]; return $bits(loc);
  endfunction
  int which = 1;
  always @(er[which]) $display("er %0d", which);
  initial begin
    static int bl[3];
    automatic int bq[$];
    automatic us_t bu;
    $display("%s|%s|%s|%s|%s", $typename(ea), $typename(ea2), $typename(eq), $typename(ed), $typename(eaa));
    $display("%s|%s|%s|%s", $typename(ev1), $typename(er), $typename(bl), $typename(bq));
    $display("%s|%s|%s|%s|%s", $typename(q), $typename(qb), $typename(d), $typename(aa), $typename(dq));
    $display("%s|%s|%s|%s|%s", $typename(qa), $typename(sq), $typename(sa), $typename(saa), $typename(rq));
    if ($typename(usa) == $typename(usq) || $typename(us) == $typename(ena) || $typename(c) == $typename(cq)
        || $typename(p) == $typename(p2) || $typename(mba) == $typename(smq) || $typename(via) == $typename(viq)
        || $typename(PA) == $typename(PU) || $typename(KA) == $typename(wa) || $typename(la2) == $typename(as)
        || $typename(dd) == $typename(ca) || $typename(ut) == $typename(prq) || $typename(bu) == $typename(q))
      $display("equal");
    #1 -> er[which];
    #1 $finish;
  end
endmodule
