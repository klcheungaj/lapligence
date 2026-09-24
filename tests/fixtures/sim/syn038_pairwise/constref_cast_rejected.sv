// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/constref_cast_rejected.sv
module tb;
    typedef bit [7:0] byte_t;
    logic [7:0] source_value;
    byte_t variable_actual;
    byte_t observed;

    task automatic capture(const ref byte_t value);
        observed = value;
    endtask

    initial begin
        source_value = 8'h25;
        variable_actual = 8'h34;
        capture(variable_actual);
        capture(byte_t'(source_value));
        $display("value=%h", observed);
        $finish(0);
    end
endmodule
