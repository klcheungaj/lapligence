// SIM-023 A02: hierarchical, alias and interface targets with live RHS inputs
// and ordinary drivers continuing underneath.
interface ifc;
  logic [3:0] iv;
  wire [3:0] iw;
  assign iw = iv;
endinterface

module leaf (input [3:0] i, output [3:0] o);
  logic [3:0] v;
  assign o = i;
endmodule

module mid (input [3:0] i);
  leaf l (.i(i), .o());
  logic [3:0] mv;
  task poke(input [3:0] val);
    force tb.top_v = val + tb.s;
  endtask
endmodule

class Forcer;
  task apply();
    force tb.cls_v = tb.s ^ 4'hf;
  endtask
  task undo();
    release tb.cls_v;
  endtask
endclass

module tb;
  logic [3:0] d, s, top_v, cls_v;
  wire [3:0] wa, wb;
  alias wa = wb;
  assign wb = d;
  mid m (.i(d));
  leaf la[1:0] (.i(d), .o());
  ifc itf ();
  for (genvar g = 0; g < 2; g++) begin : gen
    logic [3:0] gv;
    wire [3:0] gw;
    assign gw = d + g;
  end
  Forcer f;
  initial begin
    d = 4'h1; s = 4'h2; top_v = 4'h0; cls_v = 4'h0; itf.iv = 4'h3;
    f = new;
    force m.l.v = s;
    force m.l.o = d + s;
    force gen[1].gw = s;
    force gen[0].gv = m.l.o;
    force la[0].o = 4'he;
    force $root.tb.m.mv = gen[1].gw;
    force itf.iw = s + 4'h1;
    force wa[1:0] = s[1:0];
    m.poke(4'h4);
    f.apply();
    #1 $display("1 %h %h %h %h %h %h %h %h %h %h %h %h", m.l.v, m.l.o, gen[1].gw,
                gen[0].gv, la[0].o, la[1].o, m.mv, itf.iw, wb, top_v, cls_v, gen[0].gw);
    s = 4'h5;
    #1 $display("2 %h %h %h %h %h %h %h %h %h %h %h %h", m.l.v, m.l.o, gen[1].gw,
                gen[0].gv, la[0].o, la[1].o, m.mv, itf.iw, wb, top_v, cls_v, gen[0].gw);
    d = 4'h7;
    top_v = 4'h0;
    itf.iv = 4'h8;
    #1 $display("3 %h %h %h %h %h %h %h %h %h %h %h %h", m.l.v, m.l.o, gen[1].gw,
                gen[0].gv, la[0].o, la[1].o, m.mv, itf.iw, wb, top_v, cls_v, gen[0].gw);
    release gen[0].gv;
    release $root.tb.m.mv;
    release m.l.v;
    release m.l.o;
    release gen[1].gw;
    release la[0].o;
    release itf.iw;
    release wb[1:0];
    release top_v;
    f.undo();
    #1 $display("4 %h %h %h %h %h %h %h %h %h %h %h %h", m.l.v, m.l.o, gen[1].gw,
                gen[0].gv, la[0].o, la[1].o, m.mv, itf.iw, wb, top_v, cls_v, gen[0].gw);
    s = 4'h9;
    #1 $display("5 %h %h %h %h %h %h %h %h %h %h %h %h", m.l.v, m.l.o, gen[1].gw,
                gen[0].gv, la[0].o, la[1].o, m.mv, itf.iw, wb, top_v, cls_v, gen[0].gw);
    $finish;
  end
endmodule
