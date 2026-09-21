// llg-test-fixture: tests/fixtures/sim/sequential_predicates/bad_matches_out_of_scope.sv
// LRM: IEEE 1800-2009 12.6, 12.6.2-12.6.3.
module tb;
    logic [7:0] value;
    initial begin
        value = 8'h5a;
        if (value matches .bound)
            $display("matched");
        else
            $display("out-of-scope=%h", bound);
        $finish(0);
    end
endmodule
