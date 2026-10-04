// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_018/bind_interface_into_module.sv
// RTL-018 A03 nearest legal form (SV 2009 §23.11): an interface may be bound
// into a module target; its body reads the target-local signal.
interface rtl018_watch(input logic [3:0] seen);
  logic [3:0] doubled;
  assign doubled = seen << 1;
endinterface
module rtl018_target(input logic [3:0] d);
  logic [3:0] local_d;
  assign local_d = d + 4'd1;
endmodule
module tb;
  logic [3:0] d;
  rtl018_target t(.d(d));
  initial begin
    d = 4'd3;
    #1 $display("watch=%0d", t.w.doubled);
    d = 4'd6;
    #1 $display("watch=%0d", t.w.doubled);
    $finish;
  end
endmodule
bind rtl018_target rtl018_watch w(.seen(local_d));
