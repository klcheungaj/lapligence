// llg-test-fixture: tests/fixtures/sim/audit_a1_packed_constant_patterns/unpacked_subject.sv
// IEEE 1800-2009 12.6 requires a constant pattern to have the matched type;
// an unpacked structure is not an integral vector (7.2.1).
module tb;
    typedef struct { logic [3:0] hi; logic [3:0] lo; } unpacked_t;
    unpacked_t value;
    initial begin
        value = '{hi: 4'ha, lo: 4'h5};
        if (value matches 8'ha5) $display("invalid");
        $finish;
    end
endmodule
