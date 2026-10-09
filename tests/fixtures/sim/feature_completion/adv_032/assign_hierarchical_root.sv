// `$root` and interface-port paths to variables: still procedural
// assign/deassign, so still unsupported by design (ADV-001).
interface bus_if;
  logic [3:0] x;
endinterface

module user (bus_if p);
  initial begin
    assign p.x = 4'hb;
    #1 deassign p.x;
  end
endmodule

module leaf;
  logic [3:0] x;
endmodule

module tb;
  logic [3:0] src;
  leaf u ();
  bus_if i ();
  user us (i);
  initial begin
    src = 4'h2;
    assign $root.tb.u.x = src;
    assign i.x = src;
    #1 deassign $root.tb.u.x;
    deassign i.x;
  end
endmodule
