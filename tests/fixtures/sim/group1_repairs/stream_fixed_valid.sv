module tb;
    logic [7:0] a[0:3];
    logic [7:0] b[3:0];
    int lo;
    int hi;
    int iteration;
    initial begin
        lo = 0;
        hi = 1;
        a[0] = 0; a[1] = 0; a[2] = 0; a[3] = 0;
        b[0] = 0; b[1] = 0; b[2] = 0; b[3] = 0;
        // Repeated runtime-bound targets exercise the registered index owner.
        for (iteration = 0; iteration < 1000; iteration = iteration + 1)
            {>>{a with [lo:hi]}} = 16'h1234;
        if (a[0] !== 8'h12 || a[1] !== 8'h34 || a[2] !== 0)
            $fatal(1, "exact-fit stream");
        {>>{a with [lo:hi]}} = 32'haabbccdd;
        if (a[0] !== 8'haa || a[1] !== 8'hbb) $fatal(1, "excess source MSBs");
        {>>{a with [lo:hi], b with [hi:lo]}} = 32'h10203040;
        if (a[0] !== 8'h10 || a[1] !== 8'h20 || b[1] !== 8'h30 || b[0] !== 8'h40)
            $fatal(1, "multiple fixed targets");
        $display("fixed streaming passed");
        $finish(0);
    end
endmodule
