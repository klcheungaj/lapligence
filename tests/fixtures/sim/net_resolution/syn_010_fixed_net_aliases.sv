// llg-test-fixture: tests/fixtures/sim/net_resolution/syn_010_fixed_net_aliases.sv
// IEEE 1800-2009 10.11; 6.5-6.7: fixed net elements and packed projections.
module child(inout wire [3:0] p);
  wire [3:0] inner;
  alias p = inner;
  assign inner = 4'h5;
endmodule

module tb;
  wire [3:0] source [0:1];
  wire [3:0] mirror [0:1];
  alias source[0] = mirror[1];
  assign source[0] = 4'ha;
  assign mirror[1] = 4'hz;

  wire [0:3] ascending_left;
  wire [0:3] ascending_right;
  alias ascending_left = ascending_right;
  assign ascending_left = 4'b1010;
  assign ascending_right = 4'bzzzz;

  wire [1:0][1:0] packed_left;
  wire [1:0][1:0] packed_right;
  alias packed_left[0] = packed_right[1];
  assign packed_left[0] = 2'b10;
  assign packed_right[1] = 2'bzz;

  wire [1:0][3:0] nested_left;
  wire [1:0][1:0] nested_right;
  alias nested_left[0][3:2] = nested_right[1];
  assign nested_left = 8'bzzzzzzzz;
  assign nested_right = 4'b1100;

  wire [3:0] linked [0:0];
  child u_child(.p(linked[0]));

  initial begin
    #1;
    $display("CHECK: fixed=%h/%h", source[0], mirror[1]);
    force source[0] = 4'h3;
    #1 $display("CHECK: forced=%h/%h", source[0], mirror[1]);
    release source[0];
    #1 $display("CHECK: released=%h/%h", source[0], mirror[1]);
    $display("CHECK: ascending=%b/%b", ascending_left, ascending_right);
    $display("CHECK: packed=%b/%b", packed_left[0], packed_right[1]);
    $display("CHECK: nested=%b/%b", nested_left[0][3:2], nested_right[1]);
    $display("CHECK: linked=%h", linked[0]);
    $finish(0);
  end
endmodule
