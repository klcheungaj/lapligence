module tb;
    logic [7:0] a[0:3];
    logic [7:0] b[0:3];
    int lo;
    int hi;
    initial begin
        lo = 0; hi = 1;
        {>>{a with [lo:hi], b with [lo:hi]}} = 24'h112233;
        $display("ERROR: short second segment was accepted");
        $finish(0);
    end
endmodule
