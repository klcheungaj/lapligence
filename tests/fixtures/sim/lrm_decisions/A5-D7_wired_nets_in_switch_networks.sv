// Decision A5-D7: nets that `tran` or `rtran` join resolve as one net of the
// wired type when one of them is a wand/triand (or wor/trior) net: a plain
// wire there resolves its own driver and what arrives through the switch
// with the wired function. A net reached only through `tranif*`/`rtranif*`
// resolves its own drivers and what arrives with its own resolution: a wired
// net with its wired function, a plain net with the wire rule.
//
// IEEE 1800-2009 28.12.4 (SystemVerilog-1800-2009.txt L48170-48171): "The net
// types triand, wand, trior, and wor shall resolve conflicts when multiple
// drivers have the same strength. These net types shall resolve signal values
// by treating signals as inputs of logic functions." 23.3.3.7 (L41596-41597):
// "It is permissible to merge the dominating and dominated nets into a single
// net, whose type shall be that of the dominating net." The text does not
// say how a wired net resolves across a bidirectional switch; llg treats an
// uncontrolled switch like the port collapsing of Table 23-1 (a wired type
// dominates wire) and a controlled one as joining two nets. Some simulators
// resolve a plain wire joined by tran to a wand with the wire rule in some
// designs; llg applies one rule.
`timescale 1ns/1ns
module tb;
  wor o; wire b; reg e1, e2, f;
  assign o = e1; assign o = e2;
  assign b = f;
  tran t1(o, b);
  wand wc; wire cw1, cw2; reg k1, k2;
  assign cw1 = k1; assign cw2 = k2;
  tran t2(cw1, wc); tran t3(cw2, wc);
  wand [3:0] wv; wire [3:0] av; reg [3:0] v0, v1, va; reg c;
  assign wv = v0; assign wv = v1; assign av = va;
  tranif1 tv[3:0](wv, av, c);
  initial begin
    e1 = 1'b0; e2 = 1'b1; f = 1'bz;
    k1 = 1'b1; k2 = 1'b0;
    v0 = 4'b1100; v1 = 4'b1010; va = 4'bzz01; c = 1'b1;
    #1 $display("1 %v %v | %v %v %v | %b %b", o, b, wc, cw1, cw2, wv, av);
    f = 1'b0; k1 = 1'bx;
    #1 $display("2 %v %v | %v %v %v", o, b, wc, cw1, cw2);
    e1 = 1'bz; e2 = 1'bz; k1 = 1'b1; k2 = 1'b1;
    #1 $display("3 %v %v | %v %v %v", o, b, wc, cw1, cw2);
    $finish;
  end
endmodule
