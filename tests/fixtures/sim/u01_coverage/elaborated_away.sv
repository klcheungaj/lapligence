// llg-test-fixture: tests/fixtures/sim/u01_coverage/elaborated_away.sv
primitive mux2 (out, sel, a, b);
    output out;
    input sel, a, b;
    table
        0 ? 1 : 0 ;
        0 0 ? : 0 ;
        1 ? 0 : 1 ;
        1 1 ? : 1 ;
        x 0 0 : 0 ;
        x 1 1 : 1 ;
    endtable
endprimitive

module tb;
    typedef logic unused_net_t;
    logic result;
    localparam int WIDTH = 1;
    generate
        if (1'b0) begin : dead
            mux2 u(result, result, result, result);
        end
        if (1'b1) begin : live
            logic elaboration_marker;
        end
    endgenerate
    initial begin
        result = WIDTH - 1;
        #1 $display("PASS elaborated_away");
        $finish(0);
    end
endmodule
