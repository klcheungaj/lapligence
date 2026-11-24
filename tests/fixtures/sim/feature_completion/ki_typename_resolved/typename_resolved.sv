// $typename of user-defined types (IEEE 1800-2009 20.6.1, decisions TN-D1 and
// TN-D2): the innermost typedef names a structure, union or enumeration
// through other typedefs and type parameters, a class is named with its scope,
// and the text does not depend on the other spellings of the type. A constant
// $typename gives the same text.
package pk;
  typedef logic [3:0] nib_t;
  typedef struct packed { nib_t a; logic b; } ps_t;
  typedef enum {X0, X1, X9 = 9} en_t;
  class P #(type T = int); T v; static function string nm(); return $typename(T); endfunction
    function string nv(); return $typename(v); endfunction endclass
  class C; endclass
endpackage
interface ifc #(int W = 3) (input clk); logic [W-1:0] d; modport m (input d); endinterface
module sub #(type T = int, parameter int D = 1) (); T x; localparam string S = $typename(T); initial #D $display("sub %s|%s|%s", S, $typename(T), $typename(x)); endmodule
module tb;
  import pk::*;
  typedef byte byte_t; typedef byte_t bq_t [$]; typedef byte_t arr_t [0:3];
  typedef struct { byte_t a; int b; } us_t; typedef struct { us_t u; nib_t n[2]; } nest_t;
  typedef union packed { logic [3:0] a; nib_t b; } up_t;
  typedef real rl_t; typedef string st_t; typedef nib_t [1:0] nn_t; typedef C ch_t; typedef virtual ifc #(5) vi_t; typedef chandle cp_t;
  logic signed [4095:0] w; byte_t b; arr_t a; bq_t q; us_t us; nest_t ns; ps_t ps; en_t en; up_t up; rl_t r; st_t st; nn_t nn; ch_t ch; vi_t vi; cp_t cp;
  nib_t na [byte_t]; nib_t nq[$][3]; ps_t psa[2]; en_t ena[$]; C cq[$]; P #(nib_t) pn; P #(C) pc; P #(logic signed [4095:0]) pw; P#(P#(C)) ppc;
  enum {A0, A1} aen; struct {int x;} as; struct packed {logic z;} ap; typedef enum logic [1:0] {Z0} zen_t; zen_t zen;
  mailbox #(nib_t) mb; semaphore sm; process pr; event ev; 
  localparam string L1 = $typename(b), L2 = $typename(a), L3 = $typename(us), L4 = $typename(en), L5 = $typename(ps);
  wire nib_t wn; tri logic [2:0] tl; ifc #(5) fi(w[0]);
  sub #(nib_t, 1) s0(); sub #(C, 2) s1(); sub #(us_t, 3) s2(); sub #(.D(4)) s3(); sub #(logic [3:0], 5) s4();
  initial begin
    $display("%s", $typename(w)); $display("%s", $typename(b)); $display("%s", $typename(a)); $display("%s", $typename(q)); $display("%s", $typename(us)); $display("%s", $typename(ns));
    $display("%s", $typename(ps)); $display("%s", $typename(en)); $display("%s", $typename(up)); $display("%s", $typename(r)); $display("%s", $typename(st)); $display("%s", $typename(nn));
    $display("%s", $typename(ch)); $display("%s", $typename(vi)); $display("%s", $typename(cp)); $display("%s", $typename(na)); $display("%s", $typename(nq)); $display("%s", $typename(psa));
    $display("%s", $typename(ena)); $display("%s", $typename(cq)); $display("%s", $typename(pn)); $display("%s", $typename(pc)); $display("%s", $typename(pw)); $display("%s", $typename(ppc));
    $display("%s", $typename(aen)); $display("%s", $typename(as)); $display("%s", $typename(ap)); $display("%s", $typename(zen)); 
    $display("%s", $typename(mb)); $display("%s", $typename(sm)); $display("%s", $typename(pr)); $display("%s", $typename(ev)); $display("%s", $typename(wn)); $display("%s", $typename(tl));
    $display("%s|%s|%s|%s|%s", L1, L2, L3, L4, L5);
    $display("%s|%s|%s|%s", P#(nib_t)::nm(), P#(C)::nm(), P#(logic signed [4095:0])::nm(), P#()::nm());
    $display("%s", $typename(byte_t)); $display("%s", $typename(us_t)); $display("%s", $typename(C)); $display("%s", $typename(P#(C))); $display("%s", $typename(P#(nib_t)::T));
    $display("%s", $typename(X1)); $display("%s", $typename(b + 1)); $display("%s", $typename("abc")); $display("%s", $typename(1.5)); $display("%s", $typename(us.a)); $display("%s", $typename(ps.a)); $display("%s", $typename({b, b})); $display("%s", $typename(a[1])); $display("%s", $typename(ns.u)); $display("%s", $typename($time)); $display("%s", $typename(b == 1));
    pn = new; pc = new; $display("%s|%s", pn.nv(), pc.nv());
    #9 $finish;
  end
endmodule
