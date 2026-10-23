// Decision A5-D5: a net delay on a switch node delays only that net's own
// value: the network is solved from the node's drivers, so the nodes beyond
// it change at once and only the delayed net shows the change after its
// delay.
//
// IEEE 1800-2009 10.3.3 (SystemVerilog-1800-2009.txt L12911-12912): "This
// syntax, called a net delay, means that any value change that is to be
// applied to wireA by some other statement shall be delayed for ten time units
// before it takes effect"; 28.8 L47365: "The bidirectional pass switches shall
// not delay signals propagating through them."
`timescale 1ns/1ns
module tb;
  wire a; wire #3 b; wire c;
  reg ra;
  assign (strong0, strong1) a = ra;
  tran t1(a, b);
  tran t2(b, c);
  initial begin
    ra = 0;
    #5 $display("5 %v %v %v", a, b, c);
    ra = 1;
    #1 $display("6 %v %v %v", a, b, c);
    #2 $display("8 %v %v %v", a, b, c);
    #1 $display("9 %v %v %v", a, b, c);
    $finish;
  end
endmodule
