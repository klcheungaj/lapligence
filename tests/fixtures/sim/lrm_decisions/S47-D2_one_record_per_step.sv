// Decision S47-D2: while dumping is on, a variable that changed during a time
// step is recorded once, with its value at the end of the step. A variable
// that changed and returned to its earlier value is still recorded (once);
// values written by final procedures are recorded at the end time.
//
// IEEE 1800-2009 21.7.1.4 (SystemVerilog-1800-2009.txt L37887-37888): "When
// dumping is enabled, the value change dumper records the values of the
// variables that change during each time increment. Values of variables that
// do not change during a time increment are not dumped." The text does not
// say whether several changes in one increment give several records; llg
// records the end-of-step value once.
// Compare S47-D2.vcd: at `#1` one record each of g = 0 and b = 1 (both
// returned to their earlier values), `#2 1!` for g, then `#3 0!` from the
// final procedure.
module tb;
  logic g;
  logic b;
  initial begin
    g = 1'b0;
    b = 1'b1;
    $dumpfile("S47-D2.vcd");
    $dumpvars(0, tb);
    #1 g = 1'b1;
    g = 1'b0;
    b = 1'b0;
    b = 1'b1;
    $strobe("end of step 1: g=%b b=%b", g, b);
    #1 g = 1'b1;
    g = 1'b1;
    $strobe("end of step 2: g=%b b=%b", g, b);
    #1 $finish;
  end
  final begin
    g = 1'b0;
    $display("final: g=%b", g);
  end
endmodule
