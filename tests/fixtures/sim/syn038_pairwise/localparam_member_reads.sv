// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/localparam_member_reads.sv
// IEEE 1800-2009 §§6.20, 7.2, 7.3, and 11.5: packed localparam members
// project into runtime scalar destinations without signal-backed storage.
module tb;
    typedef struct packed {
        logic [7:0] hi;
        logic [7:0] lo;
    } pair_t;
    typedef union packed {
        logic [15:0] word;
        logic [1:0][7:0] octets;
    } union_t;

    localparam pair_t PAIR = '{hi:8'h12, lo:8'h34};
    localparam union_t PAYLOAD = union_t'(16'hA5C3);
    logic [7:0] observed_struct;
    logic [15:0] observed_union;
    logic [7:0] observed_octet;

    initial begin
        observed_struct = PAIR.hi;
        observed_union = PAYLOAD.word;
        observed_octet = PAYLOAD.octets[1];
        if (observed_struct !== 8'h12)
            $fatal(1, "localparam struct member mismatch");
        if (observed_union !== 16'hA5C3)
            $fatal(1, "localparam union member mismatch");
        if (observed_octet !== 8'hA5)
            $fatal(1, "localparam union packed element mismatch");
        $display("struct=%h", observed_struct);
        $display("union=%h", observed_union);
        $display("octet=%h", observed_octet);
        $finish(0);
    end
endmodule
