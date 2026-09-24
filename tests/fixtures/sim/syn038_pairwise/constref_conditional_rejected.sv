// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/constref_conditional_rejected.sv
module tb;
    typedef logic [7:0] byte_t;
    byte_t variable_actual;
    byte_t observed;

    task automatic capture(const ref byte_t value);
        observed = value;
    endtask

    initial begin
        variable_actual = 8'h25;
        capture(variable_actual);
        capture(1'b1 ? variable_actual : 8'h34);
        $display("value=%h", observed);
        $finish(0);
    end
endmodule
