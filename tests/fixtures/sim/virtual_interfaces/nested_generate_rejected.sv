// A member of a generate block inside an interface, through a virtual
// interface, is rejected with its location: it is not a member of the
// interface itself (`x` below), nor of an interface instance nested in it.
`timescale 1ns/1ns
interface outer_if;
  logic x;
  if (1) begin : g
    logic x;
  end
endinterface
module tb;
  outer_if o();
  virtual outer_if vo;
  initial begin
    vo = o;
    vo.g.x = 1;
    #1 $display("%b %b", o.x, o.g.x);
    $finish;
  end
endmodule
