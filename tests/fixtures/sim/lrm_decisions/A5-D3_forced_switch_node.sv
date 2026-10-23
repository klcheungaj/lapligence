// Decision A5-D3: a forced switch node is a source of its forced value at
// strong strength: nothing arriving through its switches changes it, its
// neighbours see the forced value (a forced z isolates them), and release
// recomputes the network from its drivers.
//
// IEEE 1800-2009 10.6.2 (SystemVerilog-1800-2009.txt L13397-13399): "A force
// procedural statement on a net shall override all drivers of the net-gate
// outputs, module outputs, and continuous assignments-until a release
// procedural statement is executed on the net. When released, the net shall
// immediately be assigned the value determined by the drivers of the net."
`timescale 1ns/1ns
module tb;
  wire a, b, c;
  reg ra;
  assign (pull0, pull1) a = ra;
  tran t1(a, b);
  tran t2(b, c);
  initial begin
    ra = 0; #1
    $display("1 %v %v %v", a, b, c);
    force b = 1; #1
    $display("2 %v %v %v", a, b, c);
    ra = 1; #1
    $display("3 %v %v %v", a, b, c);
    release b; #1
    $display("4 %v %v %v", a, b, c);
    force a = 1'bz; #1
    $display("5 %v %v %v", a, b, c);
    release a; #1
    $display("6 %v %v %v", a, b, c);
    $finish;
  end
endmodule
