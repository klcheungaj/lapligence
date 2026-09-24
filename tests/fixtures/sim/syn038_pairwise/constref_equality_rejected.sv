// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/constref_equality_rejected.sv
module tb;
    logic variable_actual;
    logic observed;

    task automatic capture(const ref logic value);
        observed = value;
    endtask

    initial begin
        variable_actual = 1'b0;
        capture(variable_actual);
        capture(variable_actual == 1'b1);
        $display("value=%b", observed);
        $finish(0);
    end
endmodule
