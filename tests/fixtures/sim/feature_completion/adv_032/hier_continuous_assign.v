// Module-scope continuous assignments into nets of other instances
// (IEEE 1364-2001 6.1 and 12.4): downward, multi-level, generate-if,
// generate-for, instance-array element, top-name-absolute and upward paths,
// and constant part-selects of one hierarchical net.
module leaf;
  wire [3:0] n;
endmodule

module mid;
  leaf v ();
endmodule

module bus;
  wire [7:0] n;
endmodule

module upward;
  assign tb.up = 4'h5;
endmodule

module tb;
  reg [3:0] src;
  wire [3:0] up;
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
  bus b ();
  upward k ();
  assign u.v.n = src;
  assign g.v.n = src + 4'h1;
  assign ga[1].w.n = src + 4'h2;
  assign arr[0].n = src + 4'h3;
  assign tb.arr[1].n = ~src;
  assign b.n[3:0] = src;
  assign b.n[7:4] = 4'ha;
  initial begin
    src = 4'h1;
    #1 $display("%h %h %h %h %h %h %h %h", u.v.n, g.v.n, ga[1].w.n, ga[0].w.n, arr[0].n,
                arr[1].n, b.n, up);
    src = 4'h6;
    #1 $display("%h %h %h %h %h %h %h %h", u.v.n, g.v.n, ga[1].w.n, ga[0].w.n, arr[0].n,
                arr[1].n, b.n, up);
    $finish;
  end
endmodule
