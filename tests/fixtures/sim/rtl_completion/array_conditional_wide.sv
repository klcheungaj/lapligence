module tb;
    typedef logic signed [64:0] signed_array_t [-1:-2];
    typedef bit [64:0] bit_array_t [7:6];
    signed_array_t a, b, result;
    bit_array_t ba, bb, br;
    logic selector;

    function automatic signed_array_t choose_signed(input logic sel, input signed_array_t x, y);
        return sel ? x : y;
    endfunction
    function automatic bit_array_t choose_bits(input logic sel, input bit_array_t x, y);
        return sel ? x : y;
    endfunction

    initial begin
        a[-1] = 65'h1_0000_0000_0000_00a5;
        b[-1] = 65'h0_0000_0000_0000_00a5;
        a[-2] = -2; b[-2] = -2;
        selector = 1;
        result = choose_signed(selector, a, b);
        if (result[-1] !== a[-1] || result[-2] !== -2 || result[-2] >= 0)
            $fatal(1, "selected signed elements");
        selector = 1'bx;
        result = choose_signed(selector, a, b);
        if (result[-1] !== {65{1'bx}} || result[-2] !== -2)
            $fatal(1, "wide element merge");
        ba[7] = 65'h1_0000_0000_0000_00a5;
        bb[7] = 65'h1_0000_0000_0000_00a6;
        ba[6] = 65'h1_1234_5678_9abc_def0; bb[6] = ba[6];
        br = choose_bits(selector, ba, bb);
        if (br[7] !== 65'b0 || br[6] !== ba[6]) $fatal(1, "two-state default zero");
        selector = 1'bz;
        br = choose_bits(selector, ba, bb);
        if (br[7] !== 65'b0 || br[6] !== ba[6]) $fatal(1, "two-state Z selector");
        $display("array conditional wide states passed");
        $finish(0);
    end
endmodule
