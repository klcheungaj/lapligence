// IEEE 1800-2009 23.3.3.7 L41592-41598, L41606-41607 (SystemVerilog-1800-2009.txt):
// "It is permissible to merge the dominating and dominated nets into a single
// net, whose type shall be that of the dominating net." "The simulated net
// shall take the delay specified for the dominating net."
// 23.3.3.1 L41461-41462: an input net the child drives is coerced to inout.
// Decision: a port whose sink has a driver of its own merges also when a side
// has a net delay. The simulated net takes the dominating declaration's delay
// and, when that declaration has none, the outermost declared delay. A port
// whose sink only the port drives stays a link: each side keeps its timing.
`timescale 1ns/1ns

module drive_in(n);
  parameter int D = 3;
  input n;
  wire #D n;
  logic d = 1'bz;
  assign n = d;
endmodule

module plain_in(input wire n);
  logic d = 1'bz;
  assign n = d;
endmodule

module plain_out(output wire o);
  logic d = 1'bz;
  assign o = d;
endmodule

module delayed_inout(n);
  parameter int D = 2;
  inout n;
  wire #D n;
  logic d = 1'bz;
  assign n = d;
endmodule

module chained(inout wire n);
  delayed_inout #(3) leaf(.n(n));
endmodule

module undriven_in(n);
  input n;
  wire #3 n;
endmodule

module undriven_out(o);
  output o;
  wire #1 o;
  logic d = 1'b0;
  assign o = d;
endmodule

module cells_in(input wire [1:0] n);
  logic [1:0] d = 2'bzz;
  assign n = d;
endmodule

module tb;
  logic x;
  wire #2 outer_only;      // merged, delay 2
  wire #1 both;            // merged, the external declaration dominates
  wire inner_only;         // merged, the formal's delay
  wire through;            // merged through two levels, the leaf's delay
  wire #2 output_side;     // merged, another driver on the actual
  wire [1:0] bits;         // one bit merged with a delayed formal
  wire [1:0] #2 delayed_bits;
  wire [1:0] #2 cells [0:1];
  wire [1:0] lone [0:1];   // a cell only the port connects
  wire #1 link_in;         // the sink has no driver: each side keeps its delay
  wire #3 link_out;

  assign outer_only = x;
  assign both = x;
  assign inner_only = x;
  assign through = x;
  assign output_side = x;
  assign bits = {x, x};
  assign delayed_bits = {x, x};
  assign cells[0] = {x, x};
  assign link_in = x;

  plain_in u0(.n(outer_only));
  drive_in #(3) u1(.n(both));
  drive_in #(2) u2(.n(inner_only));
  chained u3(.n(through));
  plain_out u4(.o(output_side));
  drive_in #(3) u5(.n(bits[0]));
  plain_in u6(.n(delayed_bits[0]));
  cells_in u7(.n(cells[0]));
  cells_in u8(.n(lone[1]));
  undriven_in u9(.n(link_in));
  undriven_out u10(.o(link_out));

  task automatic show(string at);
    $display("%s %b%b %b%b %b%b %b%b%b %b%b", at, outer_only, u0.n, both, u1.n, inner_only, u2.n,
             through, u3.n, u3.leaf.n, output_side, u4.o);
    $display("%s %b/%b %b/%b %b/%b %b/%b %b/%b %b/%b", at, bits, u5.n, delayed_bits, u6.n, cells[0],
             u7.n, lone[1], u8.n, link_in, u9.n, link_out, u10.o);
  endtask

  initial begin
    x = 1'b1;
    #1 show("t1");
    #1 show("t2");
    #1 show("t3");
    #1 show("t4");
    x = 1'b0;
    u8.d = 2'b10;
    #1 show("t5");
    #1 show("t6");
    #1 show("t7");
    #1 show("t8");
    u0.d = 1'b1;
    u1.d = 1'b1;
    u2.d = 1'b1;
    u4.d = 1'b1;
    u5.d = 1'b1;
    u6.d = 1'b1;
    u7.d = 2'b11;
    #4 show("t12");
    $finish(0);
  end
endmodule
