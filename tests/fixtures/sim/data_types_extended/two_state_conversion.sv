// IEEE 1800-2009 6.11 and 6.24.1: assignments and static casts to two-state
// types map X and Z to zero while preserving known bits and signed extension.
module tb #(parameter WIDTH = 2048);
    typedef bit [WIDTH-1:0] bits_t;

    logic [WIDTH-1:0] source;
    logic [WIDTH-1:0] expected;
    bit [WIDTH-1:0] assigned;
    logic [WIDTH-1:0] cast_observer;
    logic scalar_x;
    logic scalar_z;
    logic scalar_one;
    bit assigned_x;
    bit assigned_z;
    logic cast_x;
    logic cast_z;
    logic cast_one;
    integer failed;

    initial begin
        source = '0;
        source[0] = 1'b1;
        source[1] = 1'bx;
        source[2] = 1'bz;
        source[64] = 1'b1;
        source[65] = 1'bx;
        source[WIDTH-1] = 1'b1;
        source[WIDTH-2] = 1'bz;
        expected = '0;
        expected[0] = 1'b1;
        expected[64] = 1'b1;
        expected[WIDTH-1] = 1'b1;
        scalar_x = 1'bx;
        scalar_z = 1'bz;
        scalar_one = 1'b1;
        #1;

        assigned = source;
        cast_observer = bits_t'(source);
        assigned_x = scalar_x;
        assigned_z = scalar_z;
        cast_x = bit'(scalar_x);
        cast_z = bit'(scalar_z);
        cast_one = bit'(scalar_one);
        #1;

        failed = 0;
        if (assigned !== expected) begin
            $display("FAIL vector-assignment WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && cast_observer !== expected) begin
            $display("FAIL vector-cast WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && (assigned_x !== 1'b0 || assigned_z !== 1'b0)) begin
            $display("FAIL scalar-assignment WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed &&
            (cast_x !== 1'b0 || cast_z !== 1'b0 || cast_one !== 1'b1)) begin
            $display("FAIL scalar-cast WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS two_state_conversion WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
