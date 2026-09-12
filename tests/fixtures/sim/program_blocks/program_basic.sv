// llg-test-fixture: tests/fixtures/sim/program_blocks/program_basic.sv
// IEEE 1800-2009 §§24.3, 24.3.1 and 24.7: program reactive scheduling,
// reactive zero-delay/NBA ordering, and program completion.
program p(input logic in_value, output logic out_value);
    initial begin
        $display("program start in=%0d out=%0d", in_value, out_value);
        out_value <= in_value;
        #0 $display("program inactive in=%0d out=%0d", in_value, out_value);
        // Keep the program live until the module's explicit finish at t=1.
        #1;
    end
endprogram

module tb;
    logic in_value = 1'b0;
    logic out_value;
    p p0(.in_value(in_value), .out_value(out_value));

    initial begin
        in_value <= 1'b1;
        #0 $display("module inactive in=%0d out=%0d", in_value, out_value);
        #1 $display("module settled in=%0d out=%0d", in_value, out_value);
        $finish;
    end

    always @(out_value) $display("module saw program nba out=%0d", out_value);
endmodule
