// Net and variable ports whose sink merges with its source (decisions
// A21-D1..A21-D3): a process that changes a source reads the new value
// through every sink in the same time step, down a chain of instances and
// up through output ports. A model that keeps the VPI catalog keeps every
// port whose source is a variable a continuous assignment, so those sinks
// follow one update later; its net ports merge.
module leaf(input wire c, input wire d, output wire q, output logic r);
  assign q = d;
  always @(posedge c) begin
    r = d;
    $display("output variable: r=%b tb.r=%b", r, tb.r);
  end
endmodule

module mid(input c, input d, output q, output logic r);
  leaf l(.c(c), .d(d), .q(q), .r(r));
endmodule

module tb;
  logic c, d, r;
  wire q;
  mid u(.c(c), .d(d), .q(q), .r(r));
  initial begin
    c = 1'b0;
    d = 1'b0;
    #1 d = 1'b1;
    $display("input chain: u.d=%b u.l.d=%b", u.d, u.l.d);
    #0 $display("output net: u.l.q=%b q=%b", u.l.q, q);
    #1 c = 1'b1;
    #1 $finish(0);
  end
endmodule
