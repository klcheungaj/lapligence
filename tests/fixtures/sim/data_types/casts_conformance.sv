// llg-test-fixture: tests/fixtures/sim/data_types/casts_conformance.sv
// Expected-correct IEEE 1800-2009 section 6.24.1 cast behavior.
module tb;
    parameter WIDTH = 128;

    typedef logic [WIDTH-1:0] wide_u_t;
    typedef logic signed [WIDTH-1:0] wide_s_t;

    logic signed [31:0] signed_source;
    logic [31:0] unsigned_source;
    logic signed [7:0] signed_narrow;
    logic [WIDTH-1:0] unsigned_wide;
    logic signed [WIDTH-1:0] signed_result;
    logic [WIDTH-1:0] unsigned_result;
    logic [WIDTH-1:0] expected;

    initial begin
        signed_source = 32'h0001_8001;
        unsigned_source = 32'h0001_8002;
        signed_narrow = 8'h80;
        unsigned_wide = '0;
        unsigned_wide[WIDTH-1] = 1'b1;
        #1;

        // The source is positive and has bit 16 set. The size cast removes
        // that bit, leaving a signed 16'h8001 that sign-extends on assignment.
        signed_result = 16'(signed_source);
        expected = '1;
        expected[15:0] = 16'h8001;
        if (signed_result !== expected) begin
            $display("FAIL size-cast-signed");
            $finish;
        end

        // The unsigned size cast also removes source bit 16, but its result
        // zero-extends when assigned to the wide destination.
        unsigned_result = 16'(unsigned_source);
        expected = '0;
        expected[15:0] = 16'h8002;
        if (unsigned_result !== expected) begin
            $display("FAIL size-cast-unsigned");
            $finish;
        end

        // The typed cast first sign-extends the narrow signed source to WIDTH,
        // then retags it unsigned, so >>> is a logical shift producing one.
        unsigned_result = wide_u_t'(signed_narrow) >>> (WIDTH-1);
        expected = '0;
        expected[0] = 1'b1;
        if (unsigned_result !== expected) begin
            $display("FAIL typed-cast-unsigned-target");
            $finish;
        end

        // The typed cast retags the MSB-set wide operand signed, so >>> fills
        // all WIDTH result bits with ones rather than producing one.
        signed_result = wide_s_t'(unsigned_wide) >>> (WIDTH-1);
        expected = '1;
        if (signed_result !== expected) begin
            $display("FAIL typed-cast-signed-target");
            $finish;
        end

        $display("PASS casts-conformance WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
