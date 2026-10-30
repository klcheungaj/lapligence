// ADV-019 declaration and instance objects (IEEE 1800-2009 37.10, 33.7):
// declarations.c prints the definition and configuration properties of the
// instances a configuration binds. The test elaborates configuration `cfg`
// (`--top cfg`), whose design is work.tb.
module leaf (input logic a, output logic y);
  assign y = ~a;
endmodule
module tb;
  logic a = 1'b0;
  logic y;
  leaf u (.a(a), .y(y));
  initial begin
    #1 $declarations;
    $display("y=%b", y);
    $finish;
  end
endmodule
config cfg;
  design work.tb;
  default liblist work;
endconfig
