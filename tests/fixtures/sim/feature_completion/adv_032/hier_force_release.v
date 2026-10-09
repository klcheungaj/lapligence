// force/release on hierarchical targets (IEEE 1364-2001 9.3.2): variables
// keep the forced value after release, nets resume their drivers; forms
// cover multi-level, generate, top-name-absolute, upward, real, a net
// bit-select and a concatenation.
module leaf;
  reg [3:0] x, a, b;
  real r;
  wire [3:0] n;
  assign n = x;
endmodule

module mid;
  leaf v ();
endmodule

module upward;
  initial begin
    #2 force tb.t = 4'h9;
    force tb.w = 4'h3;
    #2 release tb.t;
    release tb.w;
  end
endmodule

module tb;
  reg [3:0] src, t, d;
  wire [3:0] w;
  assign w = d;
  mid u ();
  generate
    if (1) begin : g
      leaf v ();
    end
  endgenerate
  upward k ();
  initial begin
    u.v.x = 4'h1;
    g.v.x = 4'h2;
    u.v.r = 1.5;
    u.v.a = 4'h3;
    u.v.b = 4'h4;
    src = 4'h8;
    t = 4'h1;
    d = 4'h2;
    #1 force u.v.x = src;
    force g.v.n = src;
    force g.v.n[0] = 1'b0;
    force u.v.r = 7.25;
    force {u.v.a, u.v.b} = {src, ~src};
    force tb.g.v.x = 4'he;
    #1 $display("%h %h %h %h %0.2f %h %h", u.v.x, u.v.n, g.v.x, g.v.n, u.v.r, u.v.a, u.v.b);
    src = 4'h3;
    #1 $display("%h %h %h %h %h %h %h", u.v.x, u.v.n, g.v.n, u.v.a, u.v.b, t, w);
    release u.v.x;
    release g.v.n[0];
    release g.v.n;
    release u.v.r;
    release {u.v.a, u.v.b};
    release tb.g.v.x;
    #1 $display("%h %h %h %h %0.2f %h %h", u.v.x, u.v.n, g.v.x, g.v.n, u.v.r, u.v.a, u.v.b);
    u.v.x = 4'h5;
    u.v.r = 0.5;
    #1 $display("%h %h %0.2f %h %h", u.v.x, u.v.n, u.v.r, t, w);
    t = 4'h4;
    #1 $display("%h", t);
    $finish;
  end
endmodule
