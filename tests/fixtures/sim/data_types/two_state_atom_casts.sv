// IEEE 1800-2009 6.11 and 6.24.1: casts to byte/shortint/int/longint use
// their fixed two-state widths and source-signedness extension rules.
module tb #(parameter WIDTH = 128);
    logic [63:0] atom_source;
    logic signed [7:0] signed_source;
    // Widen into four-state observers so the cast owns both X/Z conversion
    // and signed extension. Removing a cast cannot be hidden by its LHS type.
    logic [63:0] byte_cast_value;
    logic [63:0] shortint_cast_value;
    logic [63:0] int_cast_value;
    logic [63:0] longint_cast_value;
    logic [63:0] signed_longint_cast_value;
    integer failed;

    initial begin
        atom_source = 64'h8000_0001_8001_0081;
        atom_source[1] = 1'bx;
        atom_source[2] = 1'bz;
        atom_source[30] = 1'bx;
        atom_source[29] = 1'bz;
        atom_source[62] = 1'bx;
        atom_source[61] = 1'bz;
        signed_source = 8'h81;
        #1;

        byte_cast_value = byte'(atom_source);
        shortint_cast_value = shortint'(atom_source);
        int_cast_value = int'(atom_source);
        longint_cast_value = longint'(atom_source);
        signed_longint_cast_value = longint'(signed_source);
        #1;

        failed = 0;
        if (byte_cast_value !== 64'hffff_ffff_ffff_ff81) begin
            $display("FAIL byte-cast WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && shortint_cast_value !== 64'h0000_0000_0000_0081) begin
            $display("FAIL shortint-cast WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && int_cast_value !== 64'hffff_ffff_8001_0081) begin
            $display("FAIL int-cast WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && longint_cast_value !== 64'h8000_0001_8001_0081) begin
            $display("FAIL longint-cast WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (!failed && signed_longint_cast_value !== 64'hffff_ffff_ffff_ff81) begin
            $display("FAIL signed-longint-cast WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS two_state_atom_casts WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
