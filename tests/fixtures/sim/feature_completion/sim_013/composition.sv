// SIM-013 composition: a task's by-value handle formal names the object for
// the whole activation (SV 13.5.1), so rebinding the caller's variable does
// not move the task's waits; a virtual interface inside a class names the
// instance its property holds (SV 25.9), and an interface instance in a
// generate loop is observed through it.
`timescale 1ns / 1ns
interface bus;
  logic [7:0] data;
endinterface

module tb;
  class Node;
    int v;
  endclass

  class Port;
    virtual bus vb;
    function new(virtual bus b);
      vb = b;
    endfunction
  endclass

  for (genvar G = 0; G < 2; G++) begin : lane
    bus b ();
  end

  Node h = new, other = new, keep;
  Port port;

  task automatic watch(input Node c, input int target);
    @(c.v);
    $display("%0t watch v=%0d", $time, c.v);
    wait (c.v == target);
    $display("%0t watch reached %0d", $time, target);
  endtask

  initial begin
    keep = h;
    lane[0].b.data = 8'd0;
    lane[1].b.data = 8'd0;
    port = new(lane[0].b);
  end

  initial begin
    #0 @(port.vb.data);
    $display("%0t port data=%0d", $time, port.vb.data);
    @(port.vb.data);
    $display("%0t port data=%0d", $time, port.vb.data);
  end

  initial begin
    #0 watch(h, 3);
  end

  initial begin
    #1 h = other;
    #1 other.v = 5;
    #1 keep.v = 1;
    #1 other.v = 3;
    #1 lane[1].b.data = 8'd7;
    #1 lane[0].b.data = 8'd4;
    #1 port.vb = lane[1].b;
    #1 lane[0].b.data = 8'd9;
    #1 keep.v = 3;
    #1;
    $finish(0);
  end
endmodule
