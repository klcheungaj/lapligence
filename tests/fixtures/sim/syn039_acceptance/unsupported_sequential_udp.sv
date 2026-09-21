// llg-test-fixture: SYN-039 neighboring negative for the selected UDP scope.
// A sequential UDP is deliberately outside the selected combinational UDP
// profile; this fixture contains no unrelated source error.
primitive syn039_sequential(result, clock);
    output reg result;
    input clock;
    table
        (01) : ? : 1;
    endtable
endprimitive

module tb;
    logic clock;
    wire result;
    syn039_sequential dut(result, clock);

    initial begin
        clock = 1'b0;
        #1 clock = 1'b1;
        $finish(0);
    end
endmodule
