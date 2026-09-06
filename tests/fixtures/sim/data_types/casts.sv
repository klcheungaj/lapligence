// llg-test-fixture: tests/fixtures/sim/data_types/casts.sv
module tb;
    parameter WIDTH = 128;

    logic signed [7:0] signed_byte;
    logic [7:0] unsigned_byte;
    logic [WIDTH-1:0] wide_input;
    logic signed [WIDTH-1:0] signed_input;
    logic [WIDTH-1:0] result_u;
    logic signed [WIDTH-1:0] result_s;
    logic [WIDTH-1:0] expected;
    logic [7:0] narrow;
    integer rounded_integer;
    logic [63:0] real_bits;
    logic [63:0] input_bits;
    logic [31:0] short_bits;
    logic [31:0] input_short_bits;
    real packed_real;
    real signed_real;
    real positive_real;
    real negative_real;
    real bitcast_real;
    real bitcast_source;
    real short_source;

    initial begin
        signed_byte = 8'h80;
        unsigned_byte = 8'h80;
        wide_input = '0;
        wide_input[WIDTH-1] = 1'b1;
        wide_input[100] = 1'b1;
        wide_input[64] = 1'b1;
        wide_input[50] = 1'b1;
        wide_input[7:0] = 8'ha5;
        signed_input = '1;
        signed_input[7:0] = 8'h80;
        positive_real = 2.5;
        negative_real = -2.5;
        input_bits = 64'h3ff8_0000_0000_0000;
        input_short_bits = 32'h4020_0000;
        bitcast_source = 3.141592653589793;
        short_source = 1.00000011920928955078125;
        #1;

        // Signing conversions retag the complete vector without losing bits
        // above the first runtime limb.
        result_s = $signed(wide_input);
        if (result_s[WIDTH-1] !== 1'b1 || result_s[100] !== 1'b1 ||
            result_s[64] !== 1'b1 || result_s[50] !== 1'b1 ||
            result_s[7:0] !== 8'ha5) begin
            $display("FAIL reinterpret-signed-high-bits");
            $finish;
        end
        result_u = $unsigned(result_s);
        if (result_u !== wide_input) begin
            $display("FAIL reinterpret-unsigned-high-bits");
            $finish;
        end
        result_u = $unsigned(result_s) >>> (WIDTH-1);
        if (result_u[WIDTH-1] !== 1'b0 || result_u[0] !== 1'b1) begin
            $display("FAIL reinterpret-unsigned-tag");
            $finish;
        end
        result_s = $signed(wide_input) >>> (WIDTH-1);
        if (result_s[WIDTH-1] !== 1'b1 || result_s[0] !== 1'b1) begin
            $display("FAIL reinterpret-signed-tag");
            $finish;
        end

        // Assignment conversion extends from the source signedness, then
        // carries the destination signedness. Narrowing retains low bits.
        result_u = signed_byte;
        expected = '1;
        expected[7:0] = 8'h80;
        if (result_u !== expected) begin
            $display("FAIL implicit-sign-extension");
            $finish;
        end
        result_s = unsigned_byte;
        expected = '0;
        expected[7:0] = 8'h80;
        if (result_s !== expected) begin
            $display("FAIL implicit-zero-extension");
            $finish;
        end
        narrow = wide_input;
        if (narrow !== 8'ha5) begin
            $display("FAIL implicit-truncation");
            $finish;
        end

        // A part-select is unsigned even when selected from a signed vector;
        // explicitly signing that same slice changes its widening behavior.
        result_u = signed_input[7:0];
        expected = '0;
        expected[7:0] = 8'h80;
        if (result_u !== expected) begin
            $display("FAIL part-select-unsigned");
            $finish;
        end
        result_s = $signed(signed_input[7:0]);
        expected = '1;
        expected[7:0] = 8'h80;
        if (result_s !== expected) begin
            $display("FAIL part-select-signed-cast");
            $finish;
        end

        // Packed-to-real conversion includes bits above 64. Integral casts
        // from real round halfway values away from zero.
        wide_input = '0;
        wide_input[100] = 1'b1;
        wide_input[50] = 1'b1;
        signed_input = '1;
        #1;
        packed_real = real'(wide_input);
        signed_real = real'(signed_input);
        if (packed_real != 1267650600228230527396610048000.0) begin
            $display("FAIL packed-to-real-wide");
            $finish;
        end
        if (signed_real != -1.0) begin
            $display("FAIL packed-to-real-signed");
            $finish;
        end
        rounded_integer = integer'(positive_real);
        if (rounded_integer !== 3) begin
            $display("FAIL real-round-positive");
            $finish;
        end
        rounded_integer = integer'(negative_real);
        if (rounded_integer !== -3) begin
            $display("FAIL real-round-negative");
            $finish;
        end

        // Bit conversion system functions reinterpret rather than numerically
        // convert, and shortreal conversion rounds through IEEE binary32.
        real_bits = $realtobits(bitcast_source);
        if (real_bits !== 64'h4009_21fb_5444_2d18) begin
            $display("FAIL real-to-bits");
            $finish;
        end
        bitcast_real = $bitstoreal(input_bits);
        if (bitcast_real != 1.5) begin
            $display("FAIL bits-to-real");
            $finish;
        end
        short_bits = $shortrealtobits(short_source);
        if (short_bits !== 32'h3f80_0001) begin
            $display("FAIL shortreal-round-bits");
            $finish;
        end
        bitcast_real = $bitstoshortreal(input_short_bits);
        if (bitcast_real != 2.5) begin
            $display("FAIL bits-to-shortreal");
            $finish;
        end

        $display("PASS casts WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
