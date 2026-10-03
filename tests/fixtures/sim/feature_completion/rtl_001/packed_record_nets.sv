// IEEE 1800-2009 sections 6.7 and 7.2.1: four-state packed record nets.
module tb;
  typedef struct packed { logic [7:4] upper; logic [3:0] lower; } record_t;
  wire record_t value;
  logic [3:0] source;
  assign value.upper = source;
  assign value.lower = 4'h5;
  initial begin
    source = 4'ha;
    #1;
    $display("packed-net=%h:%h whole=%h", value.upper, value.lower, value);
    source = 4'hc;
    #1;
    $display("changed=%h", value);
    $finish(0);
  end
endmodule
