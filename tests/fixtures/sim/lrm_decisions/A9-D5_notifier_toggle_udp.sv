// Decision A9-D5: a violation changes the notifier x to 0, 0 to 1 and 1 to
// 0 and leaves z; the change is an ordinary variable update, so a
// sequential UDP with the notifier as an input reacts to it in the same time
// step.
//
// IEEE 1800-2009 31.6 Table 31-13 (SystemVerilog-1800-2009.txt
// L52048-52054): "x Either 0 or 1; 0 1; 1 0; z z". IEEE 1364-2001 Table 61
// (Verilog-1364-2001.txt L17000-17008) fixes x to 0.
`timescale 1ns/1ns
primitive latch_n(q, clock, data, notifier);
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
  latch_n (q, c, d, notifier);
  specify $setup(d, posedge c, 10, notifier); endspecify
endmodule
module c1(input d, input c); reg n; initial n = 1'b1; specify $setup(d, posedge c, 10, n); endspecify endmodule
module cz(input d, input c); reg n; initial n = 1'bz; specify $setup(d, posedge c, 10, n); endspecify endmodule
module tb;
  reg c, d;
  wire q;
  ff u(q, c, d); c1 o(d, c); cz z(d, c);
  initial $monitor("%0t q=%b notifier=%b one=%b z=%b", $time, q, u.notifier, o.n, z.n);
  initial begin
    c = 0; d = 1;
    #50 c = 1;                   // 50: q = 1
    #10 c = 0;
    #35 d = 0;
    #5 c = 1;                    // 100: setup 5 < 10: q = 0, then x
    #10 c = 0;
    #20 c = 1;                   // 130: q = 0
    #10 c = 0;
    #5 d = 1;
    #5 c = 1;                    // 150: violation again
    #10 $finish;
  end
endmodule
