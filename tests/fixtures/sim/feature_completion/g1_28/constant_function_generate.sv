// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_28/constant_function_generate.sv
// A constant function evaluates once during elaboration: ADDR_BITS is 4, the
// active generate branch is selected by that constant, and the declared array
// width follows. The simulation oracle is `ADDR_BITS=4 addr=a`.
module tb;
    function automatic integer clog2i(input integer value);
        integer v;
        begin
            v = value - 1;
            clog2i = 0;
            while (v > 0) begin
                v = v >> 1;
                clog2i = clog2i + 1;
            end
        end
    endfunction

    localparam int ADDR_BITS = clog2i(16);
    logic [ADDR_BITS-1:0] addr;

    generate
        if (ADDR_BITS == 4) begin : g_live
            initial begin
                addr = 4'hA;
                $display("ADDR_BITS=%0d addr=%h", ADDR_BITS, addr);
                $finish(0);
            end
        end
        else begin : g_dead
            initial $display("BAD ADDR_BITS=%0d", ADDR_BITS);
        end
    endgenerate
endmodule
