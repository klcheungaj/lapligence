// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/localparam_runtime_index.sv
// IEEE 1800-2009 §§6.20, 7.3, and 11.5: runtime packed-array indices select
// fixed-width elements from owned packed-union localparam values.
module tb;
    typedef union packed {
        logic [15:0] word;
        logic [1:0][7:0] octets;
    } union_t;
    typedef struct packed {
        logic [7:0] sentinel;
        logic [1:0][7:0] octets;
    } record_t;
    /* verilator lint_off ASCRANGE */
    typedef struct packed {
        logic [0:1][7:0] octets;
    } ascending_t;
    /* verilator lint_on ASCRANGE */
    typedef struct packed {
        logic [0:-1][7:0] octets;
    } negative_range_t;

    localparam union_t PAYLOAD = union_t'(16'hA5C3);
    localparam union_t PAYLOAD_Z = union_t'(16'hA5Z3);
    localparam record_t RECORD = {8'hD7, 16'hA5C3};
    localparam ascending_t ASCENDING = 16'hA5C3;
    localparam negative_range_t NEGATIVE_RANGE = 16'hA5C3;
    logic signed [31:0] index;
    logic [31:0] unsigned_index;
    logic [7:0] high_octet;
    logic [7:0] low_octet;
    logic [7:0] ascending_left_octet;
    logic [7:0] ascending_right_octet;
    logic [7:0] signed_negative_octet;
    logic [7:0] z_octet;
    logic [7:0] unknown_octet;
    logic [7:0] positive_oob_octet;
    logic [7:0] negative_oob_octet;
    logic [7:0] member_oob_octet;
    logic [7:0] unsigned_oob_octet;

    initial begin
        index = 32'sd1;
        high_octet = PAYLOAD.octets[index];
        index = 32'sd0;
        low_octet = PAYLOAD.octets[index];
        ascending_left_octet = ASCENDING.octets[index];
        index = 32'sd1;
        ascending_right_octet = ASCENDING.octets[index];
        index = 32'sd0;
        z_octet = PAYLOAD_Z.octets[index];
        index = -32'sd1;
        signed_negative_octet = NEGATIVE_RANGE.octets[index];
        unsigned_index = 32'hffff_ffff;
        unsigned_oob_octet = NEGATIVE_RANGE.octets[unsigned_index];
        index = 32'hxxxx_xxxx;
        unknown_octet = PAYLOAD.octets[index];
        index = 32'sd2;
        positive_oob_octet = PAYLOAD.octets[index];
        member_oob_octet = RECORD.octets[index];
        index = -32'sd1;
        negative_oob_octet = PAYLOAD.octets[index];

        if (high_octet !== 8'hA5 || low_octet !== 8'hC3 ||
            ascending_left_octet !== 8'hA5 || ascending_right_octet !== 8'hC3 ||
            signed_negative_octet !== 8'hC3 || unsigned_oob_octet !== 8'hxx ||
            z_octet !== 8'hZ3 || unknown_octet !== 8'hxx ||
            positive_oob_octet !== 8'hxx || member_oob_octet !== 8'hxx ||
            negative_oob_octet !== 8'hxx)
            $fatal(1, "localparam runtime packed index mismatch");
        $display("valid=%h/%h asc=%h/%h signed=%h unsigned=%h z=%h unknown=%h out=%h/%h/%h",
                 high_octet, low_octet, ascending_left_octet,
                 ascending_right_octet, signed_negative_octet,
                 unsigned_oob_octet, z_octet, unknown_octet,
                 positive_oob_octet, member_oob_octet, negative_oob_octet);
        $finish(0);
    end
endmodule
