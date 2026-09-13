// llg-test-fixture: tests/fixtures/sim/partial_features/event_pure_functions.sv
// IEEE 1800-2009 §§9.4.2, 9.4.2.3, 13.4 and 13.5.2: legal input and
// const-ref functions in evaluated event controls use expression dependencies.
module tb;
    logic a = 0;
    logic b = 0;
    logic qualifier = 0;
    logic [1:0] value = 0;
    int changes = 0;
    int rises = 0;
    int const_changes = 0;

    function automatic logic both(input logic left, input logic right);
        both = left && right;
    endfunction

    function automatic logic [1:0] read_const(const ref logic [1:0] source);
        read_const = source;
    endfunction

    always @(both(a, b))
        changes = changes + 1;

    always @(posedge both(a, b) iff qualifier)
        rises = rises + 1;

    always @(read_const(value))
        const_changes = const_changes + 1;

    initial begin
        #1 a = 1;
        #1 b = 1;
        #1 qualifier = 1;
        #1 a = 0;
        #1 a = 1;
        #1 value = 2;
        #1 value = 2;
        #1 value = 3;
        #1 $display("changes=%0d rises=%0d const=%0d", changes, rises,
                    const_changes);
        $finish(0);
    end
endmodule
