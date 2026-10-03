// SV2009 sections 6.7, 7.2.2, 23.3: unpacked record nets through ports.
package layout_types;
  typedef struct { logic [7:0] lane; logic signed [3:0] tag; } record_t;
endpackage
module driver(inout layout_types::record_t bus, input logic [7:0] data);
  assign bus.lane = data;
  assign bus.tag = 4'hf;
endmodule
module tb;
  wire layout_types::record_t bus;
  logic [7:0] source;
  driver child(bus, source);
  initial begin
    source = 8'h5a;
    #1; $display("port=%h:%0d", bus.lane, bus.tag);
    source = 8'ha5;
    #1; $display("changed=%h:%0d", child.bus.lane, child.bus.tag);
    $finish(0);
  end
endmodule
