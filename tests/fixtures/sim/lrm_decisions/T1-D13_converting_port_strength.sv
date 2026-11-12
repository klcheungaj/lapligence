// IEEE 1800-2009 23.3.3 L41451-41452 (SystemVerilog-1800-2009.txt): "Each
// port connection shall be a continuous assignment of source to sink, where
// one connected item shall be a signal source and the other shall be a signal
// sink."
// 23.3.3.7 L41596-41597: "It is permissible to merge the dominating and
// dominated nets into a single net, whose type shall be that of the dominating
// net."
// The text does not say whether a port between nets of different widths
// merges, so which strength the sink shows is not specified.
// Decision: an input or output port between two nets of different widths
// joins the bits both sides have, least significant bits aligned, so the sink
// shows the source's strength and an event on it occurs on the source's
// strength-only changes; the sink's extra bits take the converted value
// (a widening's zero extension, strong). A port whose signed source widens,
// and one whose source is the sink of a widening port, stay continuous
// assignments with strong strength. A driver of the sink inside the child
// then also drives the source's joined bits.
`timescale 1ns/1ns

// Widening input: the formal's low bit is the actual, its high bit the
// zero extension. Read by %v and by an event.
module wide_in(input wire [1:0] i);
  string seen = "";
  always @(i) seen = $sformatf("i=%b %v %v", i, i[1], i[0]);
endmodule

// Narrowing input, read by an implicit event list only.
module narrow_in(input wire n);
  string seen = "";
  always @* seen = $sformatf("n=%b", n);
endmodule

// A signed actual widens with its sign bit.
module signed_in(input wire [2:0] s);
  string seen = "";
  always @(s) seen = $sformatf("s=%b %v %v %v", s, s[2], s[1], s[0]);
endmodule

// Two converting ports in a row: the inner formal shows the outer net.
module leaf(input wire [2:0] l);
  string seen = "";
  always @(l) seen = $sformatf("l=%b %v", l, l[0]);
endmodule
module mid(input wire [1:0] m);
  leaf lf(m);
endmodule

// A narrowing port, then a widening one: the leaf shows the outer net.
module leaf2(input wire [1:0] l);
  string seen = "";
  always @(l) seen = $sformatf("l=%b %v", l, l[0]);
endmodule
module mid_narrow(input wire m);
  leaf2 lf(m);
endmodule

// Widening output: the actual's high bit is the formal's zero extension.
module wide_out(output wire o, input wire r, input wire s);
  assign (weak0, weak1) o = r;
  assign (strong0, strong1) o = s;
endmodule

// A widening input whose formal has a driver of its own.
module driven_in(input wire [1:0] d, input wire e);
  bufif1 (pull0, pull1) g (d[0], e, e !== 1'bz);
  string seen = "";
  always @(d) seen = $sformatf("d=%b %v", d, d[0]);
endmodule

module tb;
  wire w, x;
  wire [1:0] v;
  wire signed [1:0] vs;
  wire [1:0] q;
  reg a = 1'b0, b = 1'bz, e = 1'bz;
  reg r = 1'b0, s = 1'bz;
  assign (weak0, weak1) w = a;
  assign (strong0, strong1) w = b;
  bufif1 (weak0, weak1) va (v[0], a, 1'b1);
  bufif1 (strong0, strong1) vb (v[0], b, b !== 1'bz);
  assign v[1] = 1'b1;
  bufif1 (weak0, weak1) vsa (vs[1], a, 1'b1);
  bufif1 (strong0, strong1) vsb (vs[1], b, b !== 1'bz);
  assign vs[0] = 1'b1;
  assign (weak0, weak1) x = a;

  wide_in wi(w);
  narrow_in ni(v);
  signed_in si(vs);
  mid md(w);
  mid_narrow mn(v);
  wide_out wo(q, r, s);
  driven_in di(x, e);
  string tq = "";
  always @(q) tq = $sformatf("q=%b %v %v", q, q[1], q[0]);

  initial begin
    #2 b = 1'b0;
    #2 b = 1'bz;
    #2 a = 1'bx;
    #2 b = 1'b1;
    #2 b = 1'bz; a = 1'b0;
    #2 s = 1'b1;
    #2 s = 1'bz; r = 1'b1;
    #2 e = 1'b1;
    #2 a = 1'b1;
    #2 a = 1'bz;
    #2 $display("%0t end", $time);
    $finish;
  end

  initial begin
    #1;
    forever begin
      $display("%0t: wi{%s} ni{%s} si{%s} lf{%s} nl{%s} wo{%s} di{%s}", $time, wi.seen,
               ni.seen, si.seen, md.lf.seen, mn.lf.seen, tq, di.seen);
      wi.seen = ""; ni.seen = ""; si.seen = ""; md.lf.seen = ""; mn.lf.seen = "";
      tq = ""; di.seen = "";
      #2;
    end
  end
endmodule
