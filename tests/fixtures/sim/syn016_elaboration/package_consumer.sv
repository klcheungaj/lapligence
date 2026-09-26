// llg-test-fixture: tests/fixtures/sim/syn016_elaboration/package_consumer.sv
// Package definitions remain visible across separate compilation units.
module tb import unit_shared::*; ();
    logic [SHARED_WIDTH-1:0] value;
    initial begin
        value = 5;
        $display("package=%0d bits=%0d", value, $bits(value));
        $finish(0);
    end
endmodule
