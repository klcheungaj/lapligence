// Notifier values (Table 31-13, V Table 61), a sequential UDP consuming the
// notifier, a notifier shared by checks violating at one event, duplicate
// checks and events at time 0 (IEEE 1800-2009 31.6, IEEE 1364-2001 15.5;
// see readme.md).
`timescale 1ns/1ns
primitive dff_n(q, clock, data, notifier);
  output q; reg q;
  input clock, data, notifier;
  table
  // clock data notifier : q : q+
     r     0    ?        : ? : 0;
     r     1    ?        : ? : 1;
     f     ?    ?        : ? : -;
     ?     *    ?        : ? : -;
     ?     ?    *        : ? : x;
  endtable
endprimitive
module ff(output q, input c, input d);
  reg notifier;
  dff_n (q, c, d, notifier);
  specify
    $setup(d, posedge c, 10, notifier);
    $hold(posedge c, d, 2, notifier);
    $width(posedge c, 20, 0, notifier);
  endspecify
endmodule
module cx(input d, input c); reg n; specify $setup(d, posedge c, 10, n); endspecify endmodule
module c1(input d, input c); reg n; initial n = 1'b1; specify $setup(d, posedge c, 10, n); endspecify endmodule
module cz(input d, input c); reg n; initial n = 1'bz; specify $setup(d, posedge c, 10, n); endspecify endmodule
module sh(input d, input e, input c); reg n;
  specify $setup(d, posedge c, 10, n); $setup(e, posedge c, 10, n); endspecify
endmodule
module du(input d, input c); reg n10, n5;
  specify $setup(d, posedge c, 10, n10); $setup(d, posedge c, 5, n5); endspecify
endmodule
module dr(input d, input c); reg n10, n5;
  specify $setup(d, posedge c, 5, n5); $setup(d, posedge c, 10, n10); endspecify
endmodule
module tb;
  reg c, d, sd, sc, tzd, tzc;
  wire q;
  ff u(q, c, d);
  cx a(sd, sc); c1 b(sd, sc); cz e(sd, sc); sh s(sd, sd, sc); du k(sd, sc); dr r(sd, sc);
  cx t(tzd, tzc);
  initial
    $monitor("%0t q=%b notifier=%b x=%b one=%b z=%b shared=%b duplicate=%b%b reordered=%b%b zero=%b",
             $time, q, u.notifier, a.n, b.n, e.n, s.n, k.n10, k.n5, r.n10, r.n5, t.n);
  initial begin
    tzd = 1; tzc = 0;                 // time 0: no timestamp
    #5 tzc = 1;                       // 5: no violation
  end
  initial begin
    c = 0; d = 0; sd = 0; sc = 0;
    #50 d = 1;
    #50 c = 1;                        // 100: q = 1
    #10 c = 0;                        // 110: width 10 < 20
    #20 d = 0;                        // 130
    #5 c = 1;                         // 135: setup 5 < 10, q = 0 then x
    #25 c = 0;                        // 160
    #30 c = 1;                        // 190: q = 0, no violation
    #1 d = 1;                         // 191: hold 1 < 2
    #29 c = 0;
    repeat (3) begin
      #12 sd = ~sd;
      #7 sc = 1;                      // every check of a, b, e, s, k and r violates
      #1 sc = 0;
    end
    #10 $finish;
  end
endmodule
