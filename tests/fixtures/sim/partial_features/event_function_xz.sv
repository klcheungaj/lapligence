// llg-test-fixture: tests/fixtures/sim/partial_features/event_function_xz.sv
// IEEE 1800-2009 §§9.4.2, 13.4 and 13.5.1: function-valued event edges retain
// four-state any-change and edge transitions.
module tb;
    logic value = 0;
    logic enable = 1;
    int changes = 0;
    int rises = 0;
    int falls = 0;

    function automatic logic identity(input logic source);
        identity = source;
    endfunction

    always @(identity(value))
        changes = changes + 1;
    always @(posedge identity(value) iff enable)
        rises = rises + 1;
    always @(negedge identity(value))
        falls = falls + 1;

    initial begin
        #1 value = 1'bx;
        #1 value = 1'bz;
        #1 value = 1;
        #1 value = 1'bz;
        #1 value = 0;
        #1 $display("function_xz=%0d %0d %0d", changes, rises, falls);
        $finish(0);
    end
endmodule
