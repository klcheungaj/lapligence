// IEEE 1800-2009 6.11 / Table 6-8: bit is two-state. Assignment from a
// four-state expression maps X and Z to zero while retaining known bits.
// Assignment padding follows the source signedness (6.24.1, 10.7, 11.8.3).
module tb #(parameter WIDTH = 128);
    logic [WIDTH-1:0] logic_source;
    reg   [WIDTH-1:0] reg_source;
    logic [WIDTH-1:0] expected_logic;
    logic [WIDTH-1:0] expected_reg;
    bit [WIDTH-1:0] from_logic;
    bit [WIDTH-1:0] from_reg;

    logic scalar_x;
    logic scalar_z;
    logic scalar_one;
    bit scalar_from_x;
    bit scalar_from_z;
    bit scalar_from_one;

    logic signed [7:0] signed_source;
    logic [7:0] unsigned_source;
    bit signed [WIDTH-1:0] signed_extended;
    bit [WIDTH-1:0] unsigned_extended;
    logic [WIDTH-1:0] expected_signed_extended;
    logic [WIDTH-1:0] expected_unsigned_extended;
    integer failed;

    initial begin
        // Populate unknowns in the low limb, across the first limb boundary,
        // and at the configured high end. WIDTH is tested at 128/512/1024.
        logic_source = '0;
        logic_source[0] = 1'b1;
        logic_source[1] = 1'bx;
        logic_source[2] = 1'bz;
        logic_source[63] = 1'b1;
        logic_source[64] = 1'bx;
        logic_source[65] = 1'bz;
        logic_source[WIDTH-1] = 1'b1;
        logic_source[WIDTH-2] = 1'bx;
        logic_source[WIDTH-3] = 1'bz;
        expected_logic = '0;
        expected_logic[0] = 1'b1;
        expected_logic[63] = 1'b1;
        expected_logic[WIDTH-1] = 1'b1;

        reg_source = '1;
        reg_source[4] = 1'bx;
        reg_source[5] = 1'bz;
        reg_source[66] = 1'bx;
        reg_source[67] = 1'bz;
        reg_source[WIDTH-4] = 1'bx;
        reg_source[WIDTH-5] = 1'bz;
        expected_reg = '1;
        expected_reg[4] = 1'b0;
        expected_reg[5] = 1'b0;
        expected_reg[66] = 1'b0;
        expected_reg[67] = 1'b0;
        expected_reg[WIDTH-4] = 1'b0;
        expected_reg[WIDTH-5] = 1'b0;

        scalar_x = 1'bx;
        scalar_z = 1'bz;
        scalar_one = 1'b1;
        signed_source = 8'h81;
        unsigned_source = 8'h81;
        expected_signed_extended = '1;
        expected_signed_extended[7:0] = 8'h81;
        expected_unsigned_extended = '0;
        expected_unsigned_extended[7:0] = 8'h81;

        // Keep conversion observable at runtime rather than as a folded
        // declaration initializer or constant expression.
        #1;
        from_logic = logic_source;
        from_reg = reg_source;
        scalar_from_x = scalar_x;
        scalar_from_z = scalar_z;
        scalar_from_one = scalar_one;
        signed_extended = signed_source;
        unsigned_extended = unsigned_source;
        #1;

        failed = 0;
        if (from_logic !== expected_logic) begin
            $display("FAIL vector-logic WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && from_reg !== expected_reg) begin
            $display("FAIL vector-reg WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && scalar_from_x !== 1'b0) begin
            $display("FAIL scalar-x WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && scalar_from_z !== 1'b0) begin
            $display("FAIL scalar-z WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && scalar_from_one !== 1'b1) begin
            $display("FAIL scalar-known WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && signed_extended !== expected_signed_extended) begin
            $display("FAIL signed-extension WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && unsigned_extended !== expected_unsigned_extended) begin
            $display("FAIL unsigned-extension WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS two_state WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
