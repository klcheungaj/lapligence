// SIM-023 A02 (IEEE 1364-2001 forms): hierarchical targets with live RHS
// inputs and ordinary drivers continuing underneath.
module leaf (i, o);
  input [3:0] i;
  output [3:0] o;
  reg [3:0] v;
  assign o = i;
endmodule

module mid (i);
  input [3:0] i;
  leaf l (.i(i), .o());
  reg [3:0] mv;
  task poke;
    input [3:0] val;
    force tb.top_v = val + tb.s;
  endtask
endmodule

module tb;
  reg [3:0] d, s, top_v;
  wire [3:0] o0, o1;
  mid m (.i(d));
  leaf la[1:0] (.i(d), .o({o1, o0}));
  genvar g;
  generate
    for (g = 0; g < 2; g = g + 1) begin : gen
      reg [3:0] gv;
      wire [3:0] gw;
      assign gw = d + g;
    end
  endgenerate
  initial begin
    d = 4'h1; s = 4'h2; top_v = 4'h0;
    force m.l.v = s;
    force m.l.o = d + s;
    force gen[1].gw = s;
    force gen[0].gv = m.l.o;
    force la[0].o = 4'he;
    force m.mv = gen[1].gw;
    m.poke(4'h4);
    #1 $display("1 %h %h %h %h %h %h %h %h %h", m.l.v, m.l.o, gen[1].gw, gen[0].gv,
                o0, o1, m.mv, top_v, gen[0].gw);
    s = 4'h5;
    #1 $display("2 %h %h %h %h %h %h %h %h %h", m.l.v, m.l.o, gen[1].gw, gen[0].gv,
                o0, o1, m.mv, top_v, gen[0].gw);
    d = 4'h7;
    top_v = 4'h0;
    #1 $display("3 %h %h %h %h %h %h %h %h %h", m.l.v, m.l.o, gen[1].gw, gen[0].gv,
                o0, o1, m.mv, top_v, gen[0].gw);
    release gen[0].gv;
    release m.mv;
    release m.l.v;
    release m.l.o;
    release gen[1].gw;
    release la[0].o;
    release top_v;
    #1 $display("4 %h %h %h %h %h %h %h %h %h", m.l.v, m.l.o, gen[1].gw, gen[0].gv,
                o0, o1, m.mv, top_v, gen[0].gw);
    s = 4'h9;
    #1 $display("5 %h %h %h %h %h %h %h %h %h", m.l.v, m.l.o, gen[1].gw, gen[0].gv,
                o0, o1, m.mv, top_v, gen[0].gw);
    $finish;
  end
endmodule
