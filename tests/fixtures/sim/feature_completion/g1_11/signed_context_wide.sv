// IEEE 1800-2009 11.8.2/11.8.3: a 65-bit and a 257-bit mixed-sign expression
// must size, sign/zero-extend and widen into the assignment context exactly
// as the LRM specifies, identically with and without optimization. Operands
// are built from known bit sets so the Rust oracle can recompute every result.
module tb;
    logic signed [64:0] s65;
    logic [64:0] u65;
    logic signed [64:0] s65b;
    logic signed [256:0] s257;
    logic [256:0] u257;
    logic signed [256:0] s257b;

    logic [64:0] n65;
    logic signed [129:0] w65;
    logic [256:0] n257;
    logic signed [513:0] w257;

    logic lt65, gt65, ss65;
    logic lt257, gt257, ss257;

    initial begin
        s65 = '0; s65[64] = 1'b1; s65[63] = 1'b1; s65[0] = 1'b1;
        u65 = '0; u65[63] = 1'b1; u65[0] = 1'b1;
        s65b = '0; s65b[1] = 1'b1; s65b[0] = 1'b1;

        s257 = '0; s257[256] = 1'b1; s257[255] = 1'b1; s257[64] = 1'b1; s257[0] = 1'b1;
        u257 = '0; u257[255] = 1'b1; u257[0] = 1'b1;
        s257b = '0; s257b[2] = 1'b1; s257b[0] = 1'b1;

        n65 = s65 + u65; w65 = s65 + u65;
        $display("add65 %b %b", n65, w65);
        n65 = s65 - u65; w65 = s65 - u65;
        $display("sub65 %b %b", n65, w65);
        n65 = s65 * u65; w65 = s65 * u65;
        $display("mulu65 %b %b", n65, w65);
        n65 = s65 * s65b; w65 = s65 * s65b;
        $display("muls65 %b %b", n65, w65);
        n65 = s65 & u65; w65 = s65 & u65;
        $display("and65 %b %b", n65, w65);
        n65 = s65 | u65; w65 = s65 | u65;
        $display("or65 %b %b", n65, w65);
        n65 = s65 ^ u65; w65 = s65 ^ u65;
        $display("xor65 %b %b", n65, w65);
        n65 = s65 << 32'd5; w65 = s65 << 32'd5;
        $display("shl65 %b %b", n65, w65);
        n65 = s65 >>> 32'd5; w65 = s65 >>> 32'd5;
        $display("ashr65 %b %b", n65, w65);
        lt65 = s65 < u65; gt65 = s65 > u65; ss65 = s65 < s65b;
        $display("cmp65 %b %b %b", lt65, gt65, ss65);

        n257 = s257 + u257; w257 = s257 + u257;
        $display("add257 %b %b", n257, w257);
        n257 = s257 - u257; w257 = s257 - u257;
        $display("sub257 %b %b", n257, w257);
        n257 = s257 * u257; w257 = s257 * u257;
        $display("mulu257 %b %b", n257, w257);
        n257 = s257 * s257b; w257 = s257 * s257b;
        $display("muls257 %b %b", n257, w257);
        n257 = s257 & u257; w257 = s257 & u257;
        $display("and257 %b %b", n257, w257);
        n257 = s257 | u257; w257 = s257 | u257;
        $display("or257 %b %b", n257, w257);
        n257 = s257 ^ u257; w257 = s257 ^ u257;
        $display("xor257 %b %b", n257, w257);
        n257 = s257 << 32'd5; w257 = s257 << 32'd5;
        $display("shl257 %b %b", n257, w257);
        n257 = s257 >>> 32'd5; w257 = s257 >>> 32'd5;
        $display("ashr257 %b %b", n257, w257);
        lt257 = s257 < u257; gt257 = s257 > u257; ss257 = s257 < s257b;
        $display("cmp257 %b %b %b", lt257, gt257, ss257);

        $display("PASS signed_context_wide");
        $finish(0);
    end
endmodule
