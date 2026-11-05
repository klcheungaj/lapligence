// IEEE 1800-2009 23.3.3.7 L41596-41598, L41606 (SystemVerilog-1800-2009.txt):
// "It is permissible to merge the dominating and dominated nets into a single
// net, whose type shall be that of the dominating net." "The simulated net
// shall take the delay specified for the dominating net."
// Decision: an output port whose actual (a whole net, a net-array cell or a
// bit of a vector) declares a net delay and whose formal declares none merges,
// so the formal shows the delayed net. A formal with a delay of its own keeps
// its link and the two delays add. An inout connection alone does not make a
// delayed input formal differ from its source: without a driver besides the
// port on the inout chain the input stays a link, so the source is not delayed.
`timescale 1ns/1ps

module drive(o);
  output o;
  parameter bit V = 1;
  logic r = 1'bz;
  assign o = r;
  initial #5 r = V;
  initial #15 r = 1'bz;
endmodule

module drive_delayed(o);
  output o;
  wire #4 o;
  logic r = 1'bz;
  assign o = r;
  initial #5 r = 1'b1;
endmodule

module pass_through(n);
  inout n;
endmodule

module chain(i);
  input i;
  wire #3 i;
  pass_through s(.n(i));
endmodule

module tb;
  wire #2 w;
  wire #2 wd;
  wire #2 n[1:0];
  wire #2 nd[1:0];
  wire plain[1:0];
  wire [1:0] #2 v;
  wire #2 pd[1:0];
  logic s = 1'bz;
  logic x;
  wire g;
  assign g = x;

  drive a(.o(w));
  drive b(.o(wd));
  assign wd = s;
  drive c(.o(n[0]));
  drive #(0) d(.o(nd[0]));
  assign nd[0] = s;
  drive e(.o(plain[0]));
  drive f(.o(v[0]));
  drive_delayed h(.o(pd[0]));
  chain ch(.i(g));

  // Sampled between events, so no line depends on the order of processes.
  string last = "";
  initial begin
    #0.5;
    forever begin
      string line;
      line = $sformatf("w=%b/%b wd=%b/%b n=%b/%b nd=%b/%b plain=%b/%b v=%b/%b pd=%b/%b g=%b/%b",
                       w, a.o, wd, b.o, n[0], c.o, nd[0], d.o, plain[0], e.o, v[0], f.o,
                       pd[0], h.o, g, ch.i);
      if (line != last) $display("%0d %s", $time, line);
      last = line;
      #1;
    end
  end

  initial begin
    x = 1'b1;
    #30 s = 1'b0;
    x = 1'b0;
    #10 s = 1'bz;
    #10 $finish;
  end
endmodule
