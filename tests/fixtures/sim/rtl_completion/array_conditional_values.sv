// IEEE 1800-2009 11.4.11: unpacked elements are not packed mux bits.
module tb;
    typedef logic [7:0] array_t [-2:-1];
    array_t a, b, result;
    logic [1:0] selector;
    logic [7:0] packed_result;

    function automatic array_t choose(input logic [1:0] sel, input array_t x, y);
        return sel ? x : y;
    endfunction

    function automatic array_t choose_local(input logic [1:0] sel, input array_t x, y);
        array_t local_value;
        local_value = sel ? x : y;
        return local_value;
    endfunction

    function automatic logic [7:0] consume(input array_t value);
        return value[-2];
    endfunction

    function automatic array_t nested(input logic outer, inner, input array_t x, y);
        return outer ? x : (inner ? x : y);
    endfunction

    initial begin
        a[-2] = 8'ha5; a[-1] = 8'h5a;
        b[-2] = 8'ha6; b[-1] = 8'h5a;
        selector = 0;
        result = choose(selector, a, b);
        if (result[-2] !== 8'ha6 || result[-1] !== 8'h5a) $fatal(1, "known false");
        selector = 1;
        result = choose(selector, a, b);
        if (result[-2] !== 8'ha5 || result[-1] !== 8'h5a) $fatal(1, "known true");
        selector = 2'b0x;
        result = choose(selector, a, b);
        packed_result = selector ? a[-2] : b[-2];
        if (result[-2] !== 8'hxx || result[-1] !== 8'h5a) $fatal(1, "whole element X");
        if (packed_result !== 8'b101001xx) $fatal(1, "packed mux changed");
        if (consume(choose(selector, a, b)) !== 8'hxx) $fatal(1, "value argument merge");
        result = choose_local(selector, a, b);
        if (result[-2] !== 8'hxx || result[-1] !== 8'h5a) $fatal(1, "local assignment");
        selector = 2'b0z;
        result = choose(selector, a, b);
        if (result[-2] !== 8'hxx || result[-1] !== 8'h5a) $fatal(1, "Z selector");
        selector = 2'bx1;
        result = choose(selector, a, b);
        if (result[-2] !== 8'ha5) $fatal(1, "known one dominates X");
        selector = 2'b0x;
        result = nested(selector[0], 1'b0, a, b);
        if (result[-2] !== 8'hxx || result[-1] !== 8'h5a) $fatal(1, "nested conditional");
        // Logical equality, rather than case equality, controls preservation.
        a[-2] = 8'b10xz0101; b[-2] = a[-2];
        result = choose(selector, a, b);
        if (result[-2] !== 8'hxx || result[-1] !== 8'h5a) $fatal(1, "unknown equality");
        selector = 1;
        result = choose(selector, a, b);
        if (result[-2] !== 8'b10xz0101) $fatal(1, "selected payload must retain X/Z");
        b[-2] = 8'h5a; a[-2] = 8'h5a;
        selector = 2'b0x;
        result = choose(selector, a, b);
        if (result[-2] !== 8'h5a || result[-1] !== 8'h5a) $fatal(1, "equal elements");
        $display("array conditional values passed");
        $finish(0);
    end
endmodule
