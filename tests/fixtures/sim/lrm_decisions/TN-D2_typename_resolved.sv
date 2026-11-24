// Decision TN-D2: $typename names the resolved type of its argument: typedefs
// and type parameters of built-in types are resolved at every level (element
// types, index types, class type parameters), and the text does not depend on
// the other spellings of the type in the design.
//
// IEEE 1800-2009 20.6.1 (SystemVerilog-1800-2009.txt):
// L34538: "The $typename system function returns a string that represents the
//   resolved type of its argument."
// L34548: "a) A typedef that creates an equivalent type is resolved
//   back to built-in or user-defined types."
// The names of user-defined types are not part of the case (TN-D1); the
// fixture feature_completion/ki_typename_resolved has them.
package pk;
  typedef logic [3:0] nib_t;
  class P #(type T = int);
    T v;
    static function string nm();
      return $typename(T);
    endfunction
    function string nv();
      return $typename(v);
    endfunction
  endclass
endpackage
module sub #(type T = int, parameter int D = 1) ();
  T x;
  localparam string S = $typename(T);
  initial #D $display("sub %s|%s|%s", S, $typename(T), $typename(x));
endmodule
module tb;
  import pk::*;
  typedef byte byte_t;
  typedef byte_t arr_t [0:3];
  typedef byte_t q_t [$];
  typedef nib_t [1:0] nn_t;
  typedef nn_t nnn_t [2];
  typedef real real_t;
  typedef string str_t;
  typedef chandle ch_t;
  typedef logic signed [4095:0] wide_t;
  // The first spellings of these types in the design.
  P #(wide_t) pw;
  P #(nib_t) pn;
  P #(str_t) ps;
  wide_t w0;
  logic signed [4095:0] w;
  logic [3:0] nib;
  byte_t b;
  byte plain;
  arr_t a;
  q_t q;
  nn_t nn;
  nnn_t nnn;
  real_t r;
  str_t st;
  ch_t c;
  nib_t na [byte_t];
  nib_t nq [$][3];
  logic [3:0] lq [$][3];
  wire nib_t wn;
  mailbox #(nib_t) mb;
  mailbox #(logic signed [4095:0]) mw;
  localparam string LB = $typename(b), LW = $typename(w);
  sub #(nib_t, 1) s0();
  sub #(logic [3:0], 2) s1();
  sub #(arr_t, 3) s2();
  sub #(.D(4)) s3();
  initial begin
    $display("%s", $typename(w));
    $display("%s", $typename(w0));
    $display("%s", $typename(nib));
    $display("%s", $typename(b));
    $display("%s", $typename(plain));
    $display("%s", $typename(a));
    $display("%s", $typename(q));
    $display("%s", $typename(nn));
    $display("%s", $typename(nnn));
    $display("%s", $typename(r));
    $display("%s", $typename(st));
    $display("%s", $typename(c));
    $display("%s", $typename(na));
    $display("%s", $typename(nq));
    $display("%s", $typename(lq));
    $display("%s", $typename(wn));
    $display("%s", $typename(mb));
    $display("%s", $typename(mw));
    $display("%s|%s", LB, LW);
    $display("%s|%s|%s|%s", P#(nib_t)::nm(), P#(wide_t)::nm(), P#(str_t)::nm(), P#()::nm());
    pn = new;
    pw = new;
    $display("%s|%s", pn.nv(), pw.nv());
    $display("%s|%s|%s", $typename(byte_t), $typename(nn_t), $typename(q_t));
    $display("%s|%s|%s", $typename(a[1]), $typename(nn[0]), $typename(nq[0]));
    $display("%s|%s|%s", $typename({b, b}), $typename(b == 1), $typename(a[1:2]));
    #9 $finish;
  end
endmodule
