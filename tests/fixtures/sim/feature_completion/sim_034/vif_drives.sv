// SIM-034 A01: synchronous drives through virtual-interface handles to two
// instances clocked by irregular clocks (IEEE 1800-2009 14.16, 25.9). Output,
// skewed, cycle-delayed, selected and inout (net) drives are bound to the
// instance the handle names when the drive executes; rebinding the handle
// before a drive matures does not redirect it. The oracle is in readme.md.
`timescale 1ns/1ns
interface sync_bus (input bit clk);
  logic [7:0] b = 8'h00;
  wire [7:0] c;
  wire [3:0] n;
  logic [7:0] c_drv = 8'hzz;
  assign c = c_drv;

  clocking sb @(posedge clk);
    output #1 b;
    inout c;
    output n;
  endclocking
endinterface

typedef virtual sync_bus vi_t;

class driver;
  vi_t bus;
  function new(vi_t bus);
    this.bus = bus;
  endfunction
  task put(logic [7:0] x);
    bus.sb.b <= ##1 x;
  endtask
endclass

module tb;
  bit c1 = 0, c2 = 0;
  sync_bus b1 (c1);
  sync_bus b2 (c2);
  vi_t v;
  vi_t vs[2];
  driver dr;

  // c1 posedges: 3, 9, 15, 21, 27, 33.
  always #3 c1 = ~c1;
  // c2 posedges: 5, 7, 16, 30.
  initial begin
    #5 c2 = 1;
    #1 c2 = 0;
    #1 c2 = 1;
    #1 c2 = 0;
    #8 c2 = 1;
    #1 c2 = 0;
    #13 c2 = 1;
    #1 c2 = 0;
  end

  always @(b1.b or b1.c or b1.n)
    if ($time > 0) $display("%0d b1 b=%h c=%h n=%b", $time, b1.b, b1.c, b1.n);
  always @(b2.b or b2.c or b2.n)
    if ($time > 0) $display("%0d b2 b=%h c=%h n=%b", $time, b2.b, b2.c, b2.n);

  initial begin
    vs[0] = b1;
    vs[1] = b2;
    v = b1;
    b2.c_drv = 8'h0F;
    #1;
    v.sb.b <= 8'h11;
    v.sb.n <= 4'h3;
    v.sb.c <= ##2 8'hC1;
    v = b2;
    v.sb.b <= ##2 8'h22;
    v.sb.c <= 8'hF0;
    v.sb.n[1:0] <= 2'b10;
    dr = new(b1);
    dr.put(8'h33);
    #9 b2.c_drv = 8'hzz;
    v = b1;
    @(b1.sb);
    $display("%0d sampled b1.c=%h", $time, v.sb.c);
    #2;
    vs[1].sb.b <= 8'h44;
    vs[0].sb.b <= 8'h55;
    #18 $finish;
  end
endmodule
