// IEEE 1800-2009 10.3, 10.9 and 10.11: positional pattern leaves on true-net
// aliases are independent continuous drivers of the shared network.
module tb;
    typedef logic [1:0][3:0] pair_t;
    wire [7:0] a, b;
    wire [3:0] c;
    wire [7:0] e;
    alias a = b;
    pair_t x = 8'h12;
    logic [3:0] y = 4'hz;
    // Selected bits of one alias view and an ordinary net.
    assign '{a[7:4], c} = x;
    // Independent drivers through the other view compete on a[7:4].
    assign b[7:4] = y;
    assign b[3:0] = y;
    // Ordinary net: two pattern leaves plus an overlapping selected driver.
    assign '{e[7:4], e[3:0]} = x;
    assign e[5:2] = y;
    initial begin
        #1 $display("%h %h %h %h", a, b, c, e);
        y = 4'h1;
        #1 $display("%h %h %h %h", a, b, c, e);
        y = 4'h3;
        #1 $display("%h %h %h %h", a, b, c, e);
        x = 8'h3c;
        #1 $display("%h %h %h %h", a, b, c, e);
        y = 4'hz;
        #1 $display("%h %h %h %h", a, b, c, e);
        $finish(0);
    end
endmodule
