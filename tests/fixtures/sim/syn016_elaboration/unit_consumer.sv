// llg-test-fixture: tests/fixtures/sim/syn016_elaboration/unit_consumer.sv
// $unit cannot see a declaration in a different separate compilation unit.
module tb;
    logic [$unit::UNIT_WIDTH-1:0] value;
    initial begin
        value = unit_add(3);
        $display("unit=%0d bits=%0d package=%0d", value, $bits(value), unit_shared::SHARED_WIDTH);
        $finish(0);
    end
endmodule
