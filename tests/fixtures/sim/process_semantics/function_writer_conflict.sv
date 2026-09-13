// llg-test-fixture: tests/fixtures/sim/process_semantics/function_writer_conflict.sv
// IEEE 1800-2009 Sections 9.2.2.2 and 13.4: a variable written by a called
// function is still an always_comb writer and cannot have a second writer.
module tb;
    logic a;
    logic b;
    logic helper;
    logic result;

    function automatic logic side_effect(input logic value);
        helper = value;
        side_effect = value;
    endfunction

    always_comb result = side_effect(a);
    always_comb helper = b;

    initial begin
        a = 1'b0;
        b = 1'b1;
        #1 $finish(0);
    end
endmodule
