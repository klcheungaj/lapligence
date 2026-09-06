// IEEE 1800-2009 11.5.1: out-of-range read bits are X, out-of-range write
// bits have no effect, and indexed part-selects retain their declared width.
module tb #(parameter WIDTH = 4096);
    logic [WIDTH-1:0] value;
    logic [WIDTH-1:0] expected;
    logic [7:0] slice;
    logic [7:0] array_value;
    logic selected;
    integer signed_base;
    logic [7:0] unsigned_index8;
    logic signed [7:0] signed_index8;
    logic [15:0] unsigned_index16;
    logic signed [15:0] signed_index16;
    logic [7:0] fixed_values [0:32768];
    integer failed;

    initial begin
        failed = 0;
        value = '0;
        value[5:0] = 6'b101011;
        signed_base = -2;
        slice = value[signed_base +: 8];
        if (slice !== 8'b101011xx) begin
            $display("FAIL negative-base-read WIDTH=%0d", WIDTH);
            failed = 1;
        end

        signed_base = 'x;
        slice = value[signed_base +: 8];
        selected = value[signed_base];
        if (!failed && (slice !== 8'hxx || selected !== 1'bx)) begin
            $display("FAIL unknown-base-read WIDTH=%0d", WIDTH);
            failed = 1;
        end

        value = '0;
        value[WIDTH-1 -: 8] = 8'ha5;
        signed_base = WIDTH-1;
        slice = value[signed_base -: 8];
        selected = value[signed_base];
        if (!failed && (slice !== 8'ha5 || selected !== 1'b1 ||
                        value[64] !== 1'b0)) begin
            $display("FAIL high-read WIDTH=%0d", WIDTH);
            failed = 1;
        end

        value = '0;
        signed_base = -2;
        value[signed_base +: 8] = 8'b11010110;
        expected = '0;
        expected[5:0] = 6'b110101;
        if (!failed && value !== expected) begin
            $display("FAIL negative-base-partial-write WIDTH=%0d", WIDTH);
            failed = 1;
        end

        expected = value;
        signed_base = 'x;
        value[signed_base +: 8] = 8'hff;
        value[signed_base] = 1'b1;
        signed_base = -1;
        value[signed_base] = 1'b1;
        if (!failed && value !== expected) begin
            $display("FAIL invalid-base-write WIDTH=%0d", WIDTH);
            failed = 1;
        end

        value = '0;
        signed_base = WIDTH-1;
        value[signed_base -: 8] = 8'h3c;
        if (!failed && (value[WIDTH-1 -: 8] !== 8'h3c ||
                        value[64] !== 1'b0 || value[0] !== 1'b0)) begin
            $display("FAIL high-write WIDTH=%0d", WIDTH);
            failed = 1;
        end

        // Narrow unsigned indices retain their numeric value even when their
        // own MSB is set. The same bit patterns are negative when signed.
        value = '0;
        value[128] = 1'b1;
        value[255] = 1'b1;
        unsigned_index8 = 8'h80;
        if (!failed && value[unsigned_index8] !== 1'b1) begin
            $display("FAIL unsigned-8-bit-index-128 WIDTH=%0d", WIDTH);
            failed = 1;
        end
        unsigned_index8 = 8'hff;
        if (!failed && value[unsigned_index8] !== 1'b1) begin
            $display("FAIL unsigned-8-bit-index-255 WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (WIDTH > 32768) begin
            value[32768] = 1'b1;
            unsigned_index16 = 16'h8000;
            if (!failed && value[unsigned_index16] !== 1'b1) begin
                $display("FAIL unsigned-16-bit-index-32768 WIDTH=%0d", WIDTH);
                failed = 1;
            end
        end
        signed_index8 = 8'h80;
        if (!failed && value[signed_index8] !== 1'bx) begin
            $display("FAIL signed-8-bit-index-negative-128 WIDTH=%0d", WIDTH);
            failed = 1;
        end
        signed_index8 = 8'hff;
        signed_index16 = 16'h8000;
        if (!failed && (value[signed_index8] !== 1'bx ||
                        value[signed_index16] !== 1'bx)) begin
            $display("FAIL signed-negative-bit-index WIDTH=%0d", WIDTH);
            failed = 1;
        end

        value = '0;
        value[128 +: 8] = 8'ha6;
        value[255 +: 8] = 8'h5b;
        unsigned_index8 = 8'h80;
        slice = value[unsigned_index8 +: 8];
        if (!failed && slice !== 8'ha6) begin
            $display("FAIL unsigned-8-bit-base-128-read WIDTH=%0d", WIDTH);
            failed = 1;
        end
        unsigned_index8 = 8'hff;
        slice = value[unsigned_index8 +: 8];
        if (!failed && slice !== 8'h5b) begin
            $display("FAIL unsigned-8-bit-base-255-read WIDTH=%0d", WIDTH);
            failed = 1;
        end
        if (WIDTH > 32775) begin
            value[32768 +: 8] = 8'hc6;
            unsigned_index16 = 16'h8000;
            slice = value[unsigned_index16 +: 8];
            if (!failed && slice !== 8'hc6) begin
                $display("FAIL unsigned-16-bit-base-read WIDTH=%0d", WIDTH);
                failed = 1;
            end
        end
        value[6:0] = 7'b1011010;
        signed_index8 = 8'hff;
        slice = value[signed_index8 +: 8];
        if (!failed && slice !== 8'b1011010x) begin
            $display("FAIL signed-minus-one-partial-read WIDTH=%0d", WIDTH);
            failed = 1;
        end
        signed_index8 = 8'h80;
        signed_index16 = 16'h8000;
        if (!failed && (value[signed_index8 +: 8] !== 8'hxx ||
                        value[signed_index16 +: 8] !== 8'hxx)) begin
            $display("FAIL signed-negative-part-read WIDTH=%0d", WIDTH);
            failed = 1;
        end

        value = '0;
        unsigned_index8 = 8'h80;
        value[unsigned_index8] = 1'b1;
        unsigned_index8 = 8'hff;
        value[unsigned_index8 +: 8] = 8'hc3;
        expected = '0;
        expected[128] = 1'b1;
        expected[255 +: 8] = 8'hc3;
        if (WIDTH > 32775) begin
            unsigned_index16 = 16'h8000;
            value[unsigned_index16 +: 8] = 8'h69;
            expected[32768 +: 8] = 8'h69;
        end
        if (!failed && value !== expected) begin
            $display("FAIL narrow-unsigned-index-writes WIDTH=%0d", WIDTH);
            failed = 1;
        end

        signed_index8 = 8'h80;
        value[signed_index8] = 1'b1;
        value[signed_index8 +: 8] = 8'hff;
        signed_index16 = 16'h8000;
        value[signed_index16] = 1'b1;
        value[signed_index16 +: 8] = 8'hff;
        if (!failed && value !== expected) begin
            $display("FAIL signed-negative-invalid-writes WIDTH=%0d", WIDTH);
            failed = 1;
        end

        value = '0;
        signed_index8 = 8'hff;
        value[signed_index8 +: 8] = 8'b11010110;
        expected = '0;
        expected[6:0] = 7'b1101011;
        if (!failed && value !== expected) begin
            $display("FAIL signed-minus-one-partial-write WIDTH=%0d", WIDTH);
            failed = 1;
        end

        // Fixed unpacked arrays use the same signed numeric index value; a
        // negative index neither aliases its unsigned bit pattern nor writes.
        fixed_values[128] = 8'h12;
        fixed_values[255] = 8'h34;
        fixed_values[32768] = 8'h56;
        unsigned_index8 = 8'h80;
        array_value = fixed_values[unsigned_index8];
        if (!failed && array_value !== 8'h12) begin
            $display("FAIL unsigned-array-index-128 WIDTH=%0d", WIDTH);
            failed = 1;
        end
        unsigned_index8 = 8'hff;
        if (!failed && fixed_values[unsigned_index8] !== 8'h34) begin
            $display("FAIL unsigned-array-index-255 WIDTH=%0d", WIDTH);
            failed = 1;
        end
        unsigned_index16 = 16'h8000;
        if (!failed && fixed_values[unsigned_index16] !== 8'h56) begin
            $display("FAIL unsigned-array-index-32768 WIDTH=%0d", WIDTH);
            failed = 1;
        end
        signed_index8 = 8'hff;
        array_value = fixed_values[signed_index8];
        fixed_values[signed_index8] = 8'hff;
        signed_index8 = 8'h80;
        signed_index16 = 16'h8000;
        if (!failed && (array_value !== 8'hxx ||
                        fixed_values[signed_index8] !== 8'hxx ||
                        fixed_values[signed_index16] !== 8'hxx ||
                        fixed_values[255] !== 8'h34 ||
                        fixed_values[32768] !== 8'h56)) begin
            $display("FAIL signed-negative-array-index WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS select_edges WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
