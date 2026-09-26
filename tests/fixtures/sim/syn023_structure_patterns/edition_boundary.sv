// llg-test-fixture: tests/fixtures/sim/syn023_structure_patterns/edition_boundary.sv
// `matches` is the only SystemVerilog-only construct in this file.
module tb;
    reg [3:0] value;
    initial begin
        value = 4'ha;
        if (value matches 4'ha) $display("structure_edition=pass");
        else $fatal(1, "constant pattern");
        $finish(0);
    end
endmodule
