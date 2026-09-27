// llg-test-fixture: tests/fixtures/sim/audit_a1_packed_constant_patterns/real_constant.sv
// IEEE 1800-2009 12.6: a constant expression pattern must be integral.
module tb;
    real value;
    initial begin
        value = 1.0;
        if (value matches 1.0) $display("invalid");
        $finish;
    end
endmodule
