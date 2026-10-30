// Decision A20-D19: delays whose source is not a constant, and the read-only
// delay objects. A primitive whose delay is an expression of variables reads
// the value its expressions have; a write replaces that value until an
// expression of the delay changes, after which the primitive uses the
// expressions again. A continuous assignment, a net declared with a delay
// and a delay control read the delays they use (a delay control: one);
// writes to a continuous assignment or a net are a vpiError.
//
// IEEE 1800-2009 37.3.4 (SystemVerilog-1800-2009.txt L57417-L57425): "Nets,
// primitives, module paths, timing checks, and continuous assignments can
// have delays specified within the SystemVerilog source code. [...] To
// access the actual delays being used by the tool, use the routine
// vpi_get_delays() on any of these objects."; 37.16 L58447-L58448 (nets),
// 37.43 L60294-L60295 (continuous assignments) and 37.63 L61199-L61200
// (delay controls) draw "-> delay vpi_get_delays()" without
// vpi_put_delays(); 37.31 L59762-L59764 draws both for primitives. Some
// simulators refuse vpi_get_delays on continuous assignments, nets and
// delay controls; llg follows the clause text, which lists them. The time a
// written runtime-valued delay gives way to its expressions matches
// industry-standard behaviour.
//
// Build A20-D19_runtime_delays.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d19(input integer stage)` declared as a system
// task).
`timescale 1ns/1ns
module tb;
  reg a;
  reg [3:0] d = 4'd2;
  wire wr, wc;
  and #(d) gr (wr, a, a);
  assign #3 wc = a;
  wire #4 wn;
  assign wn = a;
  always @(wr) $display("%0d wr=%b", $time, wr);
  initial begin
    #1 $d19(0);
    #10 a = 1'b1;
    #10 a = 1'b0;
    #10 $d19(1);
    d = 4'd5;
    a = 1'b1;
    #10 $d19(2);
    #10 $finish;
  end
endmodule
