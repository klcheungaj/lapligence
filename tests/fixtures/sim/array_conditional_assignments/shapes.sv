module tb;
    typedef logic [6:0] matrix_t [2:1][-1:0];
    typedef bit [7:0] bits_t [0:1];
    typedef logic [128:0] wide_t [0:1];
    typedef struct packed { logic [3:0] tag; logic [3:0] data; } lane_t;
    typedef lane_t lanes_t [0:1];
    matrix_t a, b, result;
    bits_t ba, bb, br;
    wide_t wa, wb, wr;
    lanes_t la, lb, lr;
    logic selector;
    initial begin
        a = '{'{7'd1, 7'd2}, '{7'd4, 7'd5}};
        b = '{'{7'd1, 7'd3}, '{7'd4, 7'd5}};
        ba = '{8'ha5, 8'h5a}; bb = '{8'ha6, 8'h5a};
        wa = '{129'h100000000000000000000000000000005, 129'h123456789abcdef};
        wb = '{129'h100000000000000000000000000000006, 129'h123456789abcdef};
        la[0] = 8'ha5; la[1] = 8'h5a; lb[0] = 8'ha6; lb[1] = 8'h5a;
        selector = 1'bx;
        result = selector ? a : b;
        br = selector ? ba : bb;
        wr = selector ? wa : wb;
        lr = selector ? la : lb;
        if (result[2][-1] !== 7'hxx || result[2][0] !== 7'hxx ||
            result[1][-1] !== 7'd4 || result[1][0] !== 7'd5) $fatal(1, "whole row defaults");
        if (br[0] !== 8'h00 || br[1] !== 8'h5a) $fatal(1, "two state element defaults");
        if (wr[0] !== {129{1'bx}} || wr[1] !== wa[1]) $fatal(1, "wide element boundaries");
        if (lr[0] !== 8'hxx || lr[1] !== 8'h5a) $fatal(1, "whole packed record defaults");
        selector = 1;
        result = selector ? a : b; br = selector ? ba : bb;
        wr = selector ? wa : wb; lr = selector ? la : lb;
        if (result[2][0] !== 7'd2 || br[0] !== 8'ha5 || wr[0] !== wa[0] || lr[0] !== 8'ha5)
            $fatal(1, "known selector preserves values");
        $display("shapes passed");
        $finish(0);
    end
endmodule
