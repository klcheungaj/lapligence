module tb;
    logic scalar = 0;
    logic [63:0] narrow = 0;
    logic [64:0] wide = 0;
    int pos_count = 0;
    int neg_count = 0;
    int narrow_count = 0;
    int wide_count = 0;
    initial repeat (3) begin @(posedge scalar); pos_count++; end
    initial repeat (3) begin @(negedge scalar); neg_count++; end
    initial repeat (4) begin @(narrow); narrow_count++; end
    initial repeat (4) begin @(wide); wide_count++; end
    initial begin
        #1 scalar = 1'bx; narrow[63] = 1'bx; wide[64] = 1'bx;
        #1 scalar = 1'bz; narrow[63] = 1'bz; wide[64] = 1'bz;
        #1 scalar = 1; narrow[0] = 1'bx; wide[0] = 1'bx;
        #1 scalar = 1'bz; narrow[0] = 1'bz; wide[0] = 1'bz;
        #1 scalar = 0;
        #1 scalar = 1;
        #1 scalar = 0;
        #1 $display("%0d %0d %0d %0d", pos_count, neg_count, narrow_count, wide_count);
        $finish(0);
    end
endmodule
