// llg-test-fixture: tests/fixtures/sim/syn022_primitive_patterns/edition_boundary.sv
// IEEE 1800-2009 12.6 adds matches; remaining syntax is Verilog-2001 compatible.
module tb;
    reg [7:0] value;
    initial begin
        value = 8'h5a;
        if (value matches 8'h5a)
            $display("edition_pattern=pass");
        else
            $display("edition_pattern=fail");
        $finish(0);
    end
endmodule
