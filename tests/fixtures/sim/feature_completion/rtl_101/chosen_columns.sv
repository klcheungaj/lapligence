// Records below the dense threshold whose member arrays the design selects:
// values, events, sensitivity and scheduling match the per-cell form.
`timescale 1ns / 1ns

typedef struct {
  logic [7:0] tag;
  int a[32];
  logic [7:0] b[32];
  logic [3:0] x;
} fixed_t;

typedef struct {
  string name;
  int a[32];
  logic [3:0] x;
} native_t;

module echo (input fixed_t i, output fixed_t o);
  always_comb begin
    o = i;
    o.a[1] = i.a[1] + 1;
  end
endmodule

module tb;
  fixed_t f, g, h, port_out;
  integer unknown;
  native_t n, m;
  int k = 3, sum, watched;
  wire [31:0] cell_wire = unsigned'(n.a[5]);
  wire [31:0] index_wire = unsigned'(n.a[k]);

  assign h = f;
  echo u_echo (.i(f), .o(port_out));

  always @(f.a[3]) $display("%0t f.a[3]=%0d", $time, f.a[3]);
  always @(n.a[k]) $display("%0t n.a[k]=%0d k=%0d", $time, n.a[k], k);
  always @(posedge f.b[2][0]) $display("%0t posedge f.b[2][0]", $time);

  always_comb begin
    sum = f.a[3] + f.a[4];
    $display("%0t comb sum=%0d", $time, sum);
  end
  always_comb watched = n.a[k] * 2;

  always @(posedge f.x[0]) begin
    f.a[9] <= f.a[9] + 1;
    n.a[k] <= 77;
  end

  initial begin
    n.name = "start";
    #1 f.a[3] = 5;
    #1 f.a[5] = 7;
    #1 f.a[4] = 6;
    #1 n.a[3] = 8;
    #1 k = 4;
    #1 n.a[3] = 9;
    #1 n.a[4] = 10;
    #1 f.b[2] = 8'h01;
    #1 f.a[3] += 2;
    #1 f.a[k][7:0] = 8'h55;
    #1 $display("sum=%0d watched=%0d wires=%0d/%0d", sum, watched, cell_wire, index_wire);
    $display("h==f %b port=%0d/%0d", h.a[4] == f.a[4], port_out.a[1], port_out.a[4]);
    f.x = 4'b0001;
    #1 $display("nba %0d %0d", f.a[9], n.a[k]);
    fork
      wait (n.a[k] == 99) $display("%0t wait hit", $time);
      #1 n.a[k] = 99;
    join
    #1 k = 40;
    n.a[k] = 1;
    $display("out of range %0d", n.a[k]);
    unknown = 'x;
    f.a[unknown] = 1;
    $display("unknown %0d", f.a[unknown]);
    k = 4;
    g = f;
    m = n;
    g.a[k]++;
    m.a[k]--;
    $display("copies %0d %0d %0d %0d", g.a[k], f.a[k], m.a[k], n.a[k]);
    $display("eq %b %b %b %b", g == f, g.a[k] === f.a[k] + 1, m == n, m.name == n.name);
    foreach (g.b[i]) g.b[i] = i[7:0];
    $display("bits %0d %h %h %b", $bits(g), g.b[31], g.b[k][3:0], g.x);
    #1 $finish(0);
  end
endmodule
