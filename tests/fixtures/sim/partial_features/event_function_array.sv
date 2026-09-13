// llg-test-fixture: tests/fixtures/sim/partial_features/event_function_array.sv
// IEEE 1800-2009 §§7.4, 9.4.2, 13.4 and 13.5.1: a legal input function
// preserves fixed-array dynamic-index dependencies in an evaluated event.
module tb;
    logic [1:0] mem [0:1];
    logic index;
    int changes = 0;

    function automatic logic [1:0] identity(input logic [1:0] source);
        identity = source;
    endfunction

    initial begin
        index = 0;
        mem[0] = 0;
        mem[1] = 0;
        #1 mem[1] = 1;
        #1 mem[0] = 1;
        #1 index = 1;
        #1 mem[0] = 2;
        #1 mem[1] = 2;
        #1 $display("array_changes=%0d", changes);
        $finish(0);
    end

    always @(identity(mem[index]))
        changes = changes + 1;
endmodule
