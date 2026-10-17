// llg-test-fixture: SYN-039 neighboring control for the selected UDP scope.
// A sequential UDP is outside the selected combinational UDP profile; ADV-003
// implements it, so this neighbor runs instead of rejecting. At time 0 the
// clock changes from x to 0, which no row lists, so the state is x (IEEE
// 1364-2001 8.4, decision S3-D4); the rising edge then sets it to 1.
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
        #1 $display("before=%b", result);
        clock = 1'b1;
        #1 $display("after=%b", result);
        $finish(0);
    end
endmodule
