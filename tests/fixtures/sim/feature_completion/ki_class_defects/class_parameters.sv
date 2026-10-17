// class-defects item 2: class parameters read inside methods. Expected lines
// are derived in readme.md.
`timescale 1ns/1ps
class P #(int W = 8, type T = logic [3:0]);
  localparam int L = W * 2;
  T v;
  function void h();
    T tmp = '1;
    $display("W=%0d L=%0d bits=%0d tmp=%0d", W, L, $bits(T), tmp);
  endfunction
  static function int sw();
    return W + 1;
  endfunction
  extern function int ex();
  extern static function int sx();
endclass

function int P::ex();
  return L + W;
endfunction

function int P::sx();
  T t = 0;
  return 2 * W + $bits(t);
endfunction

class Outer #(int W = 3, string TAG = "o");
  localparam int D = W + 1;
  int init_w = W * 10;
  static int sinit = D;
  class Inner;
    function int get();
      return W + D;
    endfunction
  endclass
  function int via_inner();
    Inner i = new;
    return i.get();
  endfunction
  function int dflt(int a = W);
    return a;
  endfunction
  function string tag();
    return TAG;
  endfunction
  static function int loop_sum();
    int s = 0;
    for (int i = 0; i < W; i++) s += i;
    return s;
  endfunction
  function int arr();
    int a[W];
    foreach (a[i]) a[i] = i + D;
    return $size(a) + a[W-1];
  endfunction
endclass

class Kid #(int K = 2) extends Outer #(K * 2);
  function int kw();
    return W + K;
  endfunction
endclass

module tb #(parameter int W = 99);
  class R #(int W = 4);
    rand bit [31:0] x;
    constraint lim {x < W;}
    function new();
      x = W;
    endfunction
    task show();
      #1 $display("%m W=%0d below=%0d", W, x < W);
    endtask
  endclass

  initial begin
    static P #(4) a = new;
    static P #(16, logic [7:0]) b = new;
    static P c = new;
    static Outer #(5, "five") o5 = new;
    static Outer o = new;
    static Kid #(3) k = new;
    static R #(3) r = new;
    static R #(7) q = new;
    int ok_r;
    int ok_q;
    a.h();
    b.h();
    c.h();
    $display("%0d %0d %0d", P#(4)::sw(), P#(16, logic [7:0])::sw(), a.ex());
    $display("%0d %0d", b.ex(), c.ex());
    $display("%0d %0d", P#(4)::sx(), P#(16, logic [7:0])::sx());
    $display("%0d %0d %0d %0d %s", o5.init_w, Outer#(5, "five")::sinit, o5.via_inner(),
             o5.dflt(), o5.tag());
    $display("%0d %0d %0d %0d %s", o.init_w, Outer#()::sinit, o.via_inner(), o.dflt(),
             o.tag());
    $display("%0d %0d %0d", Outer#(5, "five")::loop_sum(), o5.arr(), o.arr());
    $display("%0d %0d %0d", k.kw(), k.via_inner(), Outer#(5, "five")::W);
    ok_r = r.randomize();
    ok_q = q.randomize();
    $display("randomized %0d %0d module W=%0d", ok_r, ok_q, W);
    r.show();
    q.show();
    $finish;
  end
endmodule
