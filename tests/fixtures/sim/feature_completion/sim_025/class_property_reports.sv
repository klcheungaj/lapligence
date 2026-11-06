// SIM-025 A01: deferred reports that name class state. A property lives in its
// object, not in an activation (SV 13.3.2 bars only variables of automatic
// subroutines), so `$monitor`, `$strobe` and the file variants may name it. The
// monitor watches the handle and the property selected through it: rebinding
// the handle or a handle property, or writing the property, reports, and a
// write to an object the path no longer selects does not (SV 21.2.3).
`timescale 1ns/1ns
module tb;
  class N;
    int v;
    N next;
    string s;
    real r;
    int q[$];
    int fa[2];
    logic [7:0] bits = 8'h10;
    static int count = 1;
    function new(int x);
      v = x;
    endfunction
  endclass
  class P #(type T = int, int W = 4);
    T v;
    bit [W-1:0] w;
    static int count = 1;
    function new(T x);
      v = x;
      w = '1;
    endfunction
  endclass

  N h, a, b, c;
  N hs[2];
  int i = 0;
  P #(byte, 8) p;

  initial begin
    a = new(1);
    b = new(2);
    a.next = b;
    h = a;

    // A monitor reads the end of the slot, through what the path selects then.
    $monitor("%0t v=%0d", $time, h.v);
    #1 a.v = 5;
    #1 h = b;
    // Written with the value it holds: nothing to report.
    #1 b.v = 2;
    // Another object with an equal value: the printed value does not change.
    #1 h = new(2);
    #1 h.v = 3;
    // The object the path no longer selects.
    #1 b.v = 9;

    // A chain moves with each link; string, real and container properties.
    #1 h = a;
    a.s = "x";
    a.r = 1.5;
    $monitor("%0t next=%0d s=%s r=%0.1f n=%0d e=%0d b=%h", $time, h.next.v, h.s, h.r,
             h.q.size(), h.fa[1], h.bits[3:0]);
    #1 b.v = 7;
    #1 a.next = new(8);
    #1 a.next.v = 80;
    #1 b.v = 70;
    #1 h.s = "yy";
    #1 h.r = 2.5;
    #1 h.q.push_back(1);
    #1 h.fa[1] = 4;
    #1 h.bits[0] = 1'b1;

    // A static property is fixed storage; a parameterized class is ordinary.
    #1 p = new(5);
    hs[0] = a;
    hs[1] = b;
    $monitor("%0t count=%0d pv=%0d pw=%0d pc=%0d sel=%0d", $time, N::count, p.v, p.w,
             P#(byte, 8)::count, hs[i].v);
    #1 N::count = 2;
    #1 p.v = 7;
    #1 p.w[0] = 1'b0;
    #1 P#(byte, 8)::count = 3;
    #1 i = 1;
    #1 hs[0].v = 1000;
    #1 hs[1] = a;

    // $monitoroff keeps the list; a change in between reports at $monitoron.
    #1 $monitoroff;
    #1 a.v = 6;
    #1 $monitoron;

    // Reports with a file argument, and strobes reading the end of the slot.
    #1 $monitoroff;
    c = new(1);
    c.next = b;
    $fmonitor(1, "%0t f=%0d", $time, c.next.v);
    #1 c.next.v = 11;
    #1 $fstrobe(1, "%0t fs=%0d", $time, c.v);
    c.v = 12;
    #1 $strobe("%0t st=%0d %0d", $time, h.v, h.next.v);
    h = b;
    h.next = c;
    #1 $finish(0);
  end
endmodule
