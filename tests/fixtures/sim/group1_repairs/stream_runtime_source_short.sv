module tb;
    logic [7:0] a[0:3];
    logic [7:0] b[0:3];
    int lo;
    int source_hi;
    int destination_hi;
    initial begin
        lo = 0; source_hi = 0; destination_hi = 1;
        b[0] = 8'h55;
        {>>{a with [lo:destination_hi]}} = {>>{b with [lo:source_hi]}};
        $display("ERROR: short runtime source was accepted");
        $finish(0);
    end
endmodule
