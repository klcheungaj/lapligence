module tb;
    typedef logic [7:0] pair_t [0:1];
    typedef logic [7:0] quad_t [0:3];
    pair_t a, b, result;
    quad_t whole;
    logic [15:0] widened [0:3];
    logic selector;
    logic [15:0] payload;
    initial begin
        payload = 16'h12ab;
        a = pair_t'(payload);
        b = '{default: 8'h55};
        selector = 1;
        result = selector ? a : b;
        if (result[0] !== 8'h12 || result[1] !== 8'hab) $fatal(1, "bit stream cast control");
        result = selector ? pair_t'{8'h81, 8'hfe} : pair_t'(payload);
        if (result[0] !== 8'h81 || result[1] !== 8'hfe) $fatal(1, "typed pattern arm");
        selector = 0;
        result = selector ? pair_t'{8'h81, 8'hfe} : pair_t'(payload);
        if (result[0] !== 8'h12 || result[1] !== 8'hab) $fatal(1, "cast arm");
        whole = {result, b};
        if (whole[0] !== 8'h12 || whole[1] !== 8'hab || whole[2] !== 8'h55 || whole[3] !== 8'h55)
            $fatal(1, "whole array concatenation control");
        whole = {selector ? b : result, b};
        if (whole[0] !== 8'h12 || whole[1] !== 8'hab) $fatal(1, "conditional concat operand");
        // Array concatenation converts cells, not the concatenated bitstream.
        widened = {result, b};
        if (widened[0] !== 16'h0012 || widened[1] !== 16'h00ab ||
            widened[2] !== 16'h0055 || widened[3] !== 16'h0055)
            $fatal(1, "specialized per-element concatenation conversion");
        $display("expressions=%h,%h,%h,%h", whole[0], whole[1], whole[2], whole[3]);
        $finish(0);
    end
endmodule
