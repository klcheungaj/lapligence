// Procedural assign/deassign is unsupported by design in every form
// (ADV-001); hierarchical variable targets are no exception.
module leaf;
  reg [3:0] x, a, b;
  real r;
endmodule

module mid;
  leaf v ();
endmodule

module owner (input [3:0] in);
  leaf c ();
  initial begin
    assign c.x = in;
    #1 deassign c.x;
  end
endmodule

module upward;
  initial begin
    assign tb.target = tb.src;
    #1 deassign tb.target;
  end
endmodule

module tb;
  reg [3:0] src, target;
  mid u ();
  generate
    if (1) begin : g
      leaf v ();
    end
  endgenerate
  genvar i;
  generate
    for (i = 0; i < 2; i = i + 1) begin : ga
      leaf w ();
    end
  endgenerate
  leaf arr [1:0] ();
  owner o0 (src);
  owner o1 (src);
  upward k ();
  initial begin
    src = 4'h3;
    assign u.v.x = src;
    assign g.v.x = src;
    assign ga[1].w.x = src;
    assign arr[0].x = src;
    assign {u.v.a, u.v.b} = {src, src};
    assign u.v.r = 2.5;
    #1 deassign u.v.x;
    deassign g.v.x;
    deassign ga[1].w.x;
    deassign arr[0].x;
    deassign {u.v.a, u.v.b};
    deassign u.v.r;
  end
endmodule
