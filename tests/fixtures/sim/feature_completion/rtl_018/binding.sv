// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_018/binding.sv
// RTL-018 A02 (V 2001 §13; SV 2009 §§33.4-33.8): both libraries define
// rtl018_pick. Without a configuration the library search order chooses one
// (an implementation-defined tie the command line resolves); `%l` reports the
// selected library.cell binding.
module rtl018_local(output [7:0] v);
  assign v = 8'd40;
endmodule

module tb;
  wire [7:0] picked, local_v;
  rtl018_pick u(picked);
  rtl018_local l(local_v);
  initial begin
    #1 $display("picked=%0d local=%0d", picked, local_v);
    $finish;
  end
endmodule
