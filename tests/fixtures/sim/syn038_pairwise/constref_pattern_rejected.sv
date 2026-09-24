// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/constref_pattern_rejected.sv
module tb;
    typedef struct packed { logic [7:0] hi; logic [7:0] lo; } pair_t;
    logic [7:0] left_value;
    logic [7:0] right_value;
    pair_t variable_actual;
    pair_t observed;

    task automatic capture(const ref pair_t value);
        observed = value;
    endtask

    initial begin
        left_value = 8'h25;
        right_value = 8'h34;
        variable_actual = pair_t'{hi: left_value, lo: right_value};
        capture(variable_actual);
        capture(pair_t'{hi: left_value, lo: right_value});
        $display("value=%h", observed);
        $finish(0);
    end
endmodule
