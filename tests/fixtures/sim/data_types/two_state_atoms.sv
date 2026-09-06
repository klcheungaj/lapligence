// IEEE 1800-2009 6.11 / Table 6-8: byte, shortint, int, and longint are
// signed two-state atoms. integer and time remain four-state types.
module tb #(parameter WIDTH = 128);
    logic [63:0] atom_source;
    logic signed [7:0] signed_source;
    logic [7:0] unsigned_source;

    byte byte_value;
    shortint shortint_value;
    int int_value;
    longint longint_value;
    longint signed_longint;
    longint unsigned_longint;

    integer integer_value;
    time time_value;
    logic [31:0] expected_integer;
    logic [63:0] expected_time;
    integer failed;

    initial begin
        // The known literal supplies independent expected low-byte, low-word,
        // low-int, and full-longint patterns. X/Z occupy otherwise-zero bits.
        atom_source = 64'h8000_0001_8001_0081;
        atom_source[1] = 1'bx;
        atom_source[2] = 1'bz;
        atom_source[30] = 1'bx;
        atom_source[29] = 1'bz;
        atom_source[62] = 1'bx;
        atom_source[61] = 1'bz;

        expected_integer = 32'h8001_0081;
        expected_integer[1] = 1'bx;
        expected_integer[2] = 1'bz;
        expected_integer[30] = 1'bx;
        expected_integer[29] = 1'bz;
        expected_time = 64'h8000_0001_8001_0081;
        expected_time[1] = 1'bx;
        expected_time[2] = 1'bz;
        expected_time[30] = 1'bx;
        expected_time[29] = 1'bz;
        expected_time[62] = 1'bx;
        expected_time[61] = 1'bz;
        signed_source = 8'h81;
        unsigned_source = 8'h81;

        #1;
        byte_value = atom_source;
        shortint_value = atom_source;
        int_value = atom_source;
        longint_value = atom_source;
        signed_longint = signed_source;
        unsigned_longint = unsigned_source;
        integer_value = atom_source;
        time_value = atom_source;
        #1;

        failed = 0;
        if (byte_value !== 8'h81) begin
            $display("FAIL byte-assignment WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && shortint_value !== 16'h0081) begin
            $display("FAIL shortint-assignment WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && int_value !== 32'h8001_0081) begin
            $display("FAIL int-assignment WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && longint_value !== 64'h8000_0001_8001_0081) begin
            $display("FAIL longint-assignment WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && signed_longint !== 64'hffff_ffff_ffff_ff81) begin
            $display("FAIL signed-longint-extension WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && unsigned_longint !== 64'h0000_0000_0000_0081) begin
            $display("FAIL unsigned-longint-extension WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && integer_value !== expected_integer) begin
            $display("FAIL integer-four-state WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && time_value !== expected_time) begin
            $display("FAIL time-four-state WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS two_state_atoms WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
