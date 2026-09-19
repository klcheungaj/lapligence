// IEEE 1800-2009 11.4.3, 11.4.10, 11.8.4: division or modulo by zero yields
// all-X, an unknown shift count yields all-X, and shifting by a count at or
// beyond the operand width yields zero-fill (or sign-fill for `>>>`) without
// invoking undefined C shifts. Widths cross the 64-bit limb boundary.
module tb;
    logic [7:0] a;
    logic signed [7:0] sa;
    logic [7:0] zero;
    logic [7:0] q, r, sl, sr;
    logic signed [7:0] ssr;
    logic [64:0] wa;
    logic signed [64:0] wsa;
    logic [64:0] wq, wsl, wsr;
    logic signed [64:0] wssr;
    logic [256:0] va;
    logic [256:0] vq, vsl, vsr;

    initial begin
        a = 8'd10;
        sa = -8'sd10;
        zero = 8'd0;

        q = a / 8'd0; r = a % 8'd0;
        $display("small const divz %b %b", q, r);
        q = a / zero; r = a % zero;
        $display("small runtime divz %b %b", q, r);
        q = a / 8'bx; r = a % 8'bz;
        $display("small unknown divz %b %b", q, r);
        sl = a << 8'd9; sr = a >> 8'd9; ssr = sa >>> 8'd9;
        $display("small overshift %b %b %b", sl, sr, ssr);
        sl = a << 8'bx; sr = a >> 8'bz;
        $display("small unknown shift %b %b", sl, sr);

        wa = 65'd10;
        wsa = -65'sd1;
        wq = wa / 65'd0;
        wsl = wa << 65'd200; wsr = wa >> 65'd200; wssr = wsa >>> 65'd200;
        $display("wide divz=%b shl=%b shr=%b ashr=%b", wq, wsl, wsr, wssr);

        va = 257'd10;
        vq = va / 257'd0;
        vsl = va << 257'd300; vsr = va >> 257'd300;
        $display("vwide divz=%b shl=%b shr=%b", vq, vsl, vsr);
        vsl = va << 257'bx;
        $display("vwide unknown shift=%b", vsl);

        $display("PASS divide_zero_and_shift");
        $finish(0);
    end
endmodule
