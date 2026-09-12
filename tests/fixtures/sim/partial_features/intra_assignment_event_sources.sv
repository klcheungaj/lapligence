// llg-test-fixture: tests/fixtures/sim/partial_features/intra_assignment_event_sources.sv
module tb;
    logic clocked;
    logic reset;
    logic qualifier;
    logic source;
    logic result;

    function automatic logic identity(input logic value);
        identity = value;
    endfunction

    initial begin
        clocked = 1'b0;
        reset = 1'b1;
        qualifier = 1'b0;
        source = 1'b0;
        result = 1'b0;
        result = @(posedge identity(source) iff qualifier or negedge reset) 1'b1;
        $display("sources t=%0t result=%0d source=%0d", $time, result, source);
        $finish(0);
    end

    initial begin
        #1 qualifier = 1'b1;
        #1 source = 1'b1;
    end
endmodule
