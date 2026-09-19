module tb;
    logic [7:0] a[0:3];
    int lo;
    int hi;
    initial begin
        lo = 0; hi = 1;
        {>>{a with [lo:hi]}} = 15'h1234;
        $display("ERROR: short stream was accepted");
        $finish(0);
    end
endmodule
