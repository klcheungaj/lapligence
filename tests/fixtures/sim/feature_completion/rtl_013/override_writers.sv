// IEEE 1800-2009 9.2.2.2-9.2.2.4 and 10.6.2: force and release are overrides,
// not assignments, so another process may override storage that always_ff or
// always_comb owns. A released variable keeps its forced value until its
// owner next assigns it.
module tb;
    logic clk;
    logic [7:0] d, q, x, y;

    always_ff @(posedge clk) q <= d;
    always_comb y = x + 8'd1;

    initial begin
        clk = 0;
        d = 8'h11;
        x = 8'h20;
        #1 clk = 1;
        #1 $display("t2 %h %h", q, y);
        force q = 8'haa;
        force y = 8'hbb;
        clk = 0;
        d = 8'h22;
        x = 8'h30;
        #1 clk = 1;
        #1 $display("t4 %h %h", q, y);
        release q;
        release y;
        #1 $display("t5 %h %h", q, y);
        clk = 0;
        x = 8'h40;
        #1 clk = 1;
        #1 $display("t7 %h %h", q, y);
        $finish(0);
    end
endmodule
